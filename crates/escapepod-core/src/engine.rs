// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pass-through tap/hold state machine.

use std::collections::BTreeMap;

use crate::{DualRole, Input, Instant, KeyCode, KeyEvent, KeyValue, Role, Rules};

/// What the engine decided about a managed key, for the status socket and
/// `event monitor`. Unmanaged keys produce no decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// A dual-role key went down; its hold key was pressed.
    Hold(DualRole),
    /// A dual-role key was released alone and in time; its tap key was
    /// tapped.
    Tap(DualRole),
    /// A dual-role key was released after another key or a pointer button
    /// was pressed while it was held.
    Used(DualRole),
    /// A dual-role key was released later than the tap timeout.
    TimedOut(DualRole),
    /// A remap source produced an event, emitted for its target.
    Remap {
        /// The physical key.
        source: KeyCode,
        /// The key emitted.
        target: KeyCode,
    },
}

#[derive(Clone, Copy, Debug)]
struct Held {
    pressed_at: Instant,
    used: bool,
}

/// The tap/hold engine for one keyboard.
///
/// The engine is pure: it does no I/O and never reads a clock. Each call to
/// [`Engine::step`] takes one input with its time and appends the events to
/// emit. The hold key goes down at once; the tap key is added only on a
/// lone, quick release; nothing is buffered or delayed.
///
/// # Examples
///
/// ```
/// use escapepod_core::{Engine, Input, Instant, KeyCode, KeyEvent, Rules};
///
/// let rules = Rules::builder()
///     .dual_role(KeyCode::CAPSLOCK, KeyCode::LEFTCTRL, KeyCode::ESC)
///     .build()?;
/// let mut engine = Engine::new(rules);
/// let mut out = Vec::new();
/// engine.step(Input::Key(KeyEvent::press(KeyCode::CAPSLOCK)), Instant::from_micros(0), &mut out);
/// engine.step(Input::Key(KeyEvent::release(KeyCode::CAPSLOCK)), Instant::from_micros(50_000), &mut out);
/// assert_eq!(out, [
///     KeyEvent::press(KeyCode::LEFTCTRL),
///     KeyEvent::release(KeyCode::LEFTCTRL),
///     KeyEvent::press(KeyCode::ESC),
///     KeyEvent::release(KeyCode::ESC),
/// ]);
/// # Ok::<(), escapepod_core::RulesError>(())
/// ```
#[derive(Clone, Debug)]
pub struct Engine {
    rules: Rules,
    held: BTreeMap<KeyCode, Held>,
}

impl Engine {
    /// An engine applying `rules`, with no key held.
    #[must_use]
    pub fn new(rules: Rules) -> Self {
        Self {
            rules,
            held: BTreeMap::new(),
        }
    }

    /// The rules this engine applies.
    #[must_use]
    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// Consumes one input at time `now`, appends the events to emit to
    /// `out`, and returns the decision about a managed key, if any.
    pub fn step(
        &mut self,
        input: Input,
        now: Instant,
        out: &mut Vec<KeyEvent>,
    ) -> Option<Decision> {
        let ev = match input {
            Input::Key(ev) => ev,
            Input::PointerPress | Input::ForeignPress => {
                self.mark_used(None);
                return None;
            }
        };
        let role = self.rules.role(ev.key);
        if ev.value == KeyValue::Press && !matches!(role, Role::DualRole(_)) {
            self.mark_used(None);
        }
        match role {
            Role::Protected | Role::Other => {
                out.push(ev);
                None
            }
            Role::Remap(target) => {
                out.push(KeyEvent {
                    key: target,
                    value: ev.value,
                });
                Some(Decision::Remap {
                    source: ev.key,
                    target,
                })
            }
            Role::DualRole(d) => Some(self.dual(d, ev.value, now, out)),
        }
    }

    fn dual(
        &mut self,
        d: DualRole,
        value: KeyValue,
        now: Instant,
        out: &mut Vec<KeyEvent>,
    ) -> Decision {
        match value {
            KeyValue::Press if !self.held.contains_key(&d.key) => {
                self.mark_used(Some(d.key));
                self.held.insert(
                    d.key,
                    Held {
                        pressed_at: now,
                        used: false,
                    },
                );
                out.push(KeyEvent::press(d.hold));
                Decision::Hold(d)
            }
            // A second press while held, or the kernel's autorepeat: neither
            // is another key, so neither marks the key used (ESC-SRS-014).
            KeyValue::Press | KeyValue::Repeat => {
                out.push(KeyEvent::repeat(d.hold));
                Decision::Hold(d)
            }
            KeyValue::Release => {
                out.push(KeyEvent::release(d.hold));
                let Some(h) = self.held.remove(&d.key) else {
                    return Decision::Used(d);
                };
                if h.used {
                    Decision::Used(d)
                } else if now.saturating_since(h.pressed_at) > self.rules.tap_timeout() {
                    Decision::TimedOut(d)
                } else {
                    out.push(KeyEvent::press(d.tap));
                    out.push(KeyEvent::release(d.tap));
                    Decision::Tap(d)
                }
            }
        }
    }

    /// Marks every held dual-role key used, except `except`.
    fn mark_used(&mut self, except: Option<KeyCode>) {
        for (k, h) in &mut self.held {
            if Some(*k) != except {
                h.used = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const CAPS: KeyCode = KeyCode::CAPSLOCK;
    const CTRL: KeyCode = KeyCode::LEFTCTRL;
    const ESC: KeyCode = KeyCode::ESC;

    fn key(name: &str) -> KeyCode {
        KeyCode::from_name(name).unwrap_or(ESC)
    }

    fn ms(n: u64) -> Instant {
        Instant::from_micros(n * 1000)
    }

    fn caps_engine() -> Engine {
        let rules = Rules::builder()
            .dual_role(CAPS, CTRL, ESC)
            .build()
            .unwrap_or_default();
        Engine::new(rules)
    }

    fn run(engine: &mut Engine, inputs: &[(Input, Instant)]) -> Vec<KeyEvent> {
        let mut out = Vec::new();
        for &(i, t) in inputs {
            engine.step(i, t, &mut out);
        }
        out
    }

    fn k(ev: KeyEvent) -> Input {
        Input::Key(ev)
    }

    // Verifies: ESC-SRS-009
    #[test]
    fn hold_key_goes_down_with_the_press() {
        let mut e = caps_engine();
        let mut out = Vec::new();
        let d = e.step(k(KeyEvent::press(CAPS)), ms(0), &mut out);
        assert_eq!(out, [KeyEvent::press(CTRL)]);
        assert!(matches!(d, Some(Decision::Hold(_))));
    }

    // Verifies: ESC-SRS-010
    #[test]
    fn lone_quick_release_taps_after_the_hold_release() {
        let mut e = caps_engine();
        let out = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(0)),
                (k(KeyEvent::release(CAPS)), ms(200)),
            ],
        );
        assert_eq!(
            out,
            [
                KeyEvent::press(CTRL),
                KeyEvent::release(CTRL),
                KeyEvent::press(ESC),
                KeyEvent::release(ESC)
            ]
        );
    }

    // Verifies: ESC-SRS-011
    #[test]
    fn another_key_cancels_the_tap() {
        let mut e = caps_engine();
        let c = key("KEY_C");
        let out = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(0)),
                (k(KeyEvent::press(c)), ms(10)),
                (k(KeyEvent::release(c)), ms(20)),
                (k(KeyEvent::release(CAPS)), ms(30)),
            ],
        );
        assert_eq!(
            out,
            [
                KeyEvent::press(CTRL),
                KeyEvent::press(c),
                KeyEvent::release(c),
                KeyEvent::release(CTRL)
            ]
        );
    }

    // Verifies: ESC-SRS-012
    #[test]
    fn pointer_press_cancels_the_tap() {
        let mut e = caps_engine();
        let out = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(0)),
                (Input::PointerPress, ms(10)),
                (k(KeyEvent::release(CAPS)), ms(30)),
            ],
        );
        assert_eq!(out, [KeyEvent::press(CTRL), KeyEvent::release(CTRL)]);
        let mut e = caps_engine();
        let out = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(0)),
                (Input::ForeignPress, ms(10)),
                (k(KeyEvent::release(CAPS)), ms(30)),
            ],
        );
        assert_eq!(out, [KeyEvent::press(CTRL), KeyEvent::release(CTRL)]);
    }

    // Verifies: ESC-SRS-013
    #[test]
    fn late_release_does_not_tap() {
        let mut e = caps_engine();
        let mut out = Vec::new();
        e.step(k(KeyEvent::press(CAPS)), ms(0), &mut out);
        let d = e.step(
            k(KeyEvent::release(CAPS)),
            Instant::from_micros(200_001),
            &mut out,
        );
        assert_eq!(out, [KeyEvent::press(CTRL), KeyEvent::release(CTRL)]);
        assert!(matches!(d, Some(Decision::TimedOut(_))));
    }

    // Verifies: ESC-SRS-014
    #[test]
    fn own_autorepeat_keeps_the_tap() {
        let mut e = caps_engine();
        let out = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(0)),
                (k(KeyEvent::repeat(CAPS)), ms(100)),
                (k(KeyEvent::repeat(CAPS)), ms(130)),
                (k(KeyEvent::release(CAPS)), ms(150)),
            ],
        );
        assert_eq!(
            out,
            [
                KeyEvent::press(CTRL),
                KeyEvent::repeat(CTRL),
                KeyEvent::repeat(CTRL),
                KeyEvent::release(CTRL),
                KeyEvent::press(ESC),
                KeyEvent::release(ESC),
            ]
        );
    }

    // Verifies: ESC-SRS-015
    #[test]
    fn overlapping_dual_role_keys_cancel_the_first_tap() {
        let enter = key("KEY_ENTER");
        let rctrl = key("KEY_RIGHTCTRL");
        let rules = Rules::builder()
            .dual_role(CAPS, CTRL, ESC)
            .dual_role(enter, rctrl, enter)
            .build()
            .unwrap_or_default();
        let mut e = Engine::new(rules);
        let out = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(0)),
                (k(KeyEvent::press(enter)), ms(10)),
                (k(KeyEvent::release(enter)), ms(20)),
                (k(KeyEvent::release(CAPS)), ms(30)),
            ],
        );
        // The second key was pressed alone after the first and released in
        // time, so it taps; the first was used by the second.
        assert_eq!(
            out,
            [
                KeyEvent::press(CTRL),
                KeyEvent::press(rctrl),
                KeyEvent::release(rctrl),
                KeyEvent::press(enter),
                KeyEvent::release(enter),
                KeyEvent::release(CTRL),
            ]
        );
    }

    // Verifies: ESC-SRS-016
    #[test]
    fn remap_covers_all_three_event_kinds() {
        let rules = Rules::builder()
            .remap(CAPS, CTRL)
            .build()
            .unwrap_or_default();
        let mut e = Engine::new(rules);
        let out = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(0)),
                (k(KeyEvent::repeat(CAPS)), ms(500)),
                (k(KeyEvent::release(CAPS)), ms(600)),
            ],
        );
        assert_eq!(
            out,
            [
                KeyEvent::press(CTRL),
                KeyEvent::repeat(CTRL),
                KeyEvent::release(CTRL)
            ]
        );
    }

    // Verifies: ESC-SRS-017
    #[test]
    fn unmanaged_keys_pass_unchanged_in_order() {
        let mut e = caps_engine();
        let (a, b) = (key("KEY_A"), key("KEY_B"));
        let input = [
            KeyEvent::press(a),
            KeyEvent::press(b),
            KeyEvent::repeat(b),
            KeyEvent::release(a),
            KeyEvent::release(b),
        ];
        let timed: Vec<_> = input.iter().map(|&ev| (k(ev), ms(1))).collect();
        assert_eq!(run(&mut e, &timed), input);
    }

    // Verifies: ESC-SRS-020
    #[test]
    fn protected_keys_pass_unchanged() {
        let rules = Rules::builder()
            .dual_role(CAPS, CTRL, ESC)
            .protect(CAPS)
            .build()
            .unwrap_or_default();
        let mut e = Engine::new(rules);
        let input = [
            KeyEvent::press(CAPS),
            KeyEvent::repeat(CAPS),
            KeyEvent::release(CAPS),
        ];
        let timed: Vec<_> = input.iter().map(|&ev| (k(ev), ms(1))).collect();
        assert_eq!(run(&mut e, &timed), input);
    }

    // Verifies: ESC-SRS-019
    #[test]
    fn default_timeout_decides_the_tap() {
        let mut e = caps_engine();
        assert_eq!(e.rules().tap_timeout(), Duration::from_millis(200));
        let on_time = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(0)),
                (k(KeyEvent::release(CAPS)), ms(200)),
            ],
        );
        assert_eq!(on_time.len(), 4);
        let late = run(
            &mut e,
            &[
                (k(KeyEvent::press(CAPS)), ms(1000)),
                (k(KeyEvent::release(CAPS)), ms(1201)),
            ],
        );
        assert_eq!(late.len(), 2);
    }

    #[test]
    fn release_without_press_does_not_tap() {
        let mut e = caps_engine();
        let out = run(&mut e, &[(k(KeyEvent::release(CAPS)), ms(0))]);
        assert_eq!(out, [KeyEvent::release(CTRL)]);
    }
}

// Rust guideline compliant 2026-05-18
