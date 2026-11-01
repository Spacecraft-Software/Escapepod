// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The daemon, its status socket and `event monitor`. The evdev backend
//! and the event loop land in M3; until then these commands report that
//! no backend is built.

use std::path::Path;

use crate::app::Io;
use crate::cli::BackendArg;
use crate::config::Config;
use crate::error::{AppError, Exit};
use crate::output::Mode;

fn unavailable(what: &str) -> AppError {
    AppError::new(
        "BACKEND_UNAVAILABLE",
        Exit::Permission,
        format!("{what} is not available in this build: the backends land in milestone M3"),
        "escapepod daemon run --dry-run",
    )
}

/// `daemon run` after its guards.
pub fn run(
    _cfg: &Config,
    _backend: BackendArg,
    _mode: &Mode,
    _io: &mut Io<'_>,
) -> Result<Exit, AppError> {
    Err(unavailable("daemon run"))
}

/// `daemon status`.
pub fn status(_socket: Option<&Path>, _mode: &Mode, _io: &mut Io<'_>) -> Result<Exit, AppError> {
    Err(unavailable("daemon status"))
}

/// `event monitor`.
pub fn monitor(
    _cfg: &Config,
    _all_keys: bool,
    _mode: &Mode,
    _io: &mut Io<'_>,
) -> Result<Exit, AppError> {
    Err(unavailable("event monitor"))
}

// Rust guideline compliant 2026-05-18
