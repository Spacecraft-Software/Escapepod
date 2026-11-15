// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Property-based tests of the engine over generated balanced sequences.

mod common;

use std::collections::BTreeMap;

use common::{action, inputs, rules};
use escapepod_core::{
    DeviceId, Direction, Engine, Input, Instant, KeyEvent, KeyValue, Resolution, Rules, Session,
    TouchInput,
};
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

    #[test]
    fn interleaved_button_shortcuts_are_balanced(actions in proptest::collection::vec((0u8..3, any::<bool>()), 0..80)) {
        let ctrl = common::key("KEY_LEFTCTRL");
        let right = common::key("KEY_RIGHT");
        let left = common::key("KEY_LEFT");
        let side = common::key("BTN_SIDE");
        let extra = common::key("BTN_EXTRA");
        let rules = Rules::builder().button(side, vec![ctrl, right]).button(extra, vec![ctrl, left]).build().unwrap();
        let mut session = Session::default();
        session.add_device(DeviceId(1), Rules::default());
        session.add_pointer(DeviceId(2), rules);
        let mut out = Vec::new();
        for (which, press) in actions {
            let key = match which { 0 => ctrl, 1 => side, _ => extra };
            let ev = if press { KeyEvent::press(key) } else { KeyEvent::release(key) };
            if which == 0 { session.on_key(DeviceId(1), ev, Instant::default(), &mut out); }
            else { session.on_button(DeviceId(2), ev, Instant::default(), &mut out); }
        }
        session.release_all(&mut out);
        let mut depth = BTreeMap::new();
        for ev in out {
            let d = depth.entry(ev.key).or_insert(0i32);
            match ev.value { KeyValue::Press => *d += 1, KeyValue::Release => *d -= 1, KeyValue::Repeat => {} }
            prop_assert!((0..=1).contains(d));
        }
        prop_assert!(depth.values().all(|&d| d == 0));
    }

    // Verifies: ESC-SRS-119, ESC-SRS-120, ESC-SRS-121
    /// Swipes interleaved with a keyboard holding Control and with random
    /// touchpad noise never leave a key down, never press a held key twice
    /// and send at most one shortcut per swipe.
    #[test]
    fn interleaved_swipes_are_balanced(actions in proptest::collection::vec((0u8..4, -400i32..400, any::<bool>()), 0..120)) {
        let ctrl = common::key("KEY_LEFTCTRL");
        let meta = common::key("KEY_LEFTMETA");
        let right = common::key("KEY_RIGHT");
        let left = common::key("KEY_LEFT");
        let rules = Rules::builder()
            .gesture(3, Direction::Left, vec![meta, ctrl, right], None)
            .gesture(4, Direction::Right, vec![meta, ctrl, left], Some(5))
            .build()
            .unwrap();
        let mut session = Session::default();
        session.add_device(DeviceId(1), Rules::default());
        session.add_touchpad(DeviceId(2), rules, Resolution::new(10, 10).unwrap());
        let mut out = Vec::new();
        let mut sent = 0usize;
        let mut ends = 0usize;
        for (which, amount, flag) in actions {
            match which {
                0 => {
                    let ev = if flag { KeyEvent::press(ctrl) } else { KeyEvent::release(ctrl) };
                    session.on_key(DeviceId(1), ev, Instant::default(), &mut out);
                }
                1 => {
                    let fingers = u8::try_from(amount.unsigned_abs() % 6).unwrap();
                    let before = out.len();
                    if let Some(d) = session.on_touch(DeviceId(2), TouchInput::Fingers(fingers), Instant::default(), &mut out) {
                        ends += 1;
                        if matches!(d, escapepod_core::SwipeDecision::Sent { .. }) {
                            sent += 1;
                            prop_assert!(out.len() > before);
                        } else {
                            prop_assert_eq!(out.len(), before);
                        }
                    }
                }
                _ => {
                    let (dx, dy) = if flag { (amount, amount / 3) } else { (amount / 3, amount) };
                    let before = out.len();
                    session.on_touch(DeviceId(2), TouchInput::Motion { dx, dy }, Instant::default(), &mut out);
                    prop_assert_eq!(out.len(), before, "motion alone sends nothing");
                }
            }
        }
        prop_assert!(sent <= ends);
        session.release_all(&mut out);
        let mut depth = BTreeMap::new();
        for ev in out {
            let d = depth.entry(ev.key).or_insert(0i32);
            match ev.value { KeyValue::Press => *d += 1, KeyValue::Release => *d -= 1, KeyValue::Repeat => {} }
            prop_assert!((0..=1).contains(d));
        }
        prop_assert!(depth.values().all(|&d| d == 0));
    }

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
