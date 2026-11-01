// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device I/O seam: what the daemon needs from a keyboard, a pointer,
//! the virtual keyboard and the device opener, as traits, so the daemon's
//! logic is tested against fakes and the real path end to end.

use std::io;
use std::path::Path;

use escapepod_core::KeyEvent;

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

/// A pointer the daemon reads, never grabs, for button presses.
pub trait Pointer {
    /// The number of button presses waiting, read without blocking.
    ///
    /// # Errors
    ///
    /// The read error; `ENODEV` when the device was removed.
    fn read_presses(&mut self) -> io::Result<usize>;
}

/// The virtual keyboard every output goes to.
pub trait Sink {
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
