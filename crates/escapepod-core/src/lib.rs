// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pure tap/hold engine of Escape Pod.
//!
//! This crate turns a stream of key events into the stream the virtual
//! keyboard emits. It decides when a dual-role key is a tap and when it is a
//! hold, and it applies one-to-one remaps. It performs no I/O and reads no
//! clock: the backends feed it events and the current monotonic time, which
//! keeps every decision deterministic and testable.
//!
//! - [`Engine`] is the state machine of one keyboard under one set of
//!   [`Rules`].
//! - [`Session`] holds one engine per grabbed keyboard, the emergency
//!   [`Chord`], and the [`HeldKeys`] ledger every exit path empties.
//! - [`KeyCode`] parses kernel and X keysym key names.
//!
//! # Examples
//!
//! ```
//! use escapepod_core::{DeviceId, Instant, KeyCode, KeyEvent, Rules, Session};
//!
//! let rules = Rules::builder()
//!     .dual_role(KeyCode::CAPSLOCK, KeyCode::LEFTCTRL, KeyCode::ESC)
//!     .build()?;
//! let mut session = Session::default();
//! session.add_device(DeviceId(0), rules);
//! let mut out = Vec::new();
//! session.on_key(DeviceId(0), KeyEvent::press(KeyCode::CAPSLOCK), Instant::from_micros(0), &mut out);
//! assert_eq!(out, [KeyEvent::press(KeyCode::LEFTCTRL)]);
//! # Ok::<(), escapepod_core::RulesError>(())
//! ```

// Implements: ESC-SRS-021
#![forbid(unsafe_code)]

mod chord;
mod engine;
mod event;
mod key;
mod keytable;
mod rules;
mod session;

#[doc(inline)]
pub use chord::{Chord, DEFAULT_CHORD, EmptyChordError};
#[doc(inline)]
pub use engine::{Decision, Engine};
#[doc(inline)]
pub use event::{Input, Instant, KeyEvent, KeyValue};
#[doc(inline)]
pub use key::KeyCode;
#[doc(inline)]
pub use rules::{DEFAULT_TAP_TIMEOUT, DualRole, Role, Rules, RulesBuilder, RulesError};
#[doc(inline)]
pub use session::{DeviceId, HeldKeys, Session, Step};

// Rust guideline compliant 2026-05-18
