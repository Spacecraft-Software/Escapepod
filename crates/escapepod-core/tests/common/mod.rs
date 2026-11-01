// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared generators for the property and differential tests: balanced
//! input sequences over a small key set with a dual-role key, a second
//! dual-role key, a remap, a protected key and plain keys.

#![allow(dead_code, reason = "each test binary uses a subset of the helpers")]

use escapepod_core::{Input, Instant, KeyCode, KeyEvent, Rules};
use proptest::prelude::*;

pub fn key(name: &str) -> KeyCode {
    KeyCode::from_name(name).unwrap_or(KeyCode::ESC)
}

/// The physical keys the generator presses.
pub fn keys() -> Vec<KeyCode> {
    [
        "KEY_CAPSLOCK",
        "KEY_ENTER",
        "KEY_TAB",
        "KEY_F12",
        "KEY_A",
        "KEY_B",
        "KEY_LEFTCTRL",
        "KEY_ESC",
    ]
    .iter()
    .map(|n| key(n))
    .collect()
}

/// Caps Lock: Control/Escape; Enter: Right Control/Enter; Tab remapped to
/// Left Meta; F12 protected.
pub fn rules() -> Rules {
    Rules::builder()
        .dual_role(key("KEY_CAPSLOCK"), key("KEY_LEFTCTRL"), key("KEY_ESC"))
        .dual_role(key("KEY_ENTER"), key("KEY_RIGHTCTRL"), key("KEY_ENTER"))
        .remap(key("KEY_TAB"), key("KEY_LEFTMETA"))
        .protect(key("KEY_F12"))
        .build()
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    /// Press the key if it is up, else release it.
    Toggle(usize),
    /// Autorepeat the key if it is down.
    Repeat(usize),
    Pointer,
    /// Let this many milliseconds pass.
    Wait(u64),
}

pub fn action() -> impl Strategy<Value = Action> {
    let n = keys().len();
    prop_oneof![
        6 => (0..n).prop_map(Action::Toggle),
        1 => (0..n).prop_map(Action::Repeat),
        1 => Just(Action::Pointer),
        2 => (0u64..400).prop_map(Action::Wait),
    ]
}

/// Turns actions into a timed input sequence in which every pressed key is
/// released by the end.
pub fn inputs(actions: &[Action]) -> Vec<(Input, Instant)> {
    let keys = keys();
    let mut down = vec![false; keys.len()];
    let mut now = 0u64;
    let mut seq = Vec::new();
    for &a in actions {
        match a {
            Action::Toggle(i) => {
                let ev = if down[i] {
                    KeyEvent::release(keys[i])
                } else {
                    KeyEvent::press(keys[i])
                };
                down[i] = !down[i];
                seq.push((Input::Key(ev), Instant::from_micros(now * 1000)));
            }
            Action::Repeat(i) if down[i] => {
                seq.push((
                    Input::Key(KeyEvent::repeat(keys[i])),
                    Instant::from_micros(now * 1000),
                ));
            }
            Action::Repeat(_) => {}
            Action::Pointer => seq.push((Input::PointerPress, Instant::from_micros(now * 1000))),
            Action::Wait(ms) => now += ms,
        }
    }
    for (i, d) in down.iter().enumerate() {
        if *d {
            seq.push((
                Input::Key(KeyEvent::release(keys[i])),
                Instant::from_micros(now * 1000),
            ));
        }
    }
    seq
}
