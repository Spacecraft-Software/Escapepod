// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The records `event monitor` and the status socket write: one per event,
//! with unmanaged keys redacted to `other`.

use escapepod_core::{Decision, KeyCode, KeyEvent, KeyValue, Rules};
use serde_json::{Value, json};

use crate::output::Mode;
use crate::theme::Token;

/// One observed key event, already redacted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// ISO 8601 UTC time of the event.
    pub timestamp: String,
    /// The device node's name, such as `event3`.
    pub device: String,
    /// The key: a managed key's name, `other` for any other key unless
    /// `--all-keys` is on.
    pub key: String,
    /// `press`, `release` or `repeat`.
    pub value: &'static str,
    /// The engine's decision, for managed keys.
    pub decision: Option<String>,
}

fn value_name(v: KeyValue) -> &'static str {
    match v {
        KeyValue::Press => "press",
        KeyValue::Release => "release",
        KeyValue::Repeat => "repeat",
    }
}

/// The decision's short name: `hold`, `tap`, `used`, `timeout`, `remap`.
pub fn decision_name(d: Decision) -> String {
    match d {
        Decision::Hold(_) => "hold".into(),
        Decision::Tap(r) => format!("tap {}", r.tap),
        Decision::Used(_) => "used".into(),
        Decision::TimedOut(_) => "timeout".into(),
        Decision::Remap { target, .. } => format!("remap {target}"),
    }
}

/// Builds the record for `ev`. An unmanaged key is `other` unless
/// `all_keys`; its key code never leaves through this record otherwise.
/// Implements: ESC-SRS-060, ESC-SRS-061
pub fn record(
    rules: &Rules,
    device: &str,
    ev: KeyEvent,
    decision: Option<&Decision>,
    all_keys: bool,
    timestamp: String,
) -> Record {
    let managed = rules.is_managed(ev.key);
    Record {
        timestamp,
        device: device.to_owned(),
        key: if managed || all_keys {
            key_name(ev.key)
        } else {
            "other".into()
        },
        value: value_name(ev.value),
        decision: if managed {
            decision.copied().map(decision_name)
        } else {
            None
        },
    }
}

fn key_name(k: KeyCode) -> String {
    k.to_string()
}

impl Record {
    /// The record as one JSON object (one line in `jsonl` and on the socket).
    pub fn to_json(&self) -> Value {
        let mut v = json!({ "timestamp": self.timestamp, "device": self.device, "key": self.key, "value": self.value });
        if let (Some(d), Some(o)) = (&self.decision, v.as_object_mut()) {
            o.insert("decision".into(), d.clone().into());
        }
        v
    }

    /// The record as one complete human line. Output is append-only: no
    /// cursor movement, ever, so a screen reader reads it in order; in
    /// accessible mode the fields are also labelled. Implements: ESC-SRS-040
    pub fn line(&self, mode: &Mode) -> String {
        let decision = self.decision.as_deref().unwrap_or("-");
        if mode.accessible {
            format!(
                "{}: device {}, key {}, {}, decision {}\n",
                self.timestamp, self.device, self.key, self.value, decision
            )
        } else {
            let key = if self.key == "other" {
                self.key.clone()
            } else {
                mode.paint(Token::Accent, &self.key)
            };
            format!(
                "{}  {:<8}  {}  {:<7}  {}\n",
                self.timestamp, self.device, key, self.value, decision
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{Env, Flags};
    use escapepod_core::{Engine, Input, Instant};

    fn rules() -> Rules {
        Rules::builder()
            .dual_role(KeyCode::CAPSLOCK, KeyCode::LEFTCTRL, KeyCode::ESC)
            .build()
            .unwrap()
    }

    fn records(all_keys: bool) -> Vec<Record> {
        let r = rules();
        let mut e = Engine::new(r.clone());
        let a = KeyCode::from_name("KEY_A").unwrap();
        let mut out = Vec::new();
        let evs = [
            KeyEvent::press(a),
            KeyEvent::release(a),
            KeyEvent::press(KeyCode::CAPSLOCK),
            KeyEvent::release(KeyCode::CAPSLOCK),
        ];
        evs.iter()
            .enumerate()
            .map(|(i, ev)| {
                let d = e.step(
                    Input::Key(*ev),
                    Instant::from_micros(i as u64 * 1000),
                    &mut out,
                );
                record(
                    &r,
                    "event3",
                    *ev,
                    d.as_ref(),
                    all_keys,
                    "2026-11-01T00:00:00Z".into(),
                )
            })
            .collect()
    }

    // Verifies: ESC-SRS-061
    #[test]
    fn unmanaged_keys_are_other_without_a_code() {
        let recs = records(false);
        assert_eq!(recs[0].key, "other");
        assert_eq!(recs[0].decision, None);
        assert!(!recs[0].to_json().to_string().contains("KEY_A"));
        assert_eq!(recs[3].key, "KEY_CAPSLOCK");
        assert_eq!(recs[3].decision.as_deref(), Some("tap KEY_ESC"));
        assert_eq!(records(true)[0].key, "KEY_A");
    }

    // Verifies: ESC-SRS-040
    #[test]
    fn accessible_output_is_one_line_per_event_without_cursor_movement() {
        let env = Env::tty(&[("SPACECRAFT_A11Y", "1"), ("FORCE_COLOR", "1")]);
        let (mode, _) =
            Mode::resolve(&Flags::default(), &env, "escapepod event monitor".into()).unwrap();
        assert!(mode.accessible);
        for rec in records(false) {
            let line = rec.line(&mode);
            assert_eq!(line.matches('\n').count(), 1, "{line:?}");
            assert!(line.ends_with('\n'));
            assert!(!line.contains('\r'), "{line:?}");
            // CSI sequences other than SGR colour (final byte `m`) move the
            // cursor or erase; none may appear.
            let mut rest = line.as_str();
            while let Some(i) = rest.find("\x1b[") {
                let tail = &rest[i + 2..];
                let end = tail.find(|c: char| c.is_ascii_alphabetic()).unwrap();
                assert_eq!(&tail[end..=end], "m", "{line:?}");
                rest = &tail[end + 1..];
            }
            assert!(line.contains("device event3"));
        }
    }
}

// Rust guideline compliant 2026-05-18
