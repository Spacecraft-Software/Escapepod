// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The emergency chord: physical keys which, held together, release every
//! grab.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{KeyCode, KeyEvent, KeyValue};

/// The default emergency chord: Backspace, Escape and Enter
/// (ESC-SRS-049). Three keys far apart that no shortcut uses together.
pub const DEFAULT_CHORD: [KeyCode; 3] = [KeyCode::BACKSPACE, KeyCode::ESC, KeyCode::ENTER];

/// Tracks the physical keys of every device and reports when every key of
/// the emergency chord is down at once.
///
/// It sees physical keys before any mapping, so a broken configuration
/// cannot hide the chord.
///
/// # Examples
///
/// ```
/// use escapepod_core::{Chord, KeyCode, KeyEvent};
///
/// let mut chord = Chord::default();
/// assert!(!chord.observe(KeyEvent::press(KeyCode::BACKSPACE)));
/// assert!(!chord.observe(KeyEvent::press(KeyCode::ESC)));
/// assert!(chord.observe(KeyEvent::press(KeyCode::ENTER)));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    keys: BTreeSet<KeyCode>,
    down: BTreeMap<KeyCode, u32>,
    fired: bool,
}

impl Chord {
    /// A chord of `keys`.
    ///
    /// # Errors
    ///
    /// Returns [`EmptyChordError`] when `keys` is empty, since an empty
    /// chord would fire on the first event.
    pub fn new(keys: impl IntoIterator<Item = KeyCode>) -> Result<Self, EmptyChordError> {
        let keys: BTreeSet<KeyCode> = keys.into_iter().collect();
        if keys.is_empty() {
            return Err(EmptyChordError);
        }
        Ok(Self {
            keys,
            down: BTreeMap::new(),
            fired: false,
        })
    }

    /// The keys of the chord, in key-code order.
    pub fn keys(&self) -> impl Iterator<Item = KeyCode> + '_ {
        self.keys.iter().copied()
    }

    /// Observes one physical key event from any device; returns `true`
    /// once, on the event that completes the chord.
    pub fn observe(&mut self, ev: KeyEvent) -> bool {
        if !self.keys.contains(&ev.key) {
            return false;
        }
        match ev.value {
            KeyValue::Press => *self.down.entry(ev.key).or_insert(0) += 1,
            KeyValue::Release => {
                if let Some(n) = self.down.get_mut(&ev.key) {
                    *n = n.saturating_sub(1);
                    if *n == 0 {
                        self.down.remove(&ev.key);
                    }
                }
                self.fired = false;
            }
            KeyValue::Repeat => {}
        }
        if !self.fired && self.down.len() == self.keys.len() {
            self.fired = true;
            return true;
        }
        false
    }

    /// Forgets every key held, as when a device goes away.
    pub fn clear(&mut self) {
        self.down.clear();
        self.fired = false;
    }
}

impl Default for Chord {
    fn default() -> Self {
        Self {
            keys: DEFAULT_CHORD.into_iter().collect(),
            down: BTreeMap::new(),
            fired: false,
        }
    }
}

/// An emergency chord was configured with no keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyChordError;

impl fmt::Display for EmptyChordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the emergency chord has no keys")
    }
}

impl std::error::Error for EmptyChordError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> KeyCode {
        KeyCode::from_name(name).unwrap_or(KeyCode::ESC)
    }

    #[test]
    fn default_chord_fires_once_all_three_are_down() {
        let mut c = Chord::default();
        assert!(!c.observe(KeyEvent::press(KeyCode::BACKSPACE)));
        assert!(!c.observe(KeyEvent::press(KeyCode::ESC)));
        assert!(!c.observe(KeyEvent::press(key("KEY_A"))));
        assert!(c.observe(KeyEvent::press(KeyCode::ENTER)));
        assert!(!c.observe(KeyEvent::repeat(KeyCode::ENTER)));
        assert!(!c.observe(KeyEvent::release(KeyCode::ENTER)));
        assert!(c.observe(KeyEvent::press(KeyCode::ENTER)));
    }

    // Verifies: ESC-SRS-049
    #[test]
    fn chord_is_replaceable() {
        let (f1, f2) = (key("KEY_F1"), key("KEY_F2"));
        let mut c = Chord::new([f1, f2]).unwrap_or_default();
        assert_eq!(c.keys().collect::<Vec<_>>(), [f1, f2]);
        assert!(!c.observe(KeyEvent::press(KeyCode::BACKSPACE)));
        assert!(!c.observe(KeyEvent::press(KeyCode::ESC)));
        assert!(!c.observe(KeyEvent::press(KeyCode::ENTER)));
        assert!(!c.observe(KeyEvent::press(f1)));
        assert!(c.observe(KeyEvent::press(f2)));
        assert_eq!(Chord::new([]), Err(EmptyChordError));
    }

    #[test]
    fn release_and_clear_reset_the_chord() {
        let mut c = Chord::new([KeyCode::ESC]).unwrap_or_default();
        assert!(c.observe(KeyEvent::press(KeyCode::ESC)));
        c.clear();
        assert!(!c.observe(KeyEvent::release(KeyCode::ESC)));
        assert!(c.observe(KeyEvent::press(KeyCode::ESC)));
    }
}

// Rust guideline compliant 2026-05-18
