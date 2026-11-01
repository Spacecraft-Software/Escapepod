// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The X11 fallback backend of Escape Pod.
//!
//! For a session without `/dev/input` access, this crate observes key events
//! through the X `RECORD` extension and injects tap keys through `XTEST`. It
//! supports dual-role keys only; remaps belong to `setxkbmap` on X11 (PLAN
//! P-032).
//!
//! The backend lands in M4; this is the M0 workspace skeleton.

// Rust guideline compliant 2026-05-18
