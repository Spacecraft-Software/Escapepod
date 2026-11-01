// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The daemon's device logic, written against the I/O seam of
//! `escapepod-evdev` so it is tested with fakes: grabbing, reading,
//! faults, removal, hotplug, reload, termination and the emergency chord.
//! The `poll(2)` loop that drives it is in `service.rs`.

use std::collections::BTreeMap;
use std::io;

use std::time::Duration;

use escapepod_core::{DeviceId, Instant, KeyEvent, Session};
use escapepod_evdev::{DeviceInfo, DeviceKind, Keyboard, Opener, Pointer, Sink};
use serde_json::{Value, json};

use crate::config::Config;
use crate::error::AppError;
use crate::monitor::{self, Record};
use crate::output::{Severity, now_utc};

/// How long a keyboard may report keys down before it is grabbed anyway.
/// A key held at the moment of the grab would lose its release to the
/// grab and stay stuck in the compositor, so the daemon waits for the
/// keyboard to go idle first (manual, `Safety model`).
pub const IDLE_WAIT: Duration = Duration::from_secs(1);

/// How often a keyboard waiting to go idle is checked. Only while one is
/// waiting does the loop wake on a timer; otherwise it sleeps until input.
pub const IDLE_POLL: Duration = Duration::from_millis(20);

/// A diagnostic the loop writes to standard error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// Its severity.
    pub severity: Severity,
    /// Its upper-snake-case code.
    pub code: &'static str,
    /// Its one-sentence message.
    pub message: String,
}

/// What the loop does after a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Keep serving.
    Continue,
    /// The emergency chord fired: every key and grab is released; exit 6.
    Emergency,
}

struct Kbd<K> {
    info: DeviceInfo,
    dev: K,
    grabbed: bool,
    deadline: Instant,
}

/// The daemon state: the session, the open devices and the virtual
/// keyboard. Dropping it, as a panic's unwind does, releases every held
/// key (ESC-SRS-047).
pub struct Daemon<O: Opener, S: Sink> {
    opener: O,
    sink: S,
    session: Session,
    config: Config,
    keyboards: BTreeMap<DeviceId, Kbd<O::Keyboard>>,
    pointers: BTreeMap<DeviceId, (DeviceInfo, O::Pointer)>,
    next: u32,
    out: Vec<KeyEvent>,
    keys: Vec<KeyEvent>,
    notes: Vec<Note>,
    records: Vec<Record>,
}

impl<O: Opener, S: Sink> Daemon<O, S> {
    /// A daemon writing to `sink`, which already exists: the virtual
    /// keyboard is created before any device is opened (ESC-SRS-043).
    pub fn new(opener: O, sink: S, config: Config) -> Self {
        let session = Session::new(config.chord.clone());
        Self {
            opener,
            sink,
            session,
            config,
            keyboards: BTreeMap::new(),
            pointers: BTreeMap::new(),
            next: 0,
            out: Vec::new(),
            keys: Vec::new(),
            notes: Vec::new(),
            records: Vec::new(),
        }
    }

    fn note(&mut self, severity: Severity, code: &'static str, message: String) {
        self.notes.push(Note {
            severity,
            code,
            message,
        });
    }

    /// Takes the diagnostics gathered since the last call.
    pub fn take_notes(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.notes)
    }

    /// Takes the redacted records of managed keys since the last call, for
    /// the status socket.
    pub fn take_records(&mut self) -> Vec<Record> {
        std::mem::take(&mut self.records)
    }

    fn is_open(&self, path: &std::path::Path) -> bool {
        self.keyboards.values().any(|k| k.info.path == path)
            || self.pointers.values().any(|(i, _)| i.path == path)
    }

    /// Opens `info` if the daemon should: a matched keyboard for a grab
    /// once idle, or a pointer for reading. Never a device an Escape Pod
    /// process created (ESC-SRS-045). Returns whether it was opened.
    pub fn add(&mut self, info: &DeviceInfo, now: Instant) -> bool {
        if info.is_own() || self.is_open(&info.path) {
            return false;
        }
        match info.kind {
            DeviceKind::Keyboard => {
                if self.config.rules_for(info).is_none() {
                    return false;
                }
                match self.opener.keyboard(&info.path) {
                    Ok(dev) => {
                        let id = DeviceId(self.next);
                        self.next += 1;
                        let deadline = now.saturating_add(IDLE_WAIT);
                        self.keyboards.insert(
                            id,
                            Kbd {
                                info: info.clone(),
                                dev,
                                grabbed: false,
                                deadline,
                            },
                        );
                        self.try_grab(id, now);
                        true
                    }
                    Err(e) => {
                        // A fresh node is opened before udev sets its
                        // permissions; the attribute change retries it.
                        let severity = if e.kind() == io::ErrorKind::PermissionDenied {
                            Severity::Info
                        } else {
                            Severity::Warn
                        };
                        self.note(
                            severity,
                            "DEVICE_OPEN_FAILED",
                            format!("cannot open {} ({}): {e}", info.name, info.path.display()),
                        );
                        false
                    }
                }
            }
            // Pointers are read for button presses and never grabbed
            // (ESC-SRS-053). Implements: ESC-SRS-053
            DeviceKind::Pointer => match self.opener.pointer(&info.path) {
                Ok(dev) => {
                    let id = DeviceId(self.next);
                    self.next += 1;
                    self.pointers.insert(id, (info.clone(), dev));
                    true
                }
                Err(_) => false,
            },
            DeviceKind::Other => false,
        }
    }

    /// Grabs keyboard `id` exclusively once it is idle or its wait is over.
    /// Implements: ESC-SRS-044
    fn try_grab(&mut self, id: DeviceId, now: Instant) {
        let Some(k) = self.keyboards.get_mut(&id) else {
            return;
        };
        if k.grabbed {
            return;
        }
        let idle = k.dev.keys_down().map_or(true, |n| n == 0);
        if !idle && now < k.deadline {
            return;
        }
        match k.dev.grab() {
            Ok(()) => {
                k.grabbed = true;
                let rules = self.config.rules_for(&k.info).cloned().unwrap_or_default();
                let msg = format!("grabbed {} ({})", k.info.name, k.info.path.display());
                self.session.add_device(id, rules);
                self.note(Severity::Info, "DEVICE_GRABBED", msg);
            }
            Err(e) => {
                let msg = format!(
                    "cannot grab {} ({}): {e}",
                    k.info.name,
                    k.info.path.display()
                );
                self.keyboards.remove(&id);
                self.note(Severity::Warn, "DEVICE_BUSY", msg);
            }
        }
    }

    /// Grabs every waiting keyboard that went idle or ran out of time.
    pub fn tick(&mut self, now: Instant) {
        let waiting: Vec<DeviceId> = self
            .keyboards
            .iter()
            .filter(|(_, k)| !k.grabbed)
            .map(|(&id, _)| id)
            .collect();
        for id in waiting {
            self.try_grab(id, now);
        }
    }

    /// How long the loop may sleep: forever, unless a keyboard is waiting
    /// to go idle (ESC-SRS-057).
    pub fn timeout(&self) -> Option<Duration> {
        self.keyboards
            .values()
            .any(|k| !k.grabbed)
            .then_some(IDLE_POLL)
    }

    fn flush(&mut self) {
        if self.out.is_empty() {
            return;
        }
        if let Err(e) = self.sink.emit(&self.out) {
            self.notes.push(Note {
                severity: Severity::Error,
                code: "OUTPUT_FAILED",
                message: format!("cannot write to the virtual keyboard: {e}"),
            });
        }
        self.out.clear();
    }

    /// Reads keyboard `id`. Events of a keyboard not yet grabbed also reach
    /// the compositor, so they are dropped here.
    pub fn keyboard_ready(&mut self, id: DeviceId, now: Instant) -> Flow {
        let Some(k) = self.keyboards.get_mut(&id) else {
            return Flow::Continue;
        };
        self.keys.clear();
        if let Err(e) = k.dev.read(&mut self.keys) {
            self.fault(id, &e);
            return Flow::Continue;
        }
        if !k.grabbed {
            self.keys.clear();
            return Flow::Continue;
        }
        let rules = self.session.rules(id).cloned().unwrap_or_default();
        let device = k
            .info
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let keys = std::mem::take(&mut self.keys);
        for &ev in &keys {
            let step = self.session.on_key(id, ev, now, &mut self.out);
            if step.emergency {
                self.flush();
                self.release_grabs();
                self.note(
                    Severity::Warn,
                    "EMERGENCY_RELEASE",
                    "emergency chord pressed: every key and grab released".into(),
                );
                self.keys = keys;
                return Flow::Emergency;
            }
            if rules.is_managed(ev.key) {
                self.records.push(monitor::record(
                    &rules,
                    &device,
                    ev,
                    step.decision.as_ref(),
                    false,
                    now_utc(),
                ));
            }
        }
        self.keys = keys;
        self.flush();
        Flow::Continue
    }

    /// Reads pointer `id` for button presses.
    pub fn pointer_ready(&mut self, id: DeviceId, now: Instant) {
        let Some((_, p)) = self.pointers.get_mut(&id) else {
            return;
        };
        match p.read_presses() {
            Ok(0) => {}
            Ok(_) => self.session.on_pointer_press(now),
            Err(_) => {
                self.pointers.remove(&id);
            }
        }
    }

    /// A keyboard failed or went away: release its keys and its grab, warn
    /// unless it was simply unplugged, and keep serving the others.
    /// Implements: ESC-SRS-050, ESC-SRS-052, ESC-SRS-097, ESC-SRS-098
    fn fault(&mut self, id: DeviceId, e: &io::Error) {
        self.session.remove_device(id, &mut self.out);
        self.flush();
        let Some(k) = self.keyboards.remove(&id) else {
            return;
        };
        let removed = e.raw_os_error() == Some(19); // ENODEV: unplugged
        if removed {
            self.note(
                Severity::Info,
                "DEVICE_RELEASED",
                format!("{} ({}) was removed", k.info.name, k.info.path.display()),
            );
        } else {
            self.note(
                Severity::Warn,
                "DEVICE_FAULT",
                format!(
                    "reading {} ({}) failed: {e}; its grab is released",
                    k.info.name,
                    k.info.path.display()
                ),
            );
        }
        // Dropping the device closes it, which releases the grab.
        drop(k);
    }

    /// A device node appeared or changed permissions (hotplug).
    /// Implements: ESC-SRS-051
    pub fn hotplug(&mut self, info: &DeviceInfo, now: Instant) {
        self.add(info, now);
    }

    /// Applies a reloaded configuration, or keeps the current one when the
    /// new one does not validate. Devices still matched keep their grab;
    /// `devices` is a fresh scan, for devices newly matched.
    /// Implements: ESC-SRS-054, ESC-SRS-055, ESC-SRS-056, ESC-SRS-099
    pub fn reload(&mut self, new: Result<Config, AppError>, devices: &[DeviceInfo], now: Instant) {
        let cfg = match new {
            Ok(c) => c,
            Err(e) => {
                self.note(
                    Severity::Warn,
                    "RELOAD_FAILED",
                    format!(
                        "reload failed, keeping the previous configuration: {}",
                        e.message()
                    ),
                );
                return;
            }
        };
        let ids: Vec<DeviceId> = self.keyboards.keys().copied().collect();
        for id in ids {
            let Some(k) = self.keyboards.get(&id) else {
                continue;
            };
            match (cfg.rules_for(&k.info).cloned(), k.grabbed) {
                (Some(rules), true) => self.session.replace_rules(id, rules, &mut self.out),
                (Some(_), false) => {}
                (None, _) => {
                    self.session.remove_device(id, &mut self.out);
                    self.keyboards.remove(&id);
                }
            }
        }
        self.session.set_chord(cfg.chord.clone());
        self.config = cfg;
        self.flush();
        for d in devices {
            self.add(d, now);
        }
        self.note(Severity::Ok, "RELOADED", "configuration reloaded".into());
    }

    /// Releases every held key, as on `SIGTERM` or `SIGINT`.
    /// Implements: ESC-SRS-046
    pub fn terminate(&mut self) {
        self.session.release_all(&mut self.out);
        self.flush();
    }

    /// Closes every keyboard, which releases every grab (ESC-SRS-048).
    /// Implements: ESC-SRS-048
    fn release_grabs(&mut self) {
        let ids: Vec<DeviceId> = self.keyboards.keys().copied().collect();
        for id in ids {
            self.session.remove_device(id, &mut self.out);
        }
        self.keyboards.clear();
        self.flush();
    }

    /// The keyboard ids and devices, for the poll loop.
    pub fn keyboards(&self) -> impl Iterator<Item = (DeviceId, &O::Keyboard)> {
        self.keyboards.iter().map(|(&id, k)| (id, &k.dev))
    }

    /// The pointer ids and devices, for the poll loop.
    pub fn pointers(&self) -> impl Iterator<Item = (DeviceId, &O::Pointer)> {
        self.pointers.iter().map(|(&id, (_, p))| (id, p))
    }

    /// The status-socket report: devices, grabs and the number of held
    /// keys, never which keys.
    pub fn status(&self) -> Value {
        json!({
            "backend": "evdev",
            "devices": self.keyboards.values().map(|k| json!({
                "name": k.info.name, "path": k.info.path.display().to_string(), "grabbed": k.grabbed,
            })).collect::<Vec<_>>(),
            "pointers": self.pointers.len(),
            "held": self.session.held().keys().count(),
        })
    }

    /// The paths of the keyboards held open.
    #[cfg(test)]
    pub fn open_paths(&self) -> Vec<std::path::PathBuf> {
        self.keyboards
            .values()
            .map(|k| k.info.path.clone())
            .collect()
    }
}

impl<O: Opener, S: Sink> Drop for Daemon<O, S> {
    /// Runs on every exit, including a panic's unwind: no key stays held.
    /// Implements: ESC-SRS-047
    fn drop(&mut self) {
        if !self.session.held().is_empty() {
            self.session.release_all(&mut self.out);
            let _ = self.sink.emit(&self.out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Overrides, load};
    use escapepod_core::{KeyCode, KeyValue};
    use escapepod_evdev::fake::{FakeOpener, FakeSink, KeyboardHandle};

    const CAPS: KeyCode = KeyCode::CAPSLOCK;
    const CTRL: KeyCode = KeyCode::LEFTCTRL;

    fn kb(path: &str, name: &str) -> DeviceInfo {
        DeviceInfo {
            path: path.into(),
            name: name.into(),
            vendor: 1,
            product: 1,
            kind: DeviceKind::Keyboard,
        }
    }

    fn caps_config() -> Config {
        let o = Overrides {
            map: Some(crate::keymap::parse("Caps_Lock=Escape").unwrap()),
            ..Overrides::default()
        };
        let mut c = load(None, &o, &[]).unwrap();
        // Caps Lock holds Control, as the usual setup does.
        c.global = escapepod_core::Rules::builder()
            .dual_role(CAPS, CTRL, KeyCode::ESC)
            .build()
            .unwrap();
        c
    }

    type D = Daemon<FakeOpener, FakeSink>;

    fn daemon(cfg: Config) -> (D, FakeOpener, FakeSink) {
        let opener = FakeOpener::default();
        let sink = FakeSink::default();
        (Daemon::new(opener.clone(), sink.clone(), cfg), opener, sink)
    }

    fn t(ms: u64) -> Instant {
        Instant::from_micros(ms * 1000)
    }

    fn press(h: &KeyboardHandle, d: &mut D, id: u32, ev: KeyEvent, ms: u64) -> Flow {
        h.borrow_mut().queue.push_back(ev);
        d.keyboard_ready(DeviceId(id), t(ms))
    }

    fn emitted(s: &FakeSink) -> Vec<KeyEvent> {
        s.0.borrow().clone()
    }

    // Verifies: ESC-SRS-044, ESC-SRS-045
    #[test]
    fn grabs_matched_keyboards_and_never_its_own() {
        let (mut d, opener, _) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        let _ = opener.add_keyboard("/dev/input/event9");
        assert!(d.add(&kb("/dev/input/event0", "laptop"), t(0)));
        let own = DeviceInfo {
            name: format!("{} 42", escapepod_evdev::OWN_NAME_PREFIX),
            ..kb("/dev/input/event9", "")
        };
        assert!(!d.add(&own, t(0)));
        assert!(a.borrow().grabbed);
        assert_eq!(
            opener.opened.borrow().as_slice(),
            [std::path::PathBuf::from("/dev/input/event0")],
            "its own device is never opened"
        );
    }

    #[test]
    fn waits_for_idle_before_grabbing() {
        let (mut d, opener, _) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        a.borrow_mut().down = 1;
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        assert!(!a.borrow().grabbed);
        assert_eq!(d.timeout(), Some(IDLE_POLL));
        a.borrow_mut().queue.push_back(KeyEvent::release(CAPS));
        d.keyboard_ready(DeviceId(0), t(5));
        a.borrow_mut().down = 0;
        d.tick(t(10));
        assert!(a.borrow().grabbed);
        assert_eq!(d.timeout(), None, "no timer once every keyboard is grabbed");
        let b = opener.add_keyboard("/dev/input/event1");
        b.borrow_mut().down = 2;
        d.add(&kb("/dev/input/event1", "stuck"), t(20));
        d.tick(t(1021));
        assert!(b.borrow().grabbed, "grabbed anyway after the wait");
    }

    #[test]
    fn maps_through_the_session() {
        let (mut d, opener, sink) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 1);
        press(&a, &mut d, 0, KeyEvent::release(CAPS), 50);
        assert_eq!(
            emitted(&sink),
            [
                KeyEvent::press(CTRL),
                KeyEvent::release(CTRL),
                KeyEvent::press(KeyCode::ESC),
                KeyEvent::release(KeyCode::ESC)
            ]
        );
        let recs = d.take_records();
        assert_eq!(recs.len(), 2);
        assert!(recs.iter().all(|r| r.key == "KEY_CAPSLOCK"));
    }

    // Verifies: ESC-SRS-046
    #[test]
    fn terminate_releases_every_held_key() {
        let (mut d, opener, sink) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 1);
        press(
            &a,
            &mut d,
            0,
            KeyEvent::press(KeyCode::from_name("a").unwrap()),
            2,
        );
        sink.0.borrow_mut().clear();
        d.terminate();
        let out = emitted(&sink);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|e| e.value == KeyValue::Release));
    }

    // Verifies: ESC-SRS-047
    #[test]
    fn a_panic_releases_every_held_key() {
        let opener = FakeOpener::default();
        let sink = FakeSink::default();
        let a = opener.add_keyboard("/dev/input/event0");
        let (o2, s2, a2) = (opener.clone(), sink.clone(), std::rc::Rc::clone(&a));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let mut d = Daemon::new(o2, s2, caps_config());
            d.add(&kb("/dev/input/event0", "laptop"), t(0));
            press(&a2, &mut d, 0, KeyEvent::press(CAPS), 1);
            panic!("a defect");
        }));
        assert!(result.is_err());
        assert_eq!(
            emitted(&sink),
            [KeyEvent::press(CTRL), KeyEvent::release(CTRL)]
        );
        assert!(!a.borrow().grabbed, "the grab went with the descriptor");
    }

    // Verifies: ESC-SRS-048, ESC-SRS-096
    #[test]
    fn emergency_chord_releases_keys_and_grabs() {
        let (mut d, opener, sink) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        let b = opener.add_keyboard("/dev/input/event1");
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        d.add(&kb("/dev/input/event1", "external"), t(0));
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 1);
        press(&a, &mut d, 0, KeyEvent::press(KeyCode::BACKSPACE), 2);
        press(&b, &mut d, 1, KeyEvent::press(KeyCode::ESC), 3);
        let flow = press(&a, &mut d, 0, KeyEvent::press(KeyCode::ENTER), 4);
        assert_eq!(flow, Flow::Emergency);
        assert!(!a.borrow().grabbed && !b.borrow().grabbed);
        let out = emitted(&sink);
        let ends = out
            .iter()
            .rev()
            .take(3)
            .all(|e| e.value == KeyValue::Release);
        assert!(ends && out.contains(&KeyEvent::release(CTRL)), "{out:?}");
        assert!(d.take_notes().iter().any(|n| n.code == "EMERGENCY_RELEASE"));
    }

    // Verifies: ESC-SRS-050, ESC-SRS-097, ESC-SRS-098
    #[test]
    fn a_failing_device_is_released_and_the_others_keep_working() {
        let (mut d, opener, sink) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        let b = opener.add_keyboard("/dev/input/event1");
        d.add(&kb("/dev/input/event0", "flaky"), t(0));
        d.add(&kb("/dev/input/event1", "good"), t(0));
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 1);
        a.borrow_mut().fail = Some(io::ErrorKind::Other);
        d.keyboard_ready(DeviceId(0), t(2));
        assert!(!a.borrow().grabbed, "the faulty device's grab is released");
        let notes = d.take_notes();
        let warn = notes.iter().find(|n| n.code == "DEVICE_FAULT").unwrap();
        assert_eq!(warn.severity, Severity::Warn);
        assert!(warn.message.contains("flaky"));
        assert!(emitted(&sink).contains(&KeyEvent::release(CTRL)));
        sink.0.borrow_mut().clear();
        let x = KeyCode::from_name("x").unwrap();
        press(&b, &mut d, 1, KeyEvent::press(x), 3);
        assert_eq!(emitted(&sink), [KeyEvent::press(x)]);
    }

    // Verifies: ESC-SRS-052
    #[test]
    fn removal_releases_that_devices_keys() {
        let (mut d, opener, sink) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        d.add(&kb("/dev/input/event0", "usb"), t(0));
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 1);
        // ENODEV is how the kernel reports an unplugged device.
        d.fault(DeviceId(0), &io::Error::from_raw_os_error(19));
        assert_eq!(emitted(&sink).last(), Some(&KeyEvent::release(CTRL)));
        assert!(
            d.take_notes()
                .iter()
                .any(|n| n.code == "DEVICE_RELEASED" && n.severity == Severity::Info)
        );
        assert!(d.open_paths().is_empty());
    }

    // Verifies: ESC-SRS-051
    #[test]
    fn a_hotplugged_keyboard_is_grabbed() {
        let (mut d, opener, _) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event5");
        d.hotplug(&kb("/dev/input/event5", "dock"), t(100));
        assert!(a.borrow().grabbed);
        d.hotplug(&kb("/dev/input/event5", "dock"), t(101));
        assert_eq!(
            opener.opened.borrow().len(),
            1,
            "a known device is not reopened"
        );
    }

    // Verifies: ESC-SRS-053
    #[test]
    fn pointers_are_read_not_grabbed() {
        let (mut d, opener, sink) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        let p = opener.add_pointer("/dev/input/event2");
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        let ptr = DeviceInfo {
            kind: DeviceKind::Pointer,
            ..kb("/dev/input/event2", "touchpad")
        };
        assert!(d.add(&ptr, t(0)));
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 1);
        *p.borrow_mut() = 1;
        d.pointer_ready(DeviceId(1), t(2));
        press(&a, &mut d, 0, KeyEvent::release(CAPS), 3);
        assert_eq!(
            emitted(&sink),
            [KeyEvent::press(CTRL), KeyEvent::release(CTRL)],
            "the click used the key"
        );
    }

    // Verifies: ESC-SRS-054, ESC-SRS-056
    #[test]
    fn reload_applies_and_keeps_grabs() {
        let (mut d, opener, sink) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        let mut new = caps_config();
        new.global = escapepod_core::Rules::builder()
            .remap(CAPS, KeyCode::ESC)
            .build()
            .unwrap();
        d.reload(Ok(new), &[], t(10));
        assert!(a.borrow().grabbed, "the grab was never dropped");
        assert_eq!(
            opener.opened.borrow().len(),
            1,
            "the device was not reopened"
        );
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 20);
        assert_eq!(emitted(&sink), [KeyEvent::press(KeyCode::ESC)]);
    }

    // Verifies: ESC-SRS-055, ESC-SRS-099
    #[test]
    fn a_failed_reload_keeps_the_previous_configuration() {
        let (mut d, opener, sink) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        d.reload(
            Err(AppError::invalid(
                "unknown field `tapp` at line 5, column 1",
                "x",
            )),
            &[],
            t(10),
        );
        let notes = d.take_notes();
        let w = notes.iter().find(|n| n.code == "RELOAD_FAILED").unwrap();
        assert_eq!(w.severity, Severity::Warn);
        assert!(w.message.contains("tapp"), "names the first error");
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 20);
        assert_eq!(
            emitted(&sink),
            [KeyEvent::press(CTRL)],
            "still the old mapping"
        );
    }

    // Verifies: ESC-SRS-057
    #[test]
    fn idle_daemon_has_no_timer() {
        let (mut d, opener, _) = daemon(caps_config());
        let _ = opener.add_keyboard("/dev/input/event0");
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        assert_eq!(d.timeout(), None);
    }

    // Verifies: ESC-SRS-060
    #[test]
    fn socket_records_carry_managed_keys_only() {
        let (mut d, opener, _) = daemon(caps_config());
        let a = opener.add_keyboard("/dev/input/event0");
        d.add(&kb("/dev/input/event0", "laptop"), t(0));
        for name in ["p", "a", "s", "s"] {
            let k = KeyCode::from_name(name).unwrap();
            press(&a, &mut d, 0, KeyEvent::press(k), 1);
            press(&a, &mut d, 0, KeyEvent::release(k), 2);
        }
        press(&a, &mut d, 0, KeyEvent::press(CAPS), 3);
        let text: String = d
            .take_records()
            .iter()
            .map(|r| r.to_json().to_string())
            .collect();
        assert!(text.contains("KEY_CAPSLOCK"));
        for leaked in ["KEY_P", "KEY_A", "KEY_S"] {
            assert!(!text.contains(leaked), "{leaked} leaked: {text}");
        }
        let status = d.status().to_string();
        assert!(!status.contains("KEY_"), "status names no key: {status}");
    }
}

// Rust guideline compliant 2026-05-18
