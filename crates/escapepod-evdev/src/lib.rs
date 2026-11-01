// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The evdev backend of Escape Pod.
//!
//! This crate is the only part of Escape Pod that touches `/dev/input` and
//! `/dev/uinput`. [`scan`] lists devices from sysfs without opening any of
//! them; the grab, the virtual keyboard and hotplug build on it.

mod discover;

#[doc(inline)]
pub use discover::{DeviceInfo, DeviceKind, OWN_NAME_PREFIX, OWN_PRODUCT, OWN_VENDOR, scan};

// Rust guideline compliant 2026-05-18
