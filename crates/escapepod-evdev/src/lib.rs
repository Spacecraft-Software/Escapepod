// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The evdev backend of Escape Pod.
//!
//! This crate reads keyboards through the kernel evdev interface, takes an
//! exclusive grab on the ones that match, and writes the engine's output to a
//! uinput virtual keyboard. It also watches udev for keyboards that come and go.
//! Device access sits behind traits so the grab, release and hotplug logic can
//! be tested with fakes (PLAN P-022).
//!
//! The backend lands in M3; this is the M0 workspace skeleton.

// Rust guideline compliant 2026-05-18
