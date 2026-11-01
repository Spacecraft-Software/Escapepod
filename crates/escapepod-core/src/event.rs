// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Events in and out of the engine, and the engine's notion of time.

use std::time::Duration;

use crate::KeyCode;

/// What happened to a key: the three `EV_KEY` values of the kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyValue {
    /// The key went up (kernel value 0).
    Release,
    /// The key went down (kernel value 1).
    Press,
    /// The kernel's autorepeat for a key held down (kernel value 2).
    Repeat,
}

impl KeyValue {
    /// Converts a kernel `EV_KEY` value; `None` for any other number.
    #[must_use]
    pub const fn from_raw(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Release),
            1 => Some(Self::Press),
            2 => Some(Self::Repeat),
            _ => None,
        }
    }

    /// The kernel `EV_KEY` value.
    #[must_use]
    pub const fn raw(self) -> i32 {
        match self {
            Self::Release => 0,
            Self::Press => 1,
            Self::Repeat => 2,
        }
    }
}

/// One key event: a key and what happened to it.
///
/// # Examples
///
/// ```
/// use escapepod_core::{KeyCode, KeyEvent, KeyValue};
///
/// let ev = KeyEvent::press(KeyCode::ESC);
/// assert_eq!(ev.value, KeyValue::Press);
/// assert_eq!(KeyValue::from_raw(ev.value.raw()), Some(KeyValue::Press));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// The key.
    pub key: KeyCode,
    /// Press, release or autorepeat.
    pub value: KeyValue,
}

impl KeyEvent {
    /// A press of `key`.
    #[must_use]
    pub const fn press(key: KeyCode) -> Self {
        Self {
            key,
            value: KeyValue::Press,
        }
    }

    /// A release of `key`.
    #[must_use]
    pub const fn release(key: KeyCode) -> Self {
        Self {
            key,
            value: KeyValue::Release,
        }
    }

    /// An autorepeat of `key`.
    #[must_use]
    pub const fn repeat(key: KeyCode) -> Self {
        Self {
            key,
            value: KeyValue::Repeat,
        }
    }
}

/// An input to one [`Engine`](crate::Engine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// A key event from the engine's own keyboard.
    Key(KeyEvent),
    /// A pointer button press on any pointer device.
    PointerPress,
    /// A key press on another keyboard, which marks held dual-role keys
    /// used but is not this engine's to emit.
    ForeignPress,
}

/// A point on a monotonic clock, in microseconds from an arbitrary origin.
///
/// The engine never reads a clock: the caller passes the time of each
/// input, so every decision can be replayed exactly in a test.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use escapepod_core::Instant;
///
/// let press = Instant::from_micros(1_000);
/// let release = press.saturating_add(Duration::from_millis(150));
/// assert_eq!(release.saturating_since(press), Duration::from_millis(150));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(u64);

impl Instant {
    /// The instant `micros` microseconds after the origin.
    #[must_use]
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    /// Microseconds since the origin.
    #[must_use]
    pub const fn as_micros(self) -> u64 {
        self.0
    }

    /// The time from `earlier` to `self`, or zero if `earlier` is later.
    #[must_use]
    pub fn saturating_since(self, earlier: Self) -> Duration {
        Duration::from_micros(self.0.saturating_sub(earlier.0))
    }

    /// The instant `d` after `self`, saturating at the end of the clock.
    #[must_use]
    pub fn saturating_add(self, d: Duration) -> Self {
        let micros = u64::try_from(d.as_micros()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(micros))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_values_round_trip() {
        for v in [KeyValue::Release, KeyValue::Press, KeyValue::Repeat] {
            assert_eq!(KeyValue::from_raw(v.raw()), Some(v));
        }
        assert_eq!(KeyValue::from_raw(3), None);
    }

    #[test]
    fn instants_saturate() {
        let a = Instant::from_micros(10);
        let b = a.saturating_add(Duration::from_millis(1));
        assert_eq!(b.as_micros(), 1010);
        assert_eq!(b.saturating_since(a), Duration::from_millis(1));
        assert_eq!(a.saturating_since(b), Duration::ZERO);
        assert_eq!(b.saturating_add(Duration::MAX).as_micros(), u64::MAX);
    }
}

// Rust guideline compliant 2026-05-18
