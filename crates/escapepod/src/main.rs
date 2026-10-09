// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `escapepod` daemon and command-line interface.
//!
//! The CLI lands in M2 and the daemon loop in M3; until then the binary only
//! says so and exits with status 1, so nothing mistakes the skeleton for a
//! working daemon.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "[ERROR] escapepod {}: not implemented yet (M0 workspace skeleton)",
        env!("CARGO_PKG_VERSION")
    );
    ExitCode::FAILURE
}

// Rust guideline compliant 2026-05-18
