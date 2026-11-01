// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The xcape mapping grammar of `--map`: `KEY=TAP[;KEY=TAP]...`.
//!
//! Each entry makes KEY a dual-role key whose hold key is KEY itself and
//! whose tap key is TAP (ESC-SRS-032). Key names are kernel names or the X
//! keysym names xcape uses. xcape's multi-key taps (`A|B`) are refused:
//! a tap sends exactly one key.

use std::fmt;

use escapepod_core::{DualRole, KeyCode};

/// A `--map` string that does not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapError {
    /// What is wrong, in one phrase.
    pub message: String,
    /// The one-based character position of the offending entry.
    pub position: usize,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at character {}", self.message, self.position)
    }
}

impl std::error::Error for MapError {}

/// Parses a `--map` value. Empty entries (a trailing `;`) are allowed.
///
/// # Errors
///
/// A [`MapError`] naming the first entry that is not `KEY=TAP` with two
/// known key names, or a map with no entry at all.
pub fn parse(map: &str) -> Result<Vec<DualRole>, MapError> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for entry in map.split(';') {
        let position = offset + 1;
        offset += entry.chars().count() + 1;
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let err = |message: String| MapError { message, position };
        let (key, tap) = trimmed
            .split_once('=')
            .ok_or_else(|| err(format!("entry `{trimmed}` is not KEY=TAP")))?;
        let (key, tap) = (key.trim(), tap.trim());
        if tap.contains('|') {
            return Err(err(format!(
                "multi-key tap `{tap}` is not supported; a tap sends one key"
            )));
        }
        if tap.contains('=') {
            return Err(err(format!("entry `{trimmed}` has more than one `=`")));
        }
        let k = KeyCode::from_name(key).ok_or_else(|| err(format!("unknown key name `{key}`")))?;
        let t = KeyCode::from_name(tap).ok_or_else(|| err(format!("unknown key name `{tap}`")))?;
        out.push(DualRole {
            key: k,
            hold: k,
            tap: t,
        });
    }
    if out.is_empty() {
        return Err(MapError {
            message: "the map has no KEY=TAP entry".into(),
            position: 1,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: &str) -> KeyCode {
        KeyCode::from_name(n).unwrap_or(KeyCode::ESC)
    }

    // Verifies: ESC-SRS-032
    #[test]
    fn xcape_strings_parse_as_dual_roles() {
        let m = parse("Control_L=Escape").unwrap_or_default();
        assert_eq!(
            m,
            [DualRole {
                key: KeyCode::LEFTCTRL,
                hold: KeyCode::LEFTCTRL,
                tap: KeyCode::ESC
            }]
        );
        let two = parse("Caps_Lock=Escape;Shift_L=Tab;").unwrap_or_default();
        assert_eq!(two.len(), 2);
        assert_eq!(
            two[1],
            DualRole {
                key: key("Shift_L"),
                hold: key("Shift_L"),
                tap: key("Tab")
            }
        );
        assert_eq!(
            parse(" KEY_CAPSLOCK = KEY_ESC ").map(|m| m[0].tap),
            Ok(KeyCode::ESC)
        );
    }

    #[test]
    fn bad_strings_name_the_entry() {
        assert_eq!(
            parse("Caps_Lock=Escape;Bogus=Tab").map_err(|e| e.position),
            Err(18)
        );
        assert!(parse("Shift_L=Shift_L|parenleft").is_err_and(|e| e.message.contains("multi-key")));
        parse("Control_L").unwrap_err();
        parse("a=b=c").unwrap_err();
        parse("").unwrap_err();
        parse(";;").unwrap_err();
    }
}

// Rust guideline compliant 2026-05-18
