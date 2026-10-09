// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pure tap/hold engine of Escape Pod.
//!
//! This crate turns a stream of key events into the stream the virtual keyboard
//! emits. It decides when a dual-role key is a tap and when it is a hold, and it
//! applies one-to-one remaps. It performs no I/O and reads no clock: the
//! backends feed it events and the current monotonic time, which keeps every
//! decision deterministic and testable (PLAN P-009, P-010).
//!
//! The engine lands in M1; this is the M0 workspace skeleton.

#![forbid(unsafe_code)]

// Rust guideline compliant 2026-05-18
