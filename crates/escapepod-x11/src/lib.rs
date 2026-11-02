// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The X11 fallback backend of Escape Pod.
//!
//! It observes key and button events through the X RECORD extension and
//! injects tap keys through XTEST, over a pure-Rust connection (`x11rb`).
//! RECORD cannot suppress an event, so the physical key already reaches
//! clients: this backend injects the tap key's press and release and
//! nothing else, and it needs no access to any input device.
//!
//! - [`Tapper`] is the pure logic: decoded events in, keys to inject out,
//!   with Escape Pod's own injections recognised and skipped.
//! - [`Backend`] is the connection: it runs RECORD and calls XTEST.

use std::fmt;
use std::time::Duration;

use escapepod_core::{Decision, Engine, Input, Instant, KeyCode, KeyEvent, KeyValue, Rules};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::record::{self, ConnectionExt as _};
use x11rb::protocol::xproto::Window;
use x11rb::protocol::xtest::{self, ConnectionExt as _};
use x11rb::rust_connection::RustConnection;

/// X core event codes.
const KEY_PRESS: u8 = 2;
const KEY_RELEASE: u8 = 3;
const BUTTON_PRESS: u8 = 4;

/// X keycodes are evdev codes plus 8.
const KEYCODE_OFFSET: u8 = 8;

/// An X event the backend cares about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XEvent {
    /// A key went down or up, by X keycode.
    Key {
        /// The X keycode.
        keycode: u8,
        /// Whether it went down.
        press: bool,
    },
    /// A pointer button went down.
    Button,
}

/// Decodes the device events in a RECORD reply's data: a run of 32-byte
/// core event records. Other events are skipped.
#[must_use]
pub fn decode(data: &[u8]) -> Vec<XEvent> {
    data.as_chunks::<32>()
        .0
        .iter()
        .filter_map(|e| match e[0] & 0x7f {
            KEY_PRESS => Some(XEvent::Key {
                keycode: e[1],
                press: true,
            }),
            KEY_RELEASE => Some(XEvent::Key {
                keycode: e[1],
                press: false,
            }),
            BUTTON_PRESS => Some(XEvent::Button),
            _ => None,
        })
        .collect()
}

/// The X11 backend's pure logic: an engine fed with observed events,
/// emitting only the tap key, and skipping its own injected events when
/// RECORD reports them back.
#[derive(Debug)]
pub struct Tapper {
    engine: Engine,
    held: Vec<u8>,
    injected: Vec<(u8, bool)>,
    scratch: Vec<KeyEvent>,
}

impl Tapper {
    /// A tapper applying `rules`.
    ///
    /// # Errors
    ///
    /// [`RemapError`] when the rules contain a remap or a dual-role key
    /// whose hold key differs from the physical key: RECORD cannot suppress
    /// the physical key, so neither can be honoured (ESC-SRS-066).
    /// Implements: ESC-SRS-066
    pub fn new(rules: Rules) -> Result<Self, RemapError> {
        if let Some((source, target)) = rules.remaps().next() {
            return Err(RemapError {
                key: source,
                target,
            });
        }
        if let Some(d) = rules.dual_roles().find(|d| d.hold != d.key) {
            return Err(RemapError {
                key: d.key,
                target: d.hold,
            });
        }
        Ok(Self {
            engine: Engine::new(rules),
            held: Vec::new(),
            injected: Vec::new(),
            scratch: Vec::new(),
        })
    }

    /// Observes one event at `now`; returns the X keycodes to inject, as
    /// (keycode, press) pairs: only ever a tap key's press and release.
    /// Implements: ESC-SRS-065, ESC-SRS-088
    pub fn observe(&mut self, ev: XEvent, now: Instant) -> Vec<(u8, bool)> {
        let input = match ev {
            XEvent::Button => Input::PointerPress,
            XEvent::Key { keycode, press } => {
                if let Some(i) = self.injected.iter().position(|&x| x == (keycode, press)) {
                    // Our own injection, reported back by RECORD.
                    self.injected.remove(i);
                    return Vec::new();
                }
                let Some(key) = keycode
                    .checked_sub(KEYCODE_OFFSET)
                    .and_then(|c| KeyCode::new(u16::from(c)))
                else {
                    return Vec::new();
                };
                // X reports autorepeat as repeated presses.
                let value = if !press {
                    self.held.retain(|&k| k != keycode);
                    KeyValue::Release
                } else if self.held.contains(&keycode) {
                    KeyValue::Repeat
                } else {
                    self.held.push(keycode);
                    KeyValue::Press
                };
                Input::Key(KeyEvent { key, value })
            }
        };
        self.scratch.clear();
        match self.engine.step(input, now, &mut self.scratch) {
            Some(Decision::Tap(d)) => {
                let Some(code) = u8::try_from(d.tap.code())
                    .ok()
                    .and_then(|c| c.checked_add(KEYCODE_OFFSET))
                else {
                    return Vec::new();
                };
                let out = vec![(code, true), (code, false)];
                self.injected.extend_from_slice(&out);
                out
            }
            _ => Vec::new(),
        }
    }
}

/// The configuration asks the X11 backend for a remap it cannot make.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemapError {
    /// The physical key.
    pub key: KeyCode,
    /// The key it was to become.
    pub target: KeyCode,
}

impl fmt::Display for RemapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the X11 backend cannot remap {} to {}: X RECORD cannot suppress a key",
            self.key, self.target
        )
    }
}

impl std::error::Error for RemapError {}

/// What went wrong talking to the X server.
#[derive(Debug)]
pub struct X11Error {
    kind: X11ErrorKind,
    display: String,
    detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum X11ErrorKind {
    Connect,
    Extension,
    Lost,
}

impl X11Error {
    /// Whether the server could not be reached at all.
    #[must_use]
    pub fn is_connect(&self) -> bool {
        self.kind == X11ErrorKind::Connect
    }

    /// Whether the server lacks RECORD or XTEST.
    #[must_use]
    pub fn is_missing_extension(&self) -> bool {
        self.kind == X11ErrorKind::Extension
    }

    /// Whether an established connection was lost.
    #[must_use]
    pub fn is_lost(&self) -> bool {
        self.kind == X11ErrorKind::Lost
    }

    /// The display name.
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }
}

impl fmt::Display for X11Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            X11ErrorKind::Connect => write!(
                f,
                "cannot connect to X display {}: {}",
                self.display, self.detail
            ),
            X11ErrorKind::Extension => write!(
                f,
                "X display {} lacks the {} extension",
                self.display, self.detail
            ),
            X11ErrorKind::Lost => write!(
                f,
                "lost the connection to X display {}: {}",
                self.display, self.detail
            ),
        }
    }
}

impl std::error::Error for X11Error {}

/// A connection to the X server: one connection for RECORD's data, one for
/// control and XTEST.
#[derive(Debug)]
pub struct Backend {
    data: RustConnection,
    control: RustConnection,
    root: Window,
    context: record::Context,
    display: String,
}

impl Backend {
    /// Connects to `display` (or `$DISPLAY`) and checks for RECORD and
    /// XTEST.
    ///
    /// # Errors
    ///
    /// An [`X11Error`] for a server that cannot be reached or lacks an
    /// extension.
    pub fn connect(display: &str) -> Result<Self, X11Error> {
        let err = |kind, detail: String| X11Error {
            kind,
            display: display.to_owned(),
            detail,
        };
        let (control, screen) =
            x11rb::connect(Some(display)).map_err(|e| err(X11ErrorKind::Connect, e.to_string()))?;
        let (data, _) =
            x11rb::connect(Some(display)).map_err(|e| err(X11ErrorKind::Connect, e.to_string()))?;
        for (name, ext) in [
            ("RECORD", record::X11_EXTENSION_NAME),
            ("XTEST", xtest::X11_EXTENSION_NAME),
        ] {
            let present = control
                .extension_information(ext)
                .map_err(|e| err(X11ErrorKind::Connect, e.to_string()))?;
            if present.is_none() {
                return Err(err(X11ErrorKind::Extension, name.to_owned()));
            }
        }
        let root = control
            .setup()
            .roots
            .get(screen)
            .map(|s| s.root)
            .ok_or_else(|| err(X11ErrorKind::Connect, "no screen".into()))?;
        let context = control
            .generate_id()
            .map_err(|e| err(X11ErrorKind::Connect, e.to_string()))?;
        let range = record::Range {
            device_events: record::Range8 {
                first: KEY_PRESS,
                last: BUTTON_PRESS,
            },
            ..record::Range::default()
        };
        control
            .record_create_context(context, 0, &[record::CS::ALL_CLIENTS.into()], &[range])
            .and_then(|c| {
                c.check().map_err(|e| {
                    x11rb::errors::ConnectionError::IoError(std::io::Error::other(e.to_string()))
                })
            })
            .map_err(|e| err(X11ErrorKind::Connect, e.to_string()))?;
        let _ = control.xtest_get_version(2, 2);
        Ok(Self {
            data,
            control,
            root,
            context,
            display: display.to_owned(),
        })
    }

    /// Runs until the connection is lost: observes through RECORD, injects
    /// the tap keys `tapper` returns through XTEST. Never returns `Ok`.
    ///
    /// # Errors
    ///
    /// An [`X11Error`] that [`X11Error::is_lost`] when the server goes away.
    /// Implements: ESC-SRS-068
    pub fn run(&self, tapper: &mut Tapper) -> Result<(), X11Error> {
        let lost = |detail: String| X11Error {
            kind: X11ErrorKind::Lost,
            display: self.display.clone(),
            detail,
        };
        let start = std::time::Instant::now();
        let cookie = self
            .data
            .record_enable_context(self.context)
            .map_err(|e| lost(e.to_string()))?;
        for reply in cookie {
            let reply = reply.map_err(|e| lost(e.to_string()))?;
            if reply.category != 0 {
                continue; // only FromServer records carry device events
            }
            let now = Instant::from_micros(
                u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX),
            );
            for ev in decode(&reply.data) {
                for (keycode, press) in tapper.observe(ev, now) {
                    let kind = if press { KEY_PRESS } else { KEY_RELEASE };
                    self.control
                        .xtest_fake_input(kind, keycode, 0, self.root, 0, 0, 0)
                        .map_err(|e| lost(e.to_string()))?;
                }
            }
            self.control.flush().map_err(|e| lost(e.to_string()))?;
        }
        Err(lost("the RECORD stream ended".into()))
    }

    /// The display name.
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }

    /// A tiny pause the tests use between connecting and typing.
    #[doc(hidden)]
    pub const SETTLE: Duration = Duration::from_millis(100);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules::builder()
            .dual_role(KeyCode::LEFTCTRL, KeyCode::LEFTCTRL, KeyCode::ESC)
            .build()
            .unwrap()
    }

    fn record(kind: u8, detail: u8) -> [u8; 32] {
        let mut r = [0u8; 32];
        r[0] = kind;
        r[1] = detail;
        r
    }

    const CTRL: u8 = 29 + KEYCODE_OFFSET;
    const ESC: u8 = 1 + KEYCODE_OFFSET;

    #[test]
    fn decodes_core_records() {
        let mut data = Vec::new();
        data.extend(record(KEY_PRESS, CTRL));
        data.extend(record(KEY_RELEASE | 0x80, CTRL));
        data.extend(record(BUTTON_PRESS, 1));
        data.extend(record(6, 0)); // MotionNotify: skipped
        assert_eq!(
            decode(&data),
            [
                XEvent::Key {
                    keycode: CTRL,
                    press: true
                },
                XEvent::Key {
                    keycode: CTRL,
                    press: false
                },
                XEvent::Button
            ]
        );
    }

    // Verifies: ESC-SRS-065, ESC-SRS-088
    #[test]
    fn injects_the_tap_key_only_and_skips_its_echo() {
        let mut t = Tapper::new(rules()).unwrap();
        let ms = |n: u64| Instant::from_micros(n * 1000);
        assert!(
            t.observe(
                XEvent::Key {
                    keycode: CTRL,
                    press: true
                },
                ms(0)
            )
            .is_empty(),
            "the hold key is never injected"
        );
        assert_eq!(
            t.observe(
                XEvent::Key {
                    keycode: CTRL,
                    press: false
                },
                ms(50)
            ),
            [(ESC, true), (ESC, false)]
        );
        // RECORD reports the injected Escape back: it is skipped, not tapped on.
        assert!(
            t.observe(
                XEvent::Key {
                    keycode: ESC,
                    press: true
                },
                ms(51)
            )
            .is_empty()
        );
        assert!(
            t.observe(
                XEvent::Key {
                    keycode: ESC,
                    press: false
                },
                ms(52)
            )
            .is_empty()
        );
        // A chord and a click both cancel the tap.
        t.observe(
            XEvent::Key {
                keycode: CTRL,
                press: true,
            },
            ms(100),
        );
        t.observe(XEvent::Button, ms(110));
        assert!(
            t.observe(
                XEvent::Key {
                    keycode: CTRL,
                    press: false
                },
                ms(120)
            )
            .is_empty()
        );
        // Autorepeat (a second press) keeps the tap.
        t.observe(
            XEvent::Key {
                keycode: CTRL,
                press: true,
            },
            ms(200),
        );
        t.observe(
            XEvent::Key {
                keycode: CTRL,
                press: true,
            },
            ms(250),
        );
        assert_eq!(
            t.observe(
                XEvent::Key {
                    keycode: CTRL,
                    press: false
                },
                ms(300)
            )
            .len(),
            2
        );
    }

    // Verifies: ESC-SRS-066
    #[test]
    fn remaps_are_refused() {
        let remap = Rules::builder()
            .remap(KeyCode::CAPSLOCK, KeyCode::LEFTCTRL)
            .build()
            .unwrap();
        assert_eq!(
            Tapper::new(remap).err(),
            Some(RemapError {
                key: KeyCode::CAPSLOCK,
                target: KeyCode::LEFTCTRL
            })
        );
        let moved = Rules::builder()
            .dual_role(KeyCode::CAPSLOCK, KeyCode::LEFTCTRL, KeyCode::ESC)
            .build()
            .unwrap();
        assert!(
            Tapper::new(moved).is_err(),
            "a hold key other than the physical key is a remap"
        );
    }
}

// Rust guideline compliant 2026-05-18
