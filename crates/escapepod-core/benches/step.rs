// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The engine's share of the latency budget (ESC-SRS-079): nanoseconds per
//! `Session::on_key` over the reference workload's event mix (taps, Control
//! chords and plain keystrokes on one device).
//!
//! A std-only harness (`harness = false`): it reports the median of 31 runs
//! of 100 000 events each. Run with `cargo bench -p escapepod-core`.

use std::hint::black_box;
use std::time::Instant as Clock;

use escapepod_core::{DeviceId, Instant, KeyCode, KeyEvent, Rules, Session};

fn key(name: &str) -> KeyCode {
    KeyCode::from_name(name).unwrap_or(KeyCode::ESC)
}

/// One pass of the mix: a tap, a Control chord, and ten plain keystrokes.
fn workload() -> Vec<KeyEvent> {
    let caps = KeyCode::CAPSLOCK;
    let c = key("KEY_C");
    let mut evs = vec![KeyEvent::press(caps), KeyEvent::release(caps)];
    evs.extend([
        KeyEvent::press(caps),
        KeyEvent::press(c),
        KeyEvent::release(c),
        KeyEvent::release(caps),
    ]);
    for name in [
        "KEY_H",
        "KEY_E",
        "KEY_L",
        "KEY_L",
        "KEY_O",
        "KEY_SPACE",
        "KEY_W",
        "KEY_O",
        "KEY_R",
        "KEY_D",
    ] {
        evs.extend([KeyEvent::press(key(name)), KeyEvent::release(key(name))]);
    }
    evs
}

fn main() {
    let rules = Rules::builder()
        .dual_role(KeyCode::CAPSLOCK, KeyCode::LEFTCTRL, KeyCode::ESC)
        .build()
        .unwrap_or_default();
    let mix = workload();
    let mut session = Session::default();
    session.add_device(DeviceId(0), rules);
    let mut out = Vec::with_capacity(16);
    let events = 100_000usize;
    let mut samples = Vec::new();
    for _ in 0..31 {
        let start = Clock::now();
        let mut t = 0u64;
        for ev in mix.iter().cycle().take(events) {
            out.clear();
            t += 20_000;
            black_box(session.on_key(DeviceId(0), *ev, Instant::from_micros(t), &mut out));
        }
        samples.push(start.elapsed().as_nanos() / events as u128);
    }
    samples.sort_unstable();
    println!(
        "step: median {} ns per event over {} runs of {events} events",
        samples[15],
        samples.len()
    );
}
