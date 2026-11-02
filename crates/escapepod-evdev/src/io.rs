// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device I/O seam: what the daemon needs from a keyboard, a pointer,
//! the virtual keyboard and the device opener, as traits, so the daemon's
//! logic is tested against fakes and the real path end to end.

use std::io;
use std::path::Path;

use escapepod_core::KeyEvent;

/// A kernel input event without its timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerEvent {
    /// Kernel event type.
    pub kind: u16,
    /// Kernel event code.
    pub code: u16,
    /// Event value.
    pub value: i32,
}

/// A keyboard the daemon reads and, once idle, grabs exclusively.
pub trait Keyboard {
    /// Takes the exclusive grab (`EVIOCGRAB`): from now on only this
    /// process receives the device's events.
    ///
    /// # Errors
    ///
    /// The ioctl's error, such as `EBUSY` when another process holds it.
    fn grab(&mut self) -> io::Result<()>;

    /// How many keys the device reports down right now.
    ///
    /// # Errors
    ///
    /// The ioctl's error; `ENODEV` once the device is gone.
    fn keys_down(&self) -> io::Result<usize>;

    /// Appends every key event waiting on the device to `out`, without
    /// blocking.
    ///
    /// # Errors
    ///
    /// The read error; `ENODEV` when the device was removed.
    fn read(&mut self, out: &mut Vec<KeyEvent>) -> io::Result<()>;
}

/// A pointer observed for presses or grabbed for configured button shortcuts.
pub trait Pointer {
    /// Whether all capabilities can be forwarded by the virtual pointer.
    fn supports_forwarding(&self) -> bool {
        false
    }
    /// Whether no button is held.
    /// # Errors
    /// Returns the device query error.
    fn is_idle(&self) -> io::Result<bool> {
        Err(io::ErrorKind::Unsupported.into())
    }
    /// Takes the exclusive grab.
    /// # Errors
    /// Returns the grab error.
    fn grab(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    /// Reads complete frames, including synchronization reports.
    /// # Errors
    /// Returns the device read error.
    fn read_events(&mut self, _out: &mut Vec<PointerEvent>) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    /// The number of button presses waiting, read without blocking.
    ///
    /// # Errors
    ///
    /// The read error; `ENODEV` when the device was removed.
    fn read_presses(&mut self) -> io::Result<usize>;
}

/// The virtual keyboard every output goes to.
pub trait Sink {
    /// Whether pointer output was created before any grab.
    fn has_pointer(&self) -> bool {
        false
    }
    /// Writes a pointer frame, excluding its synchronization report.
    /// # Errors
    /// Returns the output error.
    fn emit_pointer(&mut self, _events: &[PointerEvent]) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    /// Writes `events` and one synchronisation report.
    ///
    /// # Errors
    ///
    /// The write error.
    fn emit(&mut self, events: &[KeyEvent]) -> io::Result<()>;
}

/// Opens devices by path.
pub trait Opener {
    /// The keyboard type it opens.
    type Keyboard: Keyboard;
    /// The pointer type it opens.
    type Pointer: Pointer;

    /// Opens the keyboard at `path` for reading, without grabbing it.
    ///
    /// # Errors
    ///
    /// The open error, such as `EACCES`.
    fn keyboard(&mut self, path: &Path) -> io::Result<Self::Keyboard>;

    /// Opens the pointer at `path` for reading.
    ///
    /// # Errors
    ///
    /// The open error.
    fn pointer(&mut self, path: &Path) -> io::Result<Self::Pointer>;
}

// Rust guideline compliant 2026-05-18
