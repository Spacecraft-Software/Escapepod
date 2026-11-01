// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Differential test (Standard §21.4 independence): a reference model
//! written separately from the engine, straight from the requirement text,
//! must produce the same output on every generated sequence.
//!
//! The model keeps the whole input history and decides each dual-role
//! release by scanning back to its press, where the engine keeps a running
//! `used` flag. The two share only the key names and the rule table.

mod common;

use common::{action, inputs, rules};
use escapepod_core::{Engine, Input, Instant, KeyEvent, KeyValue, Role, Rules};
use proptest::prelude::*;

/// The reference model: ESC-SRS-009 to 017 and 020, read literally.
fn oracle(r: &Rules, seq: &[(Input, Instant)]) -> Vec<KeyEvent> {
    let mut out = Vec::new();
    for (n, &(input, now)) in seq.iter().enumerate() {
        let Input::Key(ev) = input else { continue };
        match r.role(ev.key) {
            Role::Protected | Role::Other => out.push(ev),
            Role::Remap(t) => out.push(KeyEvent {
                key: t,
                value: ev.value,
            }),
            Role::DualRole(d) => match ev.value {
                KeyValue::Press if !is_down_before(seq, n, ev.key) => {
                    out.push(KeyEvent::press(d.hold));
                }
                KeyValue::Press | KeyValue::Repeat => out.push(KeyEvent::repeat(d.hold)),
                KeyValue::Release => {
                    out.push(KeyEvent::release(d.hold));
                    // Find the press that began this hold.
                    let Some(p) = (0..n).rev().find(|&j| {
                        matches!(seq[j].0, Input::Key(e) if e.key == ev.key && e.value == KeyValue::Press)
                            && !is_down_before(seq, j, ev.key)
                    }) else { continue };
                    let other = seq[p + 1..n].iter().any(|(i, _)| match i {
                        Input::Key(e) => e.key != ev.key && e.value == KeyValue::Press,
                        Input::PointerPress | Input::ForeignPress => true,
                    });
                    let in_time = now.saturating_since(seq[p].1) <= r.tap_timeout();
                    if !other && in_time {
                        out.push(KeyEvent::press(d.tap));
                        out.push(KeyEvent::release(d.tap));
                    }
                }
            },
        }
    }
    out
}

/// Whether `key` is physically down just before input `n`.
fn is_down_before(seq: &[(Input, Instant)], n: usize, key: escapepod_core::KeyCode) -> bool {
    seq[..n]
        .iter()
        .rev()
        .find_map(|(i, _)| match i {
            Input::Key(e) if e.key == key && e.value == KeyValue::Press => Some(true),
            Input::Key(e) if e.key == key && e.value == KeyValue::Release => Some(false),
            _ => None,
        })
        .unwrap_or(false)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    // Verifies: ESC-SRS-009, ESC-SRS-010, ESC-SRS-011, ESC-SRS-012, ESC-SRS-013
    // Verifies: ESC-SRS-014, ESC-SRS-015, ESC-SRS-016, ESC-SRS-020
    #[test]
    fn engine_matches_the_reference_model(actions in proptest::collection::vec(action(), 0..80)) {
        let r = rules();
        let seq = inputs(&actions);
        let mut engine = Engine::new(r.clone());
        let mut got = Vec::new();
        for &(i, t) in &seq {
            engine.step(i, t, &mut got);
        }
        prop_assert_eq!(got, oracle(&r, &seq), "input {:?}", seq);
    }
}
