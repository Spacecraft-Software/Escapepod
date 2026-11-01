// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Property-based tests of the engine over generated balanced sequences.

mod common;

use std::collections::BTreeMap;

use common::{action, inputs, rules};
use escapepod_core::{Engine, Input, KeyEvent, KeyValue};
use proptest::prelude::*;

fn run(seq: &[(Input, escapepod_core::Instant)]) -> Vec<KeyEvent> {
    let mut engine = Engine::new(rules());
    let mut out = Vec::new();
    for &(i, t) in seq {
        engine.step(i, t, &mut out);
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    // Verifies: ESC-SRS-018
    /// Every key press the engine emits is followed by a release, and no
    /// key is released more often than it was pressed.
    #[test]
    fn output_is_balanced(actions in proptest::collection::vec(action(), 0..80)) {
        let out = run(&inputs(&actions));
        let mut depth: BTreeMap<_, i64> = BTreeMap::new();
        for ev in &out {
            let d = depth.entry(ev.key).or_insert(0);
            match ev.value {
                KeyValue::Press => *d += 1,
                KeyValue::Release => *d -= 1,
                KeyValue::Repeat => prop_assert!(*d > 0, "repeat of {} while up", ev.key),
            }
            prop_assert!(*d >= 0, "{} released more than pressed in {out:?}", ev.key);
        }
        prop_assert!(depth.values().all(|&d| d == 0), "unbalanced: {depth:?}");
    }

    // Verifies: ESC-SRS-017
    /// The events of unmanaged keys come out unchanged and in input order.
    #[test]
    fn unmanaged_events_keep_their_order(actions in proptest::collection::vec(action(), 0..80)) {
        let r = rules();
        let seq = inputs(&actions);
        let unmanaged = |ev: &KeyEvent| !r.is_managed(ev.key) && r.role(ev.key) != escapepod_core::Role::Protected;
        let want: Vec<KeyEvent> = seq.iter().filter_map(|(i, _)| match i {
            Input::Key(ev) if unmanaged(ev) => Some(*ev),
            _ => None,
        }).collect();
        // The hold and tap keys of the generator's rules are also plain
        // physical keys (Left Control, Escape, Enter), so compare only the
        // keys no rule emits.
        let emitted = [common::key("KEY_LEFTCTRL"), common::key("KEY_ESC"), common::key("KEY_ENTER"),
                       common::key("KEY_RIGHTCTRL"), common::key("KEY_LEFTMETA")];
        let got: Vec<KeyEvent> = run(&seq).into_iter().filter(|e| unmanaged(e) && !emitted.contains(&e.key)).collect();
        let want: Vec<KeyEvent> = want.into_iter().filter(|e| !emitted.contains(&e.key)).collect();
        prop_assert_eq!(got, want);
    }
}
