// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The mapping rules one engine applies.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::Duration;

use crate::KeyCode;

/// The tap timeout used when the configuration sets none (ESC-SRS-019).
///
/// 200 ms covers a deliberate tap and excludes an abandoned hold. This is
/// also xcape's default.
pub const DEFAULT_TAP_TIMEOUT: Duration = Duration::from_millis(200);

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
    protected: BTreeSet<KeyCode>,
    tap_timeout: Option<Duration>,
}

impl RulesBuilder {
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
    /// remap), or when the tap timeout is zero.
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
        Ok(Rules {
            dual,
            remap,
            protected: self.protected,
            tap_timeout,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorKind {
    TwoRoles(KeyCode),
    ZeroTimeout,
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
            ErrorKind::ZeroTimeout => None,
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
}

// Rust guideline compliant 2026-05-18
