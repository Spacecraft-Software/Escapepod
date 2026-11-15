// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The mapping rules one engine applies.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::Duration;

use crate::{Direction, KeyCode};

/// The tap timeout used when the configuration sets none (ESC-SRS-019).
///
/// 200 ms covers a deliberate tap and excludes an abandoned hold. This is
/// also xcape's default.
pub const DEFAULT_TAP_TIMEOUT: Duration = Duration::from_millis(200);

/// The swipe threshold used when a gesture rule names none (ESC-SRS-128).
pub const DEFAULT_GESTURE_THRESHOLD_MM: u8 = 20;

/// The shortest swipe threshold a gesture rule may name (ESC-SRS-127).
pub const MIN_GESTURE_THRESHOLD_MM: u8 = 5;

/// The longest swipe threshold a gesture rule may name (ESC-SRS-127).
pub const MAX_GESTURE_THRESHOLD_MM: u8 = 100;

/// One gesture rule: the shortcut a swipe sends, and how far it must travel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GestureRule {
    /// The shortcut, pressed in this order and released in reverse.
    pub keys: Vec<KeyCode>,
    /// The travel a swipe needs along its direction, in millimetres.
    pub threshold_mm: u8,
}

impl GestureRule {
    /// Whether a swipe that travelled `travel_um` micrometres reaches the
    /// threshold.
    #[must_use]
    pub fn reached_by(&self, travel_um: u64) -> bool {
        travel_um >= u64::from(self.threshold_mm) * 1000
    }
}

/// One dual-role key: tapped it sends `tap`, held it acts as `hold`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DualRole {
    /// The physical key.
    pub key: KeyCode,
    /// The key emitted while `key` is held, at once on its press.
    pub hold: KeyCode,
    /// The key tapped on a lone, quick release of `key`.
    pub tap: KeyCode,
}

/// The validated rules of one engine: dual-role keys, remaps, protected
/// keys and the tap timeout.
///
/// Every physical key has at most one role. A protected key overrides any
/// other role given to it, because a protected key is one a screen reader
/// relies on (ESC-SRS-020).
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use escapepod_core::{KeyCode, Role, Rules};
///
/// let rules = Rules::builder()
///     .dual_role(KeyCode::CAPSLOCK, KeyCode::LEFTCTRL, KeyCode::ESC)
///     .tap_timeout(Duration::from_millis(150))
///     .build()?;
/// assert!(rules.is_managed(KeyCode::CAPSLOCK));
/// assert_eq!(rules.role(KeyCode::ESC), Role::Other);
/// # Ok::<(), escapepod_core::RulesError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    dual: BTreeMap<KeyCode, DualRole>,
    remap: BTreeMap<KeyCode, KeyCode>,
    buttons: BTreeMap<KeyCode, Vec<KeyCode>>,
    gestures: BTreeMap<(u8, Direction), GestureRule>,
    protected: BTreeSet<KeyCode>,
    tap_timeout: Duration,
}

/// The role a physical key has under a set of [`Rules`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A dual-role key.
    DualRole(DualRole),
    /// A remap source, with its target.
    Remap(KeyCode),
    /// A protected key, emitted unchanged.
    Protected,
    /// Any other key, emitted unchanged.
    Other,
}

impl Rules {
    /// Starts a set of rules with no mapping and the default tap timeout.
    #[must_use]
    pub fn builder() -> RulesBuilder {
        RulesBuilder::default()
    }

    /// The role of physical key `key`.
    #[must_use]
    pub fn role(&self, key: KeyCode) -> Role {
        if self.protected.contains(&key) {
            Role::Protected
        } else if let Some(d) = self.dual.get(&key) {
            Role::DualRole(*d)
        } else if let Some(t) = self.remap.get(&key) {
            Role::Remap(*t)
        } else {
            Role::Other
        }
    }

    /// Whether `key` is managed: a dual-role key or a remap source that
    /// is not protected.
    #[must_use]
    pub fn is_managed(&self, key: KeyCode) -> bool {
        matches!(self.role(key), Role::DualRole(_) | Role::Remap(_))
    }

    /// The tap timeout.
    #[must_use]
    pub fn tap_timeout(&self) -> Duration {
        self.tap_timeout
    }

    /// The dual-role keys, in key-code order.
    pub fn dual_roles(&self) -> impl Iterator<Item = &DualRole> {
        self.dual.values()
    }

    /// The remaps as `(source, target)`, in source key-code order.
    pub fn remaps(&self) -> impl Iterator<Item = (KeyCode, KeyCode)> + '_ {
        self.remap.iter().map(|(&s, &t)| (s, t))
    }

    /// The shortcut mapped to a pointer button, if any.
    #[must_use]
    pub fn button(&self, button: KeyCode) -> Option<&[KeyCode]> {
        self.buttons.get(&button).map(Vec::as_slice)
    }

    /// Button mappings in button-code order.
    pub fn buttons(&self) -> impl Iterator<Item = (KeyCode, &[KeyCode])> + '_ {
        self.buttons
            .iter()
            .map(|(&button, keys)| (button, keys.as_slice()))
    }

    /// The rule for a swipe of `fingers` fingers in `direction`, if any.
    #[must_use]
    pub fn gesture(&self, fingers: u8, direction: Direction) -> Option<&GestureRule> {
        self.gestures.get(&(fingers, direction))
    }

    /// Gesture rules in finger-count, then direction order.
    pub fn gestures(&self) -> impl Iterator<Item = (u8, Direction, &GestureRule)> + '_ {
        self.gestures
            .iter()
            .map(|(&(fingers, direction), rule)| (fingers, direction, rule))
    }

    /// The protected keys, in key-code order.
    pub fn protected(&self) -> impl Iterator<Item = KeyCode> + '_ {
        self.protected.iter().copied()
    }

    /// The managed keys that are also protected, so their mapping is
    /// inactive. The caller warns about them (ESC-SRS-093).
    pub fn shadowed(&self) -> impl Iterator<Item = KeyCode> + '_ {
        self.protected
            .iter()
            .copied()
            .filter(|k| self.dual.contains_key(k) || self.remap.contains_key(k))
    }
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            dual: BTreeMap::new(),
            remap: BTreeMap::new(),
            buttons: BTreeMap::new(),
            gestures: BTreeMap::new(),
            protected: BTreeSet::new(),
            tap_timeout: DEFAULT_TAP_TIMEOUT,
        }
    }
}

/// Builds [`Rules`]; see [`Rules::builder`].
#[derive(Clone, Debug, Default)]
pub struct RulesBuilder {
    dual: Vec<DualRole>,
    remap: Vec<(KeyCode, KeyCode)>,
    buttons: Vec<(KeyCode, Vec<KeyCode>)>,
    gestures: Vec<(u8, Direction, Vec<KeyCode>, Option<u8>)>,
    protected: BTreeSet<KeyCode>,
    tap_timeout: Option<Duration>,
}

impl RulesBuilder {
    /// Maps a swipe of `fingers` fingers in `direction` to an ordered key
    /// shortcut, with a threshold in millimetres (the default when `None`).
    #[must_use]
    pub fn gesture(
        mut self,
        fingers: u8,
        direction: Direction,
        keys: Vec<KeyCode>,
        threshold_mm: Option<u8>,
    ) -> Self {
        self.gestures.push((fingers, direction, keys, threshold_mm));
        self
    }

    /// Maps a pointer button to an ordered key shortcut.
    #[must_use]
    pub fn button(mut self, button: KeyCode, keys: Vec<KeyCode>) -> Self {
        self.buttons.push((button, keys));
        self
    }
    /// Adds a dual-role key.
    #[must_use]
    pub fn dual_role(mut self, key: KeyCode, hold: KeyCode, tap: KeyCode) -> Self {
        self.dual.push(DualRole { key, hold, tap });
        self
    }

    /// Adds a remap from `source` to `target`.
    #[must_use]
    pub fn remap(mut self, source: KeyCode, target: KeyCode) -> Self {
        self.remap.push((source, target));
        self
    }

    /// Protects `key`.
    #[must_use]
    pub fn protect(mut self, key: KeyCode) -> Self {
        self.protected.insert(key);
        self
    }

    /// Sets the tap timeout; without it [`DEFAULT_TAP_TIMEOUT`] applies.
    #[must_use]
    pub fn tap_timeout(mut self, timeout: Duration) -> Self {
        self.tap_timeout = Some(timeout);
        self
    }

    /// Validates the rules.
    ///
    /// # Errors
    ///
    /// Returns a [`RulesError`] when one physical key is given two roles
    /// (two dual-role entries, two remaps, or a dual-role entry and a
    /// remap), when the tap timeout is zero, when a button is repeated or
    /// its shortcut has fewer than one, more than eight, or repeated keys,
    /// or when a gesture rule names a finger count other than three or four,
    /// a threshold outside 5 to 100 mm, such a shortcut, or the same finger
    /// count and direction as another.
    pub fn build(self) -> Result<Rules, RulesError> {
        let tap_timeout = self.tap_timeout.unwrap_or(DEFAULT_TAP_TIMEOUT);
        if tap_timeout.is_zero() {
            return Err(RulesError::new(ErrorKind::ZeroTimeout));
        }
        let mut dual = BTreeMap::new();
        for d in self.dual {
            if dual.insert(d.key, d).is_some() {
                return Err(RulesError::new(ErrorKind::TwoRoles(d.key)));
            }
        }
        let mut remap = BTreeMap::new();
        for (s, t) in self.remap {
            if dual.contains_key(&s) || remap.insert(s, t).is_some() {
                return Err(RulesError::new(ErrorKind::TwoRoles(s)));
            }
        }
        let mut buttons = BTreeMap::new();
        for (button, keys) in self.buttons {
            if keys.is_empty()
                || keys.len() > 8
                || keys.iter().collect::<BTreeSet<_>>().len() != keys.len()
            {
                return Err(RulesError::new(ErrorKind::InvalidShortcut(button)));
            }
            if buttons.insert(button, keys).is_some() {
                return Err(RulesError::new(ErrorKind::DuplicateButton(button)));
            }
        }
        // Implements: ESC-SRS-122, ESC-SRS-123, ESC-SRS-127, ESC-SRS-128
        let mut gestures = BTreeMap::new();
        for (fingers, direction, keys, threshold) in self.gestures {
            if fingers != 3 && fingers != 4 {
                return Err(RulesError::new(ErrorKind::GestureFingers(fingers)));
            }
            let threshold_mm = threshold.unwrap_or(DEFAULT_GESTURE_THRESHOLD_MM);
            if !(MIN_GESTURE_THRESHOLD_MM..=MAX_GESTURE_THRESHOLD_MM).contains(&threshold_mm) {
                return Err(RulesError::new(ErrorKind::GestureThreshold(threshold_mm)));
            }
            if keys.is_empty()
                || keys.len() > 8
                || keys.iter().collect::<BTreeSet<_>>().len() != keys.len()
            {
                return Err(RulesError::new(ErrorKind::GestureShortcut(
                    fingers, direction,
                )));
            }
            let rule = GestureRule { keys, threshold_mm };
            if gestures.insert((fingers, direction), rule).is_some() {
                return Err(RulesError::new(ErrorKind::DuplicateGesture(
                    fingers, direction,
                )));
            }
        }
        Ok(Rules {
            dual,
            remap,
            buttons,
            gestures,
            protected: self.protected,
            tap_timeout,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorKind {
    TwoRoles(KeyCode),
    ZeroTimeout,
    InvalidShortcut(KeyCode),
    DuplicateButton(KeyCode),
    GestureFingers(u8),
    GestureThreshold(u8),
    GestureShortcut(u8, Direction),
    DuplicateGesture(u8, Direction),
}

/// The rules given to [`RulesBuilder::build`] are not valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RulesError {
    kind: ErrorKind,
}

impl RulesError {
    fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    /// The physical key given more than one role, if that is the error.
    #[must_use]
    pub fn conflicting_key(&self) -> Option<KeyCode> {
        match self.kind {
            ErrorKind::TwoRoles(k) => Some(k),
            ErrorKind::ZeroTimeout
            | ErrorKind::InvalidShortcut(_)
            | ErrorKind::DuplicateButton(_)
            | ErrorKind::GestureFingers(_)
            | ErrorKind::GestureThreshold(_)
            | ErrorKind::GestureShortcut(..)
            | ErrorKind::DuplicateGesture(..) => None,
        }
    }

    /// Whether the error is a zero tap timeout.
    #[must_use]
    pub fn is_zero_timeout(&self) -> bool {
        self.kind == ErrorKind::ZeroTimeout
    }
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::TwoRoles(k) => write!(f, "key {k} is given more than one role"),
            ErrorKind::ZeroTimeout => f.write_str("the tap timeout is zero"),
            ErrorKind::InvalidShortcut(k) => {
                write!(f, "button {k} needs one to eight distinct shortcut keys")
            }
            ErrorKind::DuplicateButton(k) => write!(f, "button {k} is mapped more than once"),
            ErrorKind::GestureFingers(n) => {
                write!(f, "a gesture names {n} fingers; swipes use three or four")
            }
            ErrorKind::GestureThreshold(mm) => write!(
                f,
                "a gesture threshold of {mm} mm is outside {MIN_GESTURE_THRESHOLD_MM} to {MAX_GESTURE_THRESHOLD_MM} mm"
            ),
            ErrorKind::GestureShortcut(n, d) => {
                write!(
                    f,
                    "the {n}-finger {d} swipe needs one to eight distinct shortcut keys"
                )
            }
            ErrorKind::DuplicateGesture(n, d) => {
                write!(f, "the {n}-finger {d} swipe is mapped more than once")
            }
        }
    }
}

impl std::error::Error for RulesError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> KeyCode {
        KeyCode::from_name(name).unwrap_or(KeyCode::ESC)
    }

    // Verifies: ESC-SRS-019
    #[test]
    fn default_tap_timeout_is_200_ms() {
        assert_eq!(DEFAULT_TAP_TIMEOUT, Duration::from_millis(200));
        let rules = Rules::builder().build();
        assert_eq!(
            rules.map(|r| r.tap_timeout()),
            Ok(Duration::from_millis(200))
        );
    }

    #[test]
    fn one_role_per_key() {
        let caps = KeyCode::CAPSLOCK;
        let two_dual = Rules::builder()
            .dual_role(caps, KeyCode::LEFTCTRL, KeyCode::ESC)
            .dual_role(caps, KeyCode::LEFTCTRL, key("KEY_TAB"))
            .build();
        assert_eq!(two_dual.err().and_then(|e| e.conflicting_key()), Some(caps));
        let dual_and_remap = Rules::builder()
            .dual_role(caps, KeyCode::LEFTCTRL, KeyCode::ESC)
            .remap(caps, KeyCode::LEFTCTRL)
            .build();
        assert_eq!(
            dual_and_remap.err().and_then(|e| e.conflicting_key()),
            Some(caps)
        );
        let zero = Rules::builder().tap_timeout(Duration::ZERO).build();
        assert!(zero.is_err_and(|e| e.is_zero_timeout()));
    }

    // Verifies: ESC-SRS-020
    #[test]
    fn protection_overrides_every_role() {
        let caps = KeyCode::CAPSLOCK;
        let rules = Rules::builder()
            .dual_role(caps, KeyCode::LEFTCTRL, KeyCode::ESC)
            .remap(key("KEY_TAB"), key("KEY_ESC"))
            .protect(caps)
            .protect(key("KEY_TAB"))
            .build();
        let rules = rules.unwrap_or_default();
        assert_eq!(rules.role(caps), Role::Protected);
        assert_eq!(rules.role(key("KEY_TAB")), Role::Protected);
        assert!(!rules.is_managed(caps));
        assert_eq!(rules.shadowed().count(), 2);
    }

    fn swipe(fingers: u8, keys: Vec<KeyCode>, threshold: Option<u8>) -> Result<Rules, RulesError> {
        Rules::builder()
            .gesture(fingers, Direction::Left, keys, threshold)
            .build()
    }

    fn chord() -> Vec<KeyCode> {
        vec![key("KEY_LEFTMETA"), KeyCode::LEFTCTRL, key("KEY_RIGHT")]
    }

    // Verifies: ESC-SRS-122
    #[test]
    fn gestures_take_three_or_four_fingers() {
        swipe(3, chord(), None).unwrap();
        swipe(4, chord(), None).unwrap();
        for n in [0, 1, 2, 5] {
            let err = swipe(n, chord(), None).unwrap_err();
            assert!(err.to_string().contains("three or four"), "{err}");
        }
    }

    // Verifies: ESC-SRS-123
    #[test]
    fn gesture_shortcuts_need_one_to_eight_distinct_keys() {
        swipe(3, vec![], None).unwrap_err();
        let nine: Vec<KeyCode> = (1..=9u16)
            .filter_map(|n| KeyCode::from_name(&format!("KEY_F{n}")))
            .collect();
        assert_eq!(nine.len(), 9);
        swipe(3, nine, None).unwrap_err();
        swipe(3, vec![KeyCode::ESC, KeyCode::ESC], None).unwrap_err();
        swipe(3, vec![KeyCode::ESC], None).unwrap();
    }

    // Verifies: ESC-SRS-127, ESC-SRS-128
    #[test]
    fn gesture_thresholds_are_5_to_100_mm_with_a_20_mm_default() {
        let rules = swipe(3, chord(), None).unwrap_or_default();
        assert_eq!(
            rules.gesture(3, Direction::Left).map(|g| g.threshold_mm),
            Some(DEFAULT_GESTURE_THRESHOLD_MM)
        );
        assert_eq!(DEFAULT_GESTURE_THRESHOLD_MM, 20);
        for mm in [5, 100] {
            swipe(3, chord(), Some(mm)).unwrap();
        }
        for mm in [0, 4, 101, 255] {
            let err = swipe(3, chord(), Some(mm)).unwrap_err();
            assert!(err.to_string().contains("5 to 100 mm"), "{err}");
        }
    }

    #[test]
    fn a_swipe_is_mapped_once_and_reaches_its_threshold_in_micrometres() {
        let dup = Rules::builder()
            .gesture(3, Direction::Left, chord(), None)
            .gesture(3, Direction::Left, vec![KeyCode::ESC], None)
            .build()
            .unwrap_err();
        assert!(dup.to_string().contains("more than once"));
        let rules = swipe(4, chord(), Some(30)).unwrap_or_default();
        let rule = rules.gesture(4, Direction::Left);
        assert_eq!(rule.map(|g| g.reached_by(29_999)), Some(false));
        assert_eq!(rule.map(|g| g.reached_by(30_000)), Some(true));
        assert!(rules.gesture(4, Direction::Right).is_none());
        assert_eq!(rules.gestures().count(), 1);
    }
}

// Rust guideline compliant 2026-05-18
