// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Many keyboards, one virtual keyboard: the per-device engines, the
//! emergency chord and the held-keys ledger.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Chord, Decision, Engine, Input, Instant, KeyCode, KeyEvent, KeyValue, Rules};

/// Identifies one grabbed device within a [`Session`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Holder {
    Keyboard(DeviceId),
    Button(DeviceId, KeyCode),
}

/// Every key held down on the virtual keyboard, and the devices holding
/// it.
///
/// A press reaches the virtual keyboard only when the first device holds
/// the key, and a release only when the last one lets go, so two keyboards
/// holding Control produce one Control. Every exit path empties the ledger
/// with [`HeldKeys::release_all`].
///
/// # Examples
///
/// ```
/// use escapepod_core::{DeviceId, HeldKeys, KeyCode, KeyEvent};
///
/// let mut held = HeldKeys::default();
/// assert!(held.apply(DeviceId(1), KeyEvent::press(KeyCode::LEFTCTRL)));
/// assert!(!held.apply(DeviceId(2), KeyEvent::press(KeyCode::LEFTCTRL)));
/// let mut out = Vec::new();
/// held.release_all(&mut out);
/// assert_eq!(out, [KeyEvent::release(KeyCode::LEFTCTRL)]);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldKeys {
    keys: BTreeMap<KeyCode, BTreeSet<Holder>>,
}

impl HeldKeys {
    /// Records `ev` from `dev`; returns whether it reaches the virtual
    /// keyboard.
    pub fn apply(&mut self, dev: DeviceId, ev: KeyEvent) -> bool {
        self.apply_holder(Holder::Keyboard(dev), ev)
    }

    fn apply_holder(&mut self, holder: Holder, ev: KeyEvent) -> bool {
        match ev.value {
            KeyValue::Press => {
                let holders = self.keys.entry(ev.key).or_default();
                let first = holders.is_empty();
                holders.insert(holder);
                first
            }
            KeyValue::Repeat => self.keys.get(&ev.key).is_some_and(|h| h.contains(&holder)),
            KeyValue::Release => {
                let Some(holders) = self.keys.get_mut(&ev.key) else {
                    return false;
                };
                if !holders.remove(&holder) {
                    return false;
                }
                let last = holders.is_empty();
                if last {
                    self.keys.remove(&ev.key);
                }
                last
            }
        }
    }

    /// Releases every key `dev` holds; appends the releases that reach the
    /// virtual keyboard to `out`.
    pub fn release_device(&mut self, dev: DeviceId, out: &mut Vec<KeyEvent>) {
        let mine: Vec<KeyCode> = self
            .keys
            .iter()
            .filter(|(_, h)| h.contains(&Holder::Keyboard(dev)))
            .map(|(&k, _)| k)
            .collect();
        for k in mine {
            if self.apply(dev, KeyEvent::release(k)) {
                out.push(KeyEvent::release(k));
            }
        }
    }

    /// Releases every held key; appends one release per key to `out`.
    pub fn release_all(&mut self, out: &mut Vec<KeyEvent>) {
        out.extend(self.keys.keys().map(|&k| KeyEvent::release(k)));
        self.keys.clear();
    }

    /// The keys held down on the virtual keyboard, in key-code order.
    pub fn keys(&self) -> impl Iterator<Item = KeyCode> + '_ {
        self.keys.keys().copied()
    }

    /// Whether no key is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// The result of one [`Session::on_key`] call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// The engine's decision about a managed key, if any.
    pub decision: Option<Decision>,
    /// Whether this event completed the emergency chord. When it did, the
    /// session has already released every key.
    pub emergency: bool,
}

/// The engines of every grabbed device, the emergency chord and the
/// held-keys ledger.
///
/// The session is pure, like the engine: the caller reads devices, passes
/// each event in, and writes the events the session appends to the virtual
/// keyboard.
///
/// # Examples
///
/// ```
/// use escapepod_core::{DeviceId, Instant, KeyCode, KeyEvent, Rules, Session};
///
/// let mut session = Session::default();
/// session.add_device(DeviceId(7), Rules::default());
/// let mut out = Vec::new();
/// session.on_key(DeviceId(7), KeyEvent::press(KeyCode::LEFTCTRL), Instant::default(), &mut out);
/// out.clear();
/// session.remove_device(DeviceId(7), &mut out);
/// assert_eq!(out, [KeyEvent::release(KeyCode::LEFTCTRL)]);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Session {
    engines: BTreeMap<DeviceId, Engine>,
    pointers: BTreeMap<DeviceId, Rules>,
    buttons_down: BTreeSet<(DeviceId, KeyCode)>,
    chord: Chord,
    ledger: HeldKeys,
    scratch: Vec<KeyEvent>,
}

impl Session {
    /// A session with no device and the emergency chord `chord`.
    #[must_use]
    pub fn new(chord: Chord) -> Self {
        Self {
            engines: BTreeMap::new(),
            pointers: BTreeMap::new(),
            buttons_down: BTreeSet::new(),
            chord,
            ledger: HeldKeys::default(),
            scratch: Vec::new(),
        }
    }

    /// Adds device `dev` with `rules`. A device already present keeps its
    /// keys; use [`Session::replace_rules`] to change its rules.
    pub fn add_device(&mut self, dev: DeviceId, rules: Rules) {
        self.engines
            .entry(dev)
            .or_insert_with(|| Engine::new(rules));
    }

    /// Whether `dev` is in the session.
    #[must_use]
    pub fn has_device(&self, dev: DeviceId) -> bool {
        self.engines.contains_key(&dev)
    }

    /// The devices in the session, in id order.
    pub fn devices(&self) -> impl Iterator<Item = DeviceId> + '_ {
        self.engines.keys().copied()
    }

    /// The rules of `dev`, if it is in the session.
    #[must_use]
    pub fn rules(&self, dev: DeviceId) -> Option<&Rules> {
        self.engines.get(&dev).map(Engine::rules)
    }

    /// Consumes one key event from `dev` at `now` and appends the events to
    /// write to the virtual keyboard.
    pub fn on_key(
        &mut self,
        dev: DeviceId,
        ev: KeyEvent,
        now: Instant,
        out: &mut Vec<KeyEvent>,
    ) -> Step {
        if self.chord.observe(ev) {
            self.release_all(out);
            return Step {
                decision: None,
                emergency: true,
            };
        }
        let Some(engine) = self.engines.get_mut(&dev) else {
            return Step {
                decision: None,
                emergency: false,
            };
        };
        self.scratch.clear();
        let decision = engine.step(Input::Key(ev), now, &mut self.scratch);
        if ev.value == KeyValue::Press {
            for (other, e) in &mut self.engines {
                if *other != dev {
                    e.step(Input::ForeignPress, now, &mut self.scratch);
                }
            }
        }
        for &o in &self.scratch {
            if self.ledger.apply(dev, o) {
                out.push(o);
            }
        }
        Step {
            decision,
            emergency: false,
        }
    }

    /// Records a pointer button press on any pointer device.
    pub fn on_pointer_press(&mut self, now: Instant) {
        self.scratch.clear();
        for e in self.engines.values_mut() {
            e.step(Input::PointerPress, now, &mut self.scratch);
        }
    }

    /// Registers a grabbed pointer and its button rules.
    pub fn add_pointer(&mut self, dev: DeviceId, rules: Rules) {
        self.pointers.entry(dev).or_insert(rules);
    }

    /// Handles a pointer button and returns whether a rule consumed it.
    pub fn on_button(
        &mut self,
        dev: DeviceId,
        ev: KeyEvent,
        now: Instant,
        out: &mut Vec<KeyEvent>,
    ) -> bool {
        if ev.value == KeyValue::Press {
            self.on_pointer_press(now);
        }
        let Some(keys) = self
            .pointers
            .get(&dev)
            .and_then(|rules| rules.button(ev.key))
        else {
            return false;
        };
        let holder = Holder::Button(dev, ev.key);
        match ev.value {
            KeyValue::Press if self.buttons_down.insert((dev, ev.key)) => {
                for &key in keys {
                    let event = KeyEvent::press(key);
                    if self.ledger.apply_holder(holder, event) {
                        out.push(event);
                    }
                }
            }
            KeyValue::Release if self.buttons_down.remove(&(dev, ev.key)) => {
                for &key in keys.iter().rev() {
                    let event = KeyEvent::release(key);
                    if self.ledger.apply_holder(holder, event) {
                        out.push(event);
                    }
                }
            }
            _ => {}
        }
        true
    }

    /// Removes `dev`, appending the releases of every key it held.
    pub fn remove_device(&mut self, dev: DeviceId, out: &mut Vec<KeyEvent>) {
        self.engines.remove(&dev);
        let buttons: Vec<KeyCode> = self
            .buttons_down
            .iter()
            .filter(|(id, _)| *id == dev)
            .map(|(_, key)| *key)
            .collect();
        for button in buttons {
            self.on_button(dev, KeyEvent::release(button), Instant::default(), out);
        }
        self.pointers.remove(&dev);
        self.ledger.release_device(dev, out);
        self.chord.clear();
    }

    /// Gives `dev` new rules: its keys are released first, then it gets a
    /// fresh engine. The device stays in the session (its grab is kept).
    pub fn replace_rules(&mut self, dev: DeviceId, rules: Rules, out: &mut Vec<KeyEvent>) {
        self.ledger.release_device(dev, out);
        self.engines.insert(dev, Engine::new(rules));
    }

    /// Replaces the emergency chord.
    pub fn set_chord(&mut self, chord: Chord) {
        self.chord = chord;
    }

    /// Releases every held key, appending one release per key.
    pub fn release_all(&mut self, out: &mut Vec<KeyEvent>) {
        self.ledger.release_all(out);
        self.buttons_down.clear();
    }

    /// The held-keys ledger.
    #[must_use]
    pub fn held(&self) -> &HeldKeys {
        &self.ledger
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: DeviceId = DeviceId(1);
    const B: DeviceId = DeviceId(2);
    const CAPS: KeyCode = KeyCode::CAPSLOCK;
    const CTRL: KeyCode = KeyCode::LEFTCTRL;
    const ESC: KeyCode = KeyCode::ESC;

    fn key(name: &str) -> KeyCode {
        KeyCode::from_name(name).unwrap_or(ESC)
    }

    fn caps() -> Rules {
        Rules::builder()
            .dual_role(CAPS, CTRL, ESC)
            .build()
            .unwrap_or_default()
    }

    fn t(ms: u64) -> Instant {
        Instant::from_micros(ms * 1000)
    }

    #[test]
    fn two_devices_holding_one_key_emit_it_once() {
        let mut s = Session::default();
        s.add_device(A, Rules::default());
        s.add_device(B, Rules::default());
        let mut out = Vec::new();
        s.on_key(A, KeyEvent::press(CTRL), t(0), &mut out);
        s.on_key(B, KeyEvent::press(CTRL), t(1), &mut out);
        s.on_key(A, KeyEvent::release(CTRL), t(2), &mut out);
        assert_eq!(out, [KeyEvent::press(CTRL)]);
        s.on_key(B, KeyEvent::release(CTRL), t(3), &mut out);
        assert_eq!(out, [KeyEvent::press(CTRL), KeyEvent::release(CTRL)]);
    }

    #[test]
    fn a_key_on_another_keyboard_uses_the_dual_role_key() {
        let mut s = Session::default();
        s.add_device(A, caps());
        s.add_device(B, Rules::default());
        let mut out = Vec::new();
        s.on_key(A, KeyEvent::press(CAPS), t(0), &mut out);
        s.on_key(B, KeyEvent::press(key("KEY_C")), t(10), &mut out);
        s.on_key(B, KeyEvent::release(key("KEY_C")), t(20), &mut out);
        s.on_key(A, KeyEvent::release(CAPS), t(30), &mut out);
        assert!(!out.contains(&KeyEvent::press(ESC)));
    }

    #[test]
    fn pointer_press_uses_the_dual_role_key() {
        let mut s = Session::default();
        s.add_device(A, caps());
        let mut out = Vec::new();
        s.on_key(A, KeyEvent::press(CAPS), t(0), &mut out);
        s.on_pointer_press(t(10));
        s.on_key(A, KeyEvent::release(CAPS), t(30), &mut out);
        assert_eq!(out, [KeyEvent::press(CTRL), KeyEvent::release(CTRL)]);
    }

    #[test]
    // Verifies: ESC-SRS-105, ESC-SRS-106
    fn overlapping_buttons_hold_shared_keys_until_the_last_release() {
        let first = key("BTN_SIDE");
        let second = key("BTN_EXTRA");
        let right = key("KEY_RIGHT");
        let left = key("KEY_LEFT");
        let rules = Rules::builder()
            .button(first, vec![CTRL, right])
            .button(second, vec![CTRL, left])
            .build()
            .unwrap();
        let mut s = Session::default();
        s.add_pointer(A, rules);
        let mut out = Vec::new();
        assert!(s.on_button(A, KeyEvent::press(first), t(0), &mut out));
        assert!(s.on_button(A, KeyEvent::press(second), t(1), &mut out));
        assert!(s.on_button(A, KeyEvent::release(first), t(2), &mut out));
        assert_eq!(
            out,
            [
                KeyEvent::press(CTRL),
                KeyEvent::press(right),
                KeyEvent::press(left),
                KeyEvent::release(right)
            ]
        );
        s.remove_device(A, &mut out);
        assert_eq!(
            out,
            [
                KeyEvent::press(CTRL),
                KeyEvent::press(right),
                KeyEvent::press(left),
                KeyEvent::release(right),
                KeyEvent::release(left),
                KeyEvent::release(CTRL)
            ]
        );
        assert!(s.held().is_empty());
    }

    #[test]
    fn button_shortcuts_reject_duplicates_and_invalid_lengths() {
        let button = key("BTN_SIDE");
        Rules::builder().button(button, vec![]).build().unwrap_err();
        Rules::builder()
            .button(button, vec![CTRL; 9])
            .build()
            .unwrap_err();
        Rules::builder()
            .button(button, vec![CTRL, CTRL])
            .build()
            .unwrap_err();
        Rules::builder()
            .button(button, vec![CTRL])
            .button(button, vec![ESC])
            .build()
            .unwrap_err();
    }

    #[test]
    fn keyboard_and_button_share_modifier_until_both_release() {
        let button = key("BTN_SIDE");
        let right = key("KEY_RIGHT");
        let rules = Rules::builder()
            .button(button, vec![CTRL, right])
            .build()
            .unwrap();
        let mut s = Session::default();
        s.add_device(A, Rules::default());
        s.add_pointer(B, rules);
        let mut out = Vec::new();
        s.on_key(A, KeyEvent::press(CTRL), t(0), &mut out);
        s.on_button(B, KeyEvent::press(button), t(1), &mut out);
        s.on_key(A, KeyEvent::release(CTRL), t(2), &mut out);
        s.on_button(B, KeyEvent::release(button), t(3), &mut out);
        assert_eq!(
            out,
            [
                KeyEvent::press(CTRL),
                KeyEvent::press(right),
                KeyEvent::release(right),
                KeyEvent::release(CTRL)
            ]
        );
        assert!(s.held().is_empty());
    }

    #[test]
    fn release_all_clears_buttons_and_ignores_late_releases() {
        let button = key("BTN_SIDE");
        let rules = Rules::builder()
            .button(button, vec![CTRL, ESC])
            .build()
            .unwrap();
        let mut s = Session::default();
        s.add_pointer(A, rules);
        let mut out = Vec::new();
        s.on_button(A, KeyEvent::press(button), t(0), &mut out);
        s.release_all(&mut out);
        let count = out.len();
        s.on_button(A, KeyEvent::release(button), t(1), &mut out);
        assert_eq!(out.len(), count);
        assert!(s.held().is_empty());
    }

    #[test]
    fn removing_a_device_releases_only_its_keys() {
        let mut s = Session::default();
        s.add_device(A, caps());
        s.add_device(B, Rules::default());
        let mut out = Vec::new();
        s.on_key(A, KeyEvent::press(CAPS), t(0), &mut out);
        s.on_key(B, KeyEvent::press(key("KEY_X")), t(1), &mut out);
        out.clear();
        s.remove_device(A, &mut out);
        assert_eq!(out, [KeyEvent::release(CTRL)]);
        assert_eq!(s.held().keys().collect::<Vec<_>>(), [key("KEY_X")]);
        // The physical release arriving after removal emits nothing.
        out.clear();
        s.on_key(A, KeyEvent::release(CAPS), t(5), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn replacing_rules_releases_then_applies() {
        let mut s = Session::default();
        s.add_device(A, caps());
        let mut out = Vec::new();
        s.on_key(A, KeyEvent::press(CAPS), t(0), &mut out);
        out.clear();
        let remap = Rules::builder()
            .remap(CAPS, key("KEY_LEFTMETA"))
            .build()
            .unwrap_or_default();
        s.replace_rules(A, remap, &mut out);
        assert_eq!(out, [KeyEvent::release(CTRL)]);
        out.clear();
        s.on_key(A, KeyEvent::release(CAPS), t(10), &mut out);
        assert!(
            out.is_empty(),
            "the stale release must not reach uinput: {out:?}"
        );
        s.on_key(A, KeyEvent::press(CAPS), t(20), &mut out);
        assert_eq!(out, [KeyEvent::press(key("KEY_LEFTMETA"))]);
        assert!(s.has_device(A));
    }

    #[test]
    fn emergency_chord_releases_everything() {
        let mut s = Session::default();
        s.add_device(A, caps());
        let mut out = Vec::new();
        s.on_key(A, KeyEvent::press(CAPS), t(0), &mut out);
        s.on_key(A, KeyEvent::press(KeyCode::BACKSPACE), t(1), &mut out);
        s.on_key(A, KeyEvent::press(ESC), t(2), &mut out);
        out.clear();
        let step = s.on_key(A, KeyEvent::press(KeyCode::ENTER), t(3), &mut out);
        assert!(step.emergency);
        assert!(s.held().is_empty());
        assert_eq!(out.len(), 3, "{out:?}");
        assert!(out.iter().all(|e| e.value == KeyValue::Release));
    }

    #[test]
    fn repeats_only_reach_uinput_from_a_holder() {
        let mut h = HeldKeys::default();
        assert!(!h.apply(A, KeyEvent::repeat(CTRL)));
        assert!(h.apply(A, KeyEvent::press(CTRL)));
        assert!(h.apply(A, KeyEvent::repeat(CTRL)));
        assert!(!h.apply(B, KeyEvent::repeat(CTRL)));
        assert!(!h.apply(B, KeyEvent::release(CTRL)));
        let mut out = Vec::new();
        h.release_all(&mut out);
        assert_eq!(out, [KeyEvent::release(CTRL)]);
    }
}

// Rust guideline compliant 2026-05-18
