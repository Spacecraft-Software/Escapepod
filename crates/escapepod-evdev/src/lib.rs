// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The evdev backend of Escape Pod.
//!
//! This crate is the only part of Escape Pod that touches `/dev/input` and
//! `/dev/uinput`.
//!
//! - [`scan`] and [`info`] describe devices from sysfs without opening any.
//! - The [`Keyboard`], [`Pointer`], [`Sink`] and [`Opener`] traits are the
//!   I/O seam the daemon is written against.
//! - [`EvdevOpener`] and [`UinputKeyboard`] are the real implementations;
//!   the `fake` feature adds fakes for tests.

mod discover;
#[cfg(feature = "fake")]
pub mod fake;
mod io;
mod real;

#[doc(inline)]
pub use discover::{DeviceInfo, DeviceKind, OWN_NAME_PREFIX, OWN_PRODUCT, OWN_VENDOR, info, scan};
#[doc(inline)]
pub use io::{Keyboard, Opener, Pointer, Sink};
#[doc(inline)]
pub use real::{EvdevKeyboard, EvdevOpener, EvdevPointer, UinputKeyboard};

// Rust guideline compliant 2026-05-18
