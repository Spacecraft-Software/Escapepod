// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The real devices: evdev keyboards and pointers, and the uinput virtual
//! keyboard, over the safe API of the `evdev` crate.

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};

use escapepod_core::{KeyCode, KeyEvent, KeyValue};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, Device, EventType, InputEvent, InputId};

use crate::discover::{OWN_NAME_PREFIX, OWN_PRODUCT, OWN_VENDOR};
use crate::io::{Keyboard, Opener, Pointer, Sink};

/// `BTN_MISC`, the first button code: keys below it, plus the range from
/// `KEY_OK` (0x160) up, are keyboard keys the virtual keyboard declares.
const BTN_MISC: u16 = 0x100;
const KEY_OK: u16 = 0x160;

/// The bus type of Escape Pod's virtual keyboard (`BUS_VIRTUAL`).
const BUS_VIRTUAL: u16 = 0x06;

/// An evdev keyboard opened for reading, non-blocking.
#[derive(Debug)]
pub struct EvdevKeyboard {
    dev: Device,
    path: PathBuf,
}

impl EvdevKeyboard {
    /// The device node.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl AsFd for EvdevKeyboard {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.dev.as_fd()
    }
}

impl Keyboard for EvdevKeyboard {
    fn grab(&mut self) -> io::Result<()> {
        self.dev.grab()
    }

    fn keys_down(&self) -> io::Result<usize> {
        Ok(self.dev.get_key_state()?.iter().count())
    }

    fn read(&mut self, out: &mut Vec<KeyEvent>) -> io::Result<()> {
        loop {
            match self.dev.fetch_events() {
                Ok(events) => out.extend(events.filter_map(|e| key_event(&e))),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e),
            }
        }
    }
}

/// An evdev pointer opened for reading, never grabbed.
#[derive(Debug)]
pub struct EvdevPointer {
    dev: Device,
}

impl AsFd for EvdevPointer {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.dev.as_fd()
    }
}

impl Pointer for EvdevPointer {
    fn read_presses(&mut self) -> io::Result<usize> {
        let mut n = 0;
        loop {
            match self.dev.fetch_events() {
                Ok(events) => {
                    n += events
                        .filter(|e| {
                            e.event_type() == EventType::KEY
                                && e.code() >= BTN_MISC
                                && e.code() < KEY_OK
                                && e.value() == 1
                        })
                        .count();
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(n),
                Err(e) => return Err(e),
            }
        }
    }
}

/// Converts a kernel event to a key event; `None` for anything else.
fn key_event(e: &InputEvent) -> Option<KeyEvent> {
    if e.event_type() != EventType::KEY {
        return None;
    }
    Some(KeyEvent {
        key: KeyCode::new(e.code())?,
        value: KeyValue::from_raw(e.value())?,
    })
}

/// Opens real devices.
#[derive(Debug, Default)]
pub struct EvdevOpener;

fn open(path: &Path) -> io::Result<Device> {
    let dev = Device::open(path)?;
    dev.set_nonblocking(true)?;
    Ok(dev)
}

impl Opener for EvdevOpener {
    type Keyboard = EvdevKeyboard;
    type Pointer = EvdevPointer;

    fn keyboard(&mut self, path: &Path) -> io::Result<EvdevKeyboard> {
        Ok(EvdevKeyboard {
            dev: open(path)?,
            path: path.to_path_buf(),
        })
    }

    fn pointer(&mut self, path: &Path) -> io::Result<EvdevPointer> {
        Ok(EvdevPointer { dev: open(path)? })
    }
}

/// Escape Pod's uinput virtual keyboard. Its name starts with
/// [`OWN_NAME_PREFIX`] and it carries Escape Pod's vendor and product
/// identifiers, so no Escape Pod process ever opens it (ESC-SRS-045).
#[derive(Debug)]
pub struct UinputKeyboard {
    dev: VirtualDevice,
}

impl UinputKeyboard {
    /// Creates the virtual keyboard, declaring every keyboard key.
    ///
    /// # Errors
    ///
    /// The error opening `/dev/uinput` or creating the device.
    pub fn create() -> io::Result<Self> {
        let mut keys = AttributeSet::<evdev::KeyCode>::new();
        for code in (1..BTN_MISC).chain(KEY_OK..=KeyCode::MAX) {
            keys.insert(evdev::KeyCode::new(code));
        }
        let name = format!("{OWN_NAME_PREFIX} {}", std::process::id());
        let dev = VirtualDevice::builder()?
            .name(name.as_bytes())
            .input_id(InputId::new(
                BusType(BUS_VIRTUAL),
                OWN_VENDOR,
                OWN_PRODUCT,
                1,
            ))
            .with_keys(&keys)?
            .build()?;
        Ok(Self { dev })
    }
}

impl Sink for UinputKeyboard {
    fn emit(&mut self, events: &[KeyEvent]) -> io::Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let raw: Vec<InputEvent> = events
            .iter()
            .map(|e| InputEvent::new(EventType::KEY.0, e.key.code(), e.value.raw()))
            .collect();
        self.dev.emit(&raw)
    }
}

// Rust guideline compliant 2026-05-18
