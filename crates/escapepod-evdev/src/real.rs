// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The real devices: evdev keyboards and pointers, and the uinput virtual
//! keyboard, over the safe API of the `evdev` crate.

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};

use escapepod_core::{KeyCode, KeyEvent, KeyValue};
use evdev::uinput::VirtualDevice;
use evdev::{
    AttributeSet, BusType, Device, EventType, InputEvent, InputId, MiscCode, RelativeAxisCode,
};

use crate::discover::{OWN_NAME_PREFIX, OWN_PRODUCT, OWN_VENDOR};
use crate::io::{Keyboard, Opener, Pointer, PointerEvent, Sink};

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

/// An evdev pointer opened non-blocking.
#[derive(Debug)]
pub struct EvdevPointer {
    dev: Device,
}

impl EvdevPointer {
    /// Whether the virtual pointer can preserve every declared event type.
    ///
    /// A caller must check this before grabbing: unsupported capabilities
    /// must remain available through the original device.
    #[must_use]
    pub fn supports_forwarding(&self) -> bool {
        self.dev.supported_events().iter().all(|kind| {
            matches!(
                kind,
                EventType::SYNCHRONIZATION | EventType::KEY | EventType::RELATIVE | EventType::MISC
            )
        }) && self
            .dev
            .supported_keys()
            .is_none_or(|keys| keys.iter().all(|key| (0x110..=0x11f).contains(&key.0)))
            && self.dev.supported_relative_axes().is_none_or(|axes| {
                axes.iter()
                    .all(|axis| (0..=9).contains(&axis.0) || (11..=12).contains(&axis.0))
            })
            && self
                .dev
                .misc_properties()
                .is_none_or(|misc| misc.iter().all(|code| code == MiscCode::MSC_SCAN))
    }

    /// Reads all complete input frames without discarding event boundaries.
    ///
    /// The events include `SYN_REPORT`. The `evdev` reader reconciles
    /// device state after a dropped synchronization report.
    ///
    /// # Errors
    ///
    /// Returns the read error, including device removal.
    pub fn read_events(&mut self, out: &mut Vec<InputEvent>) -> io::Result<()> {
        loop {
            match self.dev.fetch_events() {
                Ok(events) => out.extend(events),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e),
            }
        }
    }

    /// Returns whether every button is currently released.
    ///
    /// # Errors
    ///
    /// Returns the device state query error.
    pub fn is_idle(&self) -> io::Result<bool> {
        Ok(self.dev.get_key_state()?.iter().next().is_none())
    }
}

impl AsFd for EvdevPointer {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.dev.as_fd()
    }
}

impl Pointer for EvdevPointer {
    fn supports_forwarding(&self) -> bool {
        Self::supports_forwarding(self)
    }
    fn is_idle(&self) -> io::Result<bool> {
        Self::is_idle(self)
    }
    fn grab(&mut self) -> io::Result<()> {
        self.dev.grab()
    }
    fn read_events(&mut self, out: &mut Vec<PointerEvent>) -> io::Result<()> {
        let mut raw = Vec::new();
        self.read_events(&mut raw)?;
        out.extend(raw.into_iter().map(|event| PointerEvent {
            kind: event.event_type().0,
            code: event.code(),
            value: event.value(),
        }));
        Ok(())
    }
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
    pointer: Option<UinputPointer>,
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
        Ok(Self { dev, pointer: None })
    }

    /// Creates pointer output before the daemon opens or grabs devices.
    /// # Errors
    /// Returns the virtual pointer creation error.
    pub fn enable_pointer(&mut self) -> io::Result<()> {
        self.pointer = Some(UinputPointer::create()?);
        Ok(())
    }
}

impl Sink for UinputKeyboard {
    fn has_pointer(&self) -> bool {
        self.pointer.is_some()
    }
    fn emit_pointer(&mut self, events: &[PointerEvent]) -> io::Result<()> {
        let pointer = self.pointer.as_mut().ok_or(io::ErrorKind::Unsupported)?;
        let raw: Vec<_> = events
            .iter()
            .map(|event| InputEvent::new(event.kind, event.code, event.value))
            .collect();
        pointer.emit_frame(&raw)
    }
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

/// Virtual relative pointer for forwarding events from grabbed mice.
#[derive(Debug)]
pub struct UinputPointer {
    dev: VirtualDevice,
}

impl UinputPointer {
    /// Creates the pointer before any physical device is grabbed.
    ///
    /// # Errors
    ///
    /// Returns the uinput error if its capabilities or device cannot be created.
    pub fn create() -> io::Result<Self> {
        let mut buttons = AttributeSet::<evdev::KeyCode>::new();
        for code in 0x110..=0x11f {
            buttons.insert(evdev::KeyCode::new(code));
        }
        let mut axes = AttributeSet::<RelativeAxisCode>::new();
        for code in (0..=9).chain(11..=12) {
            axes.insert(RelativeAxisCode(code));
        }
        let mut misc = AttributeSet::<MiscCode>::new();
        misc.insert(MiscCode::MSC_SCAN);
        let name = format!("Escape Pod virtual pointer {}", std::process::id());
        let dev = VirtualDevice::builder()?
            .name(name.as_bytes())
            .input_id(InputId::new(
                BusType(BUS_VIRTUAL),
                OWN_VENDOR,
                OWN_PRODUCT,
                1,
            ))
            .with_keys(&buttons)?
            .with_relative_axes(&axes)?
            .with_msc(&misc)?
            .build()?;
        Ok(Self { dev })
    }

    /// Writes one event frame, adding its synchronization report.
    ///
    /// The caller excludes synchronization events and mapped button events.
    ///
    /// # Errors
    ///
    /// Returns the uinput write error.
    pub fn emit_frame(&mut self, events: &[InputEvent]) -> io::Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        if events.iter().any(|event| !pointer_event_supported(event)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "pointer event cannot be forwarded",
            ));
        }
        self.dev.emit(events)
    }
}

fn pointer_event_supported(event: &InputEvent) -> bool {
    match event.event_type() {
        EventType::KEY => (0x110..=0x11f).contains(&event.code()),
        EventType::RELATIVE => (0..=9).contains(&event.code()) || (11..=12).contains(&event.code()),
        EventType::MISC => event.code() == MiscCode::MSC_SCAN.0,
        _ => false,
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::*;

    #[test]
    fn supports_relative_pointer_events_but_rejects_unsupported_types() {
        assert!(pointer_event_supported(&InputEvent::new(
            EventType::RELATIVE.0,
            0,
            -4
        )));
        assert!(pointer_event_supported(&InputEvent::new(
            EventType::RELATIVE.0,
            12,
            120
        )));
        assert!(pointer_event_supported(&InputEvent::new(
            EventType::KEY.0,
            0x110,
            1
        )));
        assert!(pointer_event_supported(&InputEvent::new(
            EventType::MISC.0,
            MiscCode::MSC_SCAN.0,
            1
        )));
        assert!(!pointer_event_supported(&InputEvent::new(
            EventType::ABSOLUTE.0,
            0,
            1
        )));
        assert!(!pointer_event_supported(&InputEvent::new(
            EventType::KEY.0,
            30,
            1
        )));
        assert!(!pointer_event_supported(&InputEvent::new(
            EventType::SYNCHRONIZATION.0,
            0,
            0
        )));
    }
}

// Rust guideline compliant 2026-05-18
