// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The X11 backend under Xvfb (feature `x11-tests`): a private X server on
//! a free display, never the desktop's. The test types through XTEST and
//! watches every key event through its own RECORD context.

#![cfg(feature = "x11-tests")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly by design"
)]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::record::{self, ConnectionExt as _};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::wrapper::ConnectionExt as _;

const CTRL: u8 = 37;
const ESC: u8 = 9;
const C: u8 = 54;

struct Xvfb {
    child: Child,
    display: String,
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts Xvfb on the first free display from :87. `None` (or a failure
/// under `ESCAPEPOD_X11_REQUIRE`) when Xvfb is not installed.
fn xvfb() -> Option<Xvfb> {
    let n = (87..120).find(|n| !Path::new(&format!("/tmp/.X11-unix/X{n}")).exists())?;
    let display = format!(":{n}");
    let child = Command::new("Xvfb")
        .args([
            display.as_str(),
            "-nolisten",
            "tcp",
            "-screen",
            "0",
            "320x240x24",
        ])
        .stderr(Stdio::null())
        .spawn();
    let Ok(child) = child else {
        assert!(
            std::env::var_os("ESCAPEPOD_X11_REQUIRE").is_none(),
            "Xvfb is not installed"
        );
        eprintln!("SKIPPED: Xvfb is not installed");
        return None;
    };
    let x = Xvfb { child, display };
    let start = Instant::now();
    while x11rb::connect(Some(&x.display)).is_err() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Xvfb did not start"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    Some(x)
}

/// A recorder thread: every core key event on `display` as (keycode, press).
fn record_keys(display: &str) -> mpsc::Receiver<(u8, bool)> {
    let (tx, rx) = mpsc::channel();
    let display = display.to_owned();
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (ctl, _) = x11rb::connect(Some(&display)).unwrap();
        let (data, _) = x11rb::connect(Some(&display)).unwrap();
        let ctx = ctl.generate_id().unwrap();
        let range = record::Range {
            device_events: record::Range8 { first: 2, last: 3 },
            ..record::Range::default()
        };
        ctl.record_create_context(ctx, 0, &[record::CS::ALL_CLIENTS.into()], &[range])
            .unwrap()
            .check()
            .unwrap();
        let cookie = data.record_enable_context(ctx).unwrap();
        for reply in cookie {
            let Ok(reply) = reply else { return };
            if reply.category == 4 {
                let _ = ready_tx.send(()); // StartOfData
            }
            if reply.category != 0 {
                continue;
            }
            for e in reply.data.chunks_exact(32) {
                let kind = e[0] & 0x7f;
                if (kind == 2 || kind == 3) && tx.send((e[1], kind == 2)).is_err() {
                    return;
                }
            }
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    rx
}

fn type_keys(display: &str, keys: &[(u8, bool)]) {
    let (c, screen) = x11rb::connect(Some(display)).unwrap();
    let root = c.setup().roots[screen].root;
    for &(code, press) in keys {
        c.xtest_fake_input(if press { 2 } else { 3 }, code, 0, root, 0, 0, 0)
            .unwrap();
        c.flush().unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    c.sync().unwrap();
    // Stay connected a moment: when an XTEST client disconnects mid-sequence
    // Xvfb can drop an injection another client makes at the same instant.
    std::thread::sleep(Duration::from_millis(200));
}

fn drain(rx: &mpsc::Receiver<(u8, bool)>, wait: Duration) -> Vec<(u8, bool)> {
    let deadline = Instant::now() + wait;
    let mut got = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(e) => got.push(e),
            Err(_) => break,
        }
    }
    got
}

/// The daemon on `display`, with no device access at all: the uinput probe
/// and every device directory point at nothing.
fn daemon(display: &str, backend: &str, root: &Path) -> Child {
    std::fs::create_dir_all(root.join("sys")).unwrap();
    std::fs::create_dir_all(root.join("dev")).unwrap();
    Command::new(env!("CARGO_BIN_EXE_escapepod"))
        .args([
            "daemon",
            "run",
            "--yes",
            "--verbose",
            "--map",
            "Control_L=Escape",
            "--backend",
            backend,
        ])
        .env_clear()
        .env("DISPLAY", display)
        .env("HOME", root)
        .env("ESCAPEPOD_UINPUT", root.join("no-uinput"))
        .env("ESCAPEPOD_SYSFS", root.join("sys"))
        .env("ESCAPEPOD_DEV_DIR", root.join("dev"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// Waits until the daemon reports its backend on standard error, then a
/// moment more for its RECORD context to start; returns the rest of its
/// standard error as it arrives.
fn wait_ready(d: &mut Child) -> mpsc::Receiver<String> {
    let err = d.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufRead::lines(std::io::BufReader::new(err)).map_while(Result::ok) {
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    let start = Instant::now();
    while let Ok(line) = rx.recv_timeout(Duration::from_secs(5)) {
        if line.contains("BACKEND_SELECTED") && line.contains("X11 backend on display") {
            std::thread::sleep(Duration::from_millis(300));
            return rx;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
    }
    panic!("the daemon never reported the X11 backend");
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("escapepod-x11-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// Verifies: ESC-SRS-063, ESC-SRS-065, ESC-SRS-067, ESC-SRS-088
#[test]
fn auto_selects_x11_and_injects_taps_only() {
    let Some(x) = xvfb() else { return };
    let root = scratch("tap");
    let rx = record_keys(&x.display);
    let mut d = daemon(&x.display, "auto", &root);
    let lines = wait_ready(&mut d);
    assert!(
        d.try_wait().unwrap().is_none(),
        "the daemon runs without any device access"
    );
    // A lone tap of Control_L: the daemon adds Escape, nothing else.
    type_keys(&x.display, &[(CTRL, true), (CTRL, false)]);
    assert_eq!(
        drain(&rx, Duration::from_millis(500)),
        [(CTRL, true), (CTRL, false), (ESC, true), (ESC, false)]
    );
    // Control-C: no Escape.
    type_keys(
        &x.display,
        &[(CTRL, true), (C, true), (C, false), (CTRL, false)],
    );
    assert_eq!(
        drain(&rx, Duration::from_millis(500)),
        [(CTRL, true), (C, true), (C, false), (CTRL, false)]
    );
    let _ = d.kill();
    let _ = d.wait();
    let rest: Vec<String> = lines.try_iter().collect();
    assert!(rest.iter().all(|l| !l.contains("\"error\"")), "{rest:?}");
}

// Verifies: ESC-SRS-068, ESC-SRS-102
#[test]
fn losing_the_display_exits_1_naming_it() {
    let Some(x) = xvfb() else { return };
    let root = scratch("lost");
    let display = x.display.clone();
    let mut d = daemon(&display, "x11", &root);
    let lines = wait_ready(&mut d);
    drop(x);
    let start = Instant::now();
    let status = d.wait().unwrap();
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(status.code(), Some(1));
    let err: Vec<String> =
        std::iter::from_fn(|| lines.recv_timeout(Duration::from_secs(1)).ok()).collect();
    let line = err
        .iter()
        .find(|l| l.contains("DISPLAY_LOST"))
        .unwrap_or_else(|| panic!("{err:?}"));
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v["error"]["display"], display.as_str());
    assert!(v["error"]["message"].as_str().unwrap().contains(&display));
}
