// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Key codes and their names.
//!
//! A [`KeyCode`] is a Linux input key code (`KEY_*` or `BTN_*` from
//! `linux/input-event-codes.h`). Names are accepted in three spellings: the
//! kernel name (`KEY_CAPSLOCK`), the kernel name without its prefix
//! (`capslock`), and the X keysym names that xcape mappings use
//! (`Caps_Lock`, `Control_L`, `Escape`).

use std::fmt;

use crate::keytable::KERNEL_NAMES;

/// A Linux input key code, such as `KEY_ESC` (1) or `KEY_LEFTCTRL` (29).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyCode(u16);

impl KeyCode {
    /// `KEY_ESC`, the default tap key.
    pub const ESC: Self = Self(1);
    /// `KEY_BACKSPACE`, part of the default emergency chord.
    pub const BACKSPACE: Self = Self(14);
    /// `KEY_ENTER`, part of the default emergency chord.
    pub const ENTER: Self = Self(28);
    /// `KEY_LEFTCTRL`, the usual hold key.
    pub const LEFTCTRL: Self = Self(29);
    /// `KEY_CAPSLOCK`, the usual dual-role key.
    pub const CAPSLOCK: Self = Self(58);

    /// The highest key code the kernel defines (`KEY_MAX`).
    pub const MAX: u16 = 0x2ff;

    /// Wraps a raw kernel key code, or `None` above `KEY_MAX`.
    #[must_use]
    pub const fn new(code: u16) -> Option<Self> {
        if code <= Self::MAX {
            Some(Self(code))
        } else {
            None
        }
    }

    /// The raw kernel key code.
    #[must_use]
    pub const fn code(self) -> u16 {
        self.0
    }

    /// The canonical kernel name, such as `KEY_CAPSLOCK`, when the kernel
    /// defines one for this code.
    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        KERNEL_NAMES
            .binary_search_by_key(&self.0, |&(code, _)| code)
            .ok()
            .map(|i| first_name_at(i, self.0))
    }

    /// Parses a key name in any of the accepted spellings; see the module
    /// documentation. Matching ignores ASCII case.
    ///
    /// # Examples
    ///
    /// ```
    /// use escapepod_core::KeyCode;
    /// assert_eq!(KeyCode::from_name("Caps_Lock"), Some(KeyCode::CAPSLOCK));
    /// assert_eq!(KeyCode::from_name("KEY_LEFTCTRL"), Some(KeyCode::LEFTCTRL));
    /// assert_eq!(KeyCode::from_name("esc"), Some(KeyCode::ESC));
    /// assert_eq!(KeyCode::from_name("no such key"), None);
    /// ```
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        if let Some(&(_, kernel)) = KEYSYMS.iter().find(|(sym, _)| *sym == name) {
            return Self::from_kernel_name(kernel);
        }
        if let Some(&(_, kernel)) = KEYSYMS
            .iter()
            .find(|(sym, _)| sym.eq_ignore_ascii_case(name))
        {
            return Self::from_kernel_name(kernel);
        }
        Self::from_kernel_name(name).or_else(|| {
            ["KEY_", "BTN_"]
                .iter()
                .find_map(|prefix| Self::from_kernel_name(&format!("{prefix}{name}")))
        })
    }

    fn from_kernel_name(name: &str) -> Option<Self> {
        KERNEL_NAMES
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(name))
            .map(|&(code, _)| Self(code))
    }
}

/// Walks back to the first table entry with `code`, which is the canonical
/// name when the header defines aliases.
fn first_name_at(mut i: usize, code: u16) -> &'static str {
    while i > 0 && KERNEL_NAMES[i - 1].0 == code {
        i -= 1;
    }
    KERNEL_NAMES[i].1
}

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "KEY_{}", self.0),
        }
    }
}

/// X keysym names used by xcape mappings, with the kernel key each one is
/// on a standard keyboard. Shifted keysyms (`parenleft`) are absent: a tap
/// sends one key, never a key with a modifier.
const KEYSYMS: &[(&str, &str)] = &[
    ("Escape", "KEY_ESC"),
    ("Control_L", "KEY_LEFTCTRL"),
    ("Control_R", "KEY_RIGHTCTRL"),
    ("Shift_L", "KEY_LEFTSHIFT"),
    ("Shift_R", "KEY_RIGHTSHIFT"),
    ("Alt_L", "KEY_LEFTALT"),
    ("Alt_R", "KEY_RIGHTALT"),
    ("ISO_Level3_Shift", "KEY_RIGHTALT"),
    ("Meta_L", "KEY_LEFTMETA"),
    ("Meta_R", "KEY_RIGHTMETA"),
    ("Super_L", "KEY_LEFTMETA"),
    ("Super_R", "KEY_RIGHTMETA"),
    ("Hyper_L", "KEY_LEFTMETA"),
    ("Caps_Lock", "KEY_CAPSLOCK"),
    ("Num_Lock", "KEY_NUMLOCK"),
    ("Scroll_Lock", "KEY_SCROLLLOCK"),
    ("Tab", "KEY_TAB"),
    ("Return", "KEY_ENTER"),
    ("KP_Enter", "KEY_KPENTER"),
    ("BackSpace", "KEY_BACKSPACE"),
    ("space", "KEY_SPACE"),
    ("Insert", "KEY_INSERT"),
    ("KP_Insert", "KEY_KP0"),
    ("Delete", "KEY_DELETE"),
    ("Home", "KEY_HOME"),
    ("End", "KEY_END"),
    ("Prior", "KEY_PAGEUP"),
    ("Page_Up", "KEY_PAGEUP"),
    ("Next", "KEY_PAGEDOWN"),
    ("Page_Down", "KEY_PAGEDOWN"),
    ("Up", "KEY_UP"),
    ("Down", "KEY_DOWN"),
    ("Left", "KEY_LEFT"),
    ("Right", "KEY_RIGHT"),
    ("Menu", "KEY_COMPOSE"),
    ("Print", "KEY_SYSRQ"),
    ("Pause", "KEY_PAUSE"),
    ("minus", "KEY_MINUS"),
    ("equal", "KEY_EQUAL"),
    ("bracketleft", "KEY_LEFTBRACE"),
    ("bracketright", "KEY_RIGHTBRACE"),
    ("semicolon", "KEY_SEMICOLON"),
    ("apostrophe", "KEY_APOSTROPHE"),
    ("grave", "KEY_GRAVE"),
    ("backslash", "KEY_BACKSLASH"),
    ("comma", "KEY_COMMA"),
    ("period", "KEY_DOT"),
    ("slash", "KEY_SLASH"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for &(code, _) in KERNEL_NAMES {
            let key = KeyCode(code);
            let name = key.name().unwrap_or_default();
            assert_eq!(KeyCode::from_name(name), Some(key), "{name}");
        }
    }

    #[test]
    fn keysyms_resolve() {
        for &(sym, kernel) in KEYSYMS {
            assert!(
                KeyCode::from_kernel_name(kernel).is_some(),
                "{sym} -> {kernel}"
            );
            assert!(KeyCode::from_name(sym).is_some(), "{sym}");
        }
        assert_eq!(KeyCode::from_name("a"), KeyCode::from_name("KEY_A"));
        assert_eq!(KeyCode::from_name("F12"), KeyCode::from_name("KEY_F12"));
        assert_eq!(KeyCode::from_name("left"), KeyCode::from_name("KEY_LEFT"));
    }

    #[test]
    fn bounds_and_display() {
        assert_eq!(KeyCode::new(0x300), None);
        assert_eq!(KeyCode::new(1), Some(KeyCode::ESC));
        assert_eq!(KeyCode::ESC.to_string(), "KEY_ESC");
        assert_eq!(KeyCode(0x2fe).to_string(), "KEY_766");
    }
}

// Rust guideline compliant 2026-05-18
