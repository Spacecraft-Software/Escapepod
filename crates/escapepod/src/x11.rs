// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `daemon run` on the X11 backend: RECORD to observe, XTEST to inject the
//! tap key. It opens no input device and needs no `/dev/uinput`
//! (ESC-SRS-067).

use escapepod_core::KeyCode;
use escapepod_x11::{Backend, RemapError, Tapper};

use crate::app::Io;
use crate::config::Config;
use crate::error::{AppError, Exit};
use crate::output::{Mode, Severity};

/// The error for a remap the X11 backend cannot make, with a
/// `setxkbmap -option` hint (ESC-SRS-101). Implements: ESC-SRS-101
pub fn remap_error(e: RemapError) -> AppError {
    let hint = if (e.key, e.target) == (KeyCode::CAPSLOCK, KeyCode::LEFTCTRL) {
        "setxkbmap -option ctrl:nocaps".to_owned()
    } else {
        format!(
            "human: remap {} to {} with setxkbmap -option (see xkeyboard-config(7)), then map {} as a dual-role key",
            e.key, e.target, e.target
        )
    };
    AppError::new("X11_REMAP_UNSUPPORTED", Exit::Usage, e.to_string(), hint)
}

/// `daemon run --backend x11`, or the auto-selected fallback.
/// Implements: ESC-SRS-067
///
/// # Errors
///
/// `X11_REMAP_UNSUPPORTED` (exit 2) for a remap; `BACKEND_UNAVAILABLE`
/// (exit 4) without a reachable display; `DISPLAY_LOST` (exit 1) when the
/// connection drops.
pub fn run(cfg: &Config, mode: &Mode, io: &mut Io<'_>) -> Result<Exit, AppError> {
    let mut tapper = Tapper::new(cfg.global.clone()).map_err(remap_error)?;
    if !cfg.devices.is_empty() {
        mode.diag(
            io.err,
            Severity::Warn,
            "DEVICE_RULES_IGNORED",
            "the X11 backend cannot tell keyboards apart, so [[device]] rules are ignored",
            None,
        );
    }
    let Some(display) = io.env.var("DISPLAY").map(str::to_owned) else {
        return Err(unavailable("DISPLAY is not set"));
    };
    let backend = Backend::connect(&display).map_err(|e| unavailable(&e.to_string()))?;
    mode.diag(
        io.err,
        Severity::Info,
        "BACKEND_SELECTED",
        &format!("X11 backend on display {display}"),
        None,
    );
    match backend.run(&mut tapper) {
        Ok(()) => Ok(Exit::Success),
        // Implements: ESC-SRS-068, ESC-SRS-102
        Err(e) => Err(AppError::new(
            "DISPLAY_LOST",
            Exit::Failure,
            format!("{e}"),
            "human: restart the X session, then run escapepod daemon run --backend x11 --yes",
        )
        .with("display", e.display().to_owned())),
    }
}

/// The error for an X11 backend that cannot start (ESC-SRS-100).
pub fn unavailable(why: &str) -> AppError {
    AppError::new(
        "BACKEND_UNAVAILABLE",
        Exit::Permission,
        format!("the X11 backend is unavailable: {why}"),
        "escapepod daemon run --backend evdev --dry-run",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: ESC-SRS-101
    #[test]
    fn remap_hint_names_setxkbmap_option() {
        let caps = remap_error(RemapError {
            key: KeyCode::CAPSLOCK,
            target: KeyCode::LEFTCTRL,
        });
        assert_eq!(caps.hint(), "setxkbmap -option ctrl:nocaps");
        let other = remap_error(RemapError {
            key: KeyCode::ESC,
            target: KeyCode::CAPSLOCK,
        });
        assert!(other.hint().starts_with("human:") && other.hint().contains("setxkbmap -option"));
        assert_eq!(caps.exit(), Exit::Usage);
    }
}

// Rust guideline compliant 2026-05-18
