// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The X11 fallback backend (M4).

use crate::app::Io;
use crate::config::Config;
use crate::error::{AppError, Exit};
use crate::output::Mode;

/// `daemon run --backend x11`.
///
/// # Errors
///
/// `BACKEND_UNAVAILABLE` until the X11 backend lands in M4.
pub fn run(_cfg: &Config, _mode: &Mode, _io: &mut Io<'_>) -> Result<Exit, AppError> {
    Err(AppError::new(
        "BACKEND_UNAVAILABLE",
        Exit::Permission,
        "the X11 backend is not in this build yet (milestone M4)",
        "escapepod daemon run --backend evdev --dry-run",
    ))
}

// Rust guideline compliant 2026-05-18
