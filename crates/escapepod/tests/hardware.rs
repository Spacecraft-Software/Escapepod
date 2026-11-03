// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! uinput loopback tests of the real evdev path (feature `hardware-tests`,
//! never part of the default `cargo test`). They need `/dev/uinput` and
//! read access to `/dev/input`.
//!
//! Safety on a live desktop: the daemon is told to grab only the fixture
//! keyboard (`grab = "listed"`, matched by its unique id), and no key is
//! sent until (1) this test holds the exclusive grab on the daemon's
//! virtual keyboard and (2) the daemon reports holding the grab on the
//! fixture. If another program (a remapper such as xremap) holds either
//! first, the test stops without sending anything, so no keystroke ever
//! reaches the desktop session.

#![cfg(feature = "hardware-tests")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly by design"
)]

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, Device, EventType, InputEvent, InputId, KeyCode};

const FIXTURE_VENDOR: u16 = 0x5350;
const FIXTURE_PRODUCT: u16 = 0x5445;

struct Rig {
    dir: PathBuf,
    config: PathBuf,
    daemon: Child,
    output: Option<Device>,
    fixture: Option<VirtualDevice>,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn wait_for<T>(limit: Duration, f: impl FnMut() -> Option<T>) -> Option<T> {
    wait_every(limit, Duration::from_millis(1), f)
}

fn wait_every<T>(limit: Duration, every: Duration, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Some(v) = f() {
            return Some(v);
        }
        std::thread::sleep(every);
    }
    None
}

/// The device node of the virtual keyboard daemon `pid` created.
fn output_node(pid: u32) -> Option<PathBuf> {
    let want = format!("Escape Pod virtual keyboard {pid}");
    std::fs::read_dir("/sys/class/input")
        .ok()?
        .flatten()
        .find_map(|e| {
            let name = std::fs::read_to_string(e.path().join("device/name")).ok()?;
            let node = e.file_name().to_string_lossy().into_owned();
            (name.trim_end() == want && node.starts_with("event"))
                .then(|| Path::new("/dev/input").join(node))
        })
}

fn status(socket: &Path) -> Option<serde_json::Value> {
    let s = UnixStream::connect(socket).ok()?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

fn fixture_grabbed(socket: &Path) -> bool {
    status(socket).is_some_and(|v| {
        v["devices"].as_array().is_some_and(|d| {
            d.iter()
                .any(|x| x["name"] == "escapepod-test-keyboard" && x["grabbed"] == true)
        })
    })
}

/// [`try_rig`], retried: on a desktop running another remapper the race
/// for the first grab is lost now and then, never in a way that sends a key.
/// With `ESCAPEPOD_HW_REQUIRE` set, a test that never got its rig fails
/// instead of passing without having run: that is how the evidence run is
/// made (`tools/ci.nu --only hardware`).
fn rig(name: &str, config: &str) -> Option<Rig> {
    assert_eq!(
        std::env::var_os("ESCAPEPOD_HW_ON_HOST").as_deref(),
        Some(std::ffi::OsStr::new("1")),
        "hardware tests require ESCAPEPOD_HW_ON_HOST=1; use the NixOS VM gate otherwise"
    );
    let rig = (0..8).find_map(|attempt| try_rig(&format!("{name}-{attempt}"), config));
    assert!(
        rig.is_some() || std::env::var_os("ESCAPEPOD_HW_REQUIRE").is_none(),
        "{name}: no attempt got both grabs; the test did not run"
    );
    rig
}

/// Starts a daemon, takes its virtual keyboard, plugs the fixture in and
/// waits for the daemon to grab it. `None` (with a reason printed) when the
/// machine is not suitable or another program got a grab first.
fn try_rig(name: &str, config: &str) -> Option<Rig> {
    if std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/uinput")
        .is_err()
    {
        eprintln!("SKIPPED {name}: no write access to /dev/uinput");
        return None;
    }
    let dir = std::env::temp_dir().join(format!("escapepod-hw-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("config.toml");
    std::fs::write(&cfg, config_text(config)).unwrap();
    let socket = dir.join("escapepod.sock");
    let daemon = Command::new(env!("CARGO_BIN_EXE_escapepod"))
        .args([
            "daemon",
            "run",
            "--backend",
            "evdev",
            "--yes",
            "--verbose",
            "--only-device",
            "5350:5445",
            "--config",
        ])
        .arg(&cfg)
        .env("ESCAPEPOD_SOCKET", &socket)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = daemon.id();
    let mut rig = Rig {
        dir,
        config: cfg,
        daemon,
        output: None,
        fixture: None,
    };
    let Some(node) = wait_for(Duration::from_secs(3), || output_node(pid)) else {
        eprintln!("SKIPPED {name}: the daemon created no virtual keyboard");
        return None;
    };
    let mut output = wait_for(Duration::from_secs(1), || Device::open(&node).ok())?;
    if output.grab().is_err() {
        eprintln!("SKIPPED {name}: another program grabbed the daemon's virtual keyboard first");
        return None;
    }
    output.set_nonblocking(true).unwrap();
    rig.output = Some(output);
    let mut keys = AttributeSet::<KeyCode>::new();
    for code in 1..0x100u16 {
        keys.insert(KeyCode::new(code));
    }
    let plugged = Instant::now();
    rig.fixture = Some(
        VirtualDevice::builder()
            .unwrap()
            .name("escapepod-test-keyboard")
            .input_id(InputId::new(
                BusType::BUS_USB,
                FIXTURE_VENDOR,
                FIXTURE_PRODUCT,
                1,
            ))
            .with_keys(&keys)
            .unwrap()
            .build()
            .unwrap(),
    );
    if wait_for(Duration::from_secs(2), || {
        fixture_grabbed(&socket).then_some(())
    })
    .is_none()
    {
        let alive = rig.daemon.try_wait();
        eprintln!(
            "daemon state before kill: {alive:?}; socket exists: {}",
            socket.exists()
        );
        let _ = rig.daemon.kill();
        let mut err = String::new();
        let _ = std::io::Read::read_to_string(rig.daemon.stderr.as_mut().unwrap(), &mut err);
        eprintln!(
            "SKIPPED {name}: the daemon did not hold the fixture's grab; status {:?}; daemon said:\n{err}",
            status(&socket)
        );
        return None;
    }
    let took = plugged.elapsed();
    // Verifies: ESC-SRS-051 (hotplug grab within 1 s, measured end to end)
    assert!(took < Duration::from_secs(1), "hotplug grab took {took:?}");
    Some(rig)
}

/// A configuration that grabs only the fixture, with `mapping` for it.
fn config_text(mapping: &str) -> String {
    format!(
        "grab = \"listed\"\n\n[[device]]\nid = \"{FIXTURE_VENDOR:04x}:{FIXTURE_PRODUCT:04x}\"\n{mapping}"
    )
}

fn signal(rig: &Rig, sig: nix::sys::signal::Signal) {
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(i32::try_from(rig.daemon.id()).unwrap()),
        sig,
    )
    .unwrap();
}

fn send(rig: &mut Rig, code: u16, value: i32) {
    let ev = InputEvent::new(EventType::KEY.0, code, value);
    rig.fixture.as_mut().unwrap().emit(&[ev]).unwrap();
}

/// Reads the daemon's output until `n` key events arrive or 1 s passes.
fn read_output(rig: &mut Rig, n: usize) -> Vec<(u16, i32)> {
    let mut got = Vec::new();
    let start = Instant::now();
    while got.len() < n && start.elapsed() < Duration::from_secs(1) {
        if let Ok(evs) = rig.output.as_mut().unwrap().fetch_events() {
            got.extend(
                evs.filter(|e| e.event_type() == EventType::KEY)
                    .map(|e| (e.code(), e.value())),
            );
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    got
}

const CAPS: u16 = 58;
const CTRL: u16 = 29;
const ESC: u16 = 1;
const BACKSPACE: u16 = 14;
const ENTER: u16 = 28;
const MAPPING: &str =
    "[[device.dual_role]]\nkey = \"Caps_Lock\"\nhold = \"Control_L\"\ntap = \"Escape\"\n";

// Verifies: ESC-SRS-043, ESC-SRS-044, ESC-SRS-045, ESC-SRS-046
#[test]
fn tap_hold_and_sigterm_release_end_to_end() {
    let Some(mut rig) = rig("tap", MAPPING) else {
        return;
    };
    send(&mut rig, CAPS, 1);
    send(&mut rig, CAPS, 0);
    assert_eq!(
        read_output(&mut rig, 4),
        [(CTRL, 1), (CTRL, 0), (ESC, 1), (ESC, 0)]
    );
    // Hold Caps Lock, then stop the daemon: Control must come up.
    send(&mut rig, CAPS, 1);
    assert_eq!(read_output(&mut rig, 1), [(CTRL, 1)]);
    let sent = Instant::now();
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(i32::try_from(rig.daemon.id()).unwrap()),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();
    assert_eq!(read_output(&mut rig, 1), [(CTRL, 0)]);
    assert!(
        sent.elapsed() < Duration::from_millis(100),
        "release took {:?}",
        sent.elapsed()
    );
    let status = rig.daemon.wait().unwrap();
    assert_eq!(status.code(), Some(0));
    // The daemon never listed its own virtual keyboard as a device.
    let mut err = String::new();
    std::io::Read::read_to_string(rig.daemon.stderr.as_mut().unwrap(), &mut err).unwrap();
    assert!(
        !err.contains("grabbed Escape Pod virtual keyboard"),
        "{err}"
    );
}

// Verifies: ESC-SRS-048, ESC-SRS-096
#[test]
fn emergency_chord_releases_and_exits_6() {
    let Some(mut rig) = rig("chord", MAPPING) else {
        return;
    };
    // Open the fixture's own node: once the daemon lets go, this test takes
    // the grab, which both measures the release and keeps any later event
    // of the fixture away from the desktop.
    let node = wait_for(Duration::from_secs(1), fixture_node).unwrap();
    let mut probe = Device::open(&node).unwrap();
    send(&mut rig, CAPS, 1);
    send(&mut rig, BACKSPACE, 1);
    send(&mut rig, ESC, 1);
    let pressed = Instant::now();
    send(&mut rig, ENTER, 1);
    // The virtual keyboard vanishes when the daemon exits, taking unread
    // events with it, so its releases are read first.
    let out = read_output(&mut rig, 6);
    wait_for(Duration::from_secs(1), || probe.grab().ok()).unwrap();
    let released = pressed.elapsed();
    assert!(
        released < Duration::from_millis(100),
        "grab release took {released:?}"
    );
    let status = wait_for(Duration::from_secs(2), || {
        rig.daemon.try_wait().ok().flatten()
    })
    .unwrap();
    assert_eq!(status.code(), Some(6));
    for code in [CTRL, BACKSPACE, ESC] {
        assert!(out.contains(&(code, 0)), "{code} released: {out:?}");
    }
    rig.fixture = None;
    drop(probe);
}

/// The fixture keyboard's device node.
fn fixture_node() -> Option<PathBuf> {
    std::fs::read_dir("/sys/class/input")
        .ok()?
        .flatten()
        .find_map(|e| {
            let name = std::fs::read_to_string(e.path().join("device/name")).ok()?;
            let node = e.file_name().to_string_lossy().into_owned();
            (name.trim_end() == "escapepod-test-keyboard" && node.starts_with("event"))
                .then(|| Path::new("/dev/input").join(node))
        })
}

// Verifies: ESC-SRS-052
#[test]
fn unplugging_releases_held_keys() {
    let Some(mut rig) = rig("unplug", MAPPING) else {
        return;
    };
    send(&mut rig, CAPS, 1);
    assert_eq!(read_output(&mut rig, 1), [(CTRL, 1)]);
    // Held past the tap timeout: the kernel's own release on unplug must
    // not tap (an unplug inside the timeout does: known issue, #11).
    std::thread::sleep(Duration::from_millis(300));
    rig.fixture = None;
    assert_eq!(read_output(&mut rig, 1), [(CTRL, 0)]);
    assert!(
        rig.daemon.try_wait().unwrap().is_none(),
        "the daemon keeps running"
    );
}

// Verifies: ESC-SRS-054, ESC-SRS-055, ESC-SRS-056, ESC-SRS-099
#[test]
fn sighup_reloads_and_keeps_the_grab() {
    let Some(mut rig) = rig("reload", MAPPING) else {
        return;
    };
    // A remap replaces the dual role; the fixture stays grabbed throughout,
    // which the status socket shows and the absence of leaks proves.
    std::fs::write(
        &rig.config,
        config_text("[[device.remap]]\nfrom = \"Caps_Lock\"\nto = \"Escape\"\n"),
    )
    .unwrap();
    signal(&rig, nix::sys::signal::Signal::SIGHUP);
    std::thread::sleep(Duration::from_millis(200));
    send(&mut rig, CAPS, 1);
    send(&mut rig, CAPS, 0);
    assert_eq!(read_output(&mut rig, 2), [(ESC, 1), (ESC, 0)]);
    // A broken file is refused and the remap stays.
    std::fs::write(&rig.config, "grab = \"listed\"\ntapp = 1\n").unwrap();
    signal(&rig, nix::sys::signal::Signal::SIGHUP);
    std::thread::sleep(Duration::from_millis(200));
    send(&mut rig, CAPS, 1);
    send(&mut rig, CAPS, 0);
    assert_eq!(read_output(&mut rig, 2), [(ESC, 1), (ESC, 0)]);
    signal(&rig, nix::sys::signal::Signal::SIGTERM);
    let _ = rig.daemon.wait();
    let mut err = String::new();
    std::io::Read::read_to_string(rig.daemon.stderr.as_mut().unwrap(), &mut err).unwrap();
    let failed = err
        .lines()
        .find(|l| l.contains("RELOAD_FAILED"))
        .unwrap_or_else(|| panic!("{err}"));
    assert!(
        failed.contains("\"severity\":\"warn\"") && failed.contains("tapp"),
        "{failed}"
    );
}
