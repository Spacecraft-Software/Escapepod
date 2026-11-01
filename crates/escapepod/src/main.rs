// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `escapepod`: tap a key for Escape, hold it for Control.
//!
//! The command line follows the Spacecraft Software CLI Standard: JSON in
//! agent environments and pipes, structured errors with runnable hints, and
//! the canonical exit codes.

mod app;
mod cli;
mod config;
mod daemon;
mod error;
mod keymap;
mod monitor;
mod orca;
mod output;
mod schema;
mod service;
mod theme;
mod x11;

use std::process::ExitCode;

fn main() -> ExitCode {
    let env = output::Env::from_process();
    let mut out = std::io::stdout().lock();
    let mut err = std::io::stderr().lock();
    let mut io = app::Io {
        out: &mut out,
        err: &mut err,
        env: &env,
    };
    app::run(&std::env::args_os().collect::<Vec<_>>(), &mut io).into()
}

// Rust guideline compliant 2026-05-18
