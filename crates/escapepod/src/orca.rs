// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Orca screen-reader modifier discovery (manual, `Design` → `Screen
//! readers`).
//!
//! Orca keeps its modifier keys in `$XDG_DATA_HOME/orca/user-settings.conf`
//! (JSON) as X keysym names, under `general.orcaModifierKeys` and under each
//! profile's `orcaModifierKeys`. A missing or unreadable file means no Orca
//! modifier.

use std::path::{Path, PathBuf};

use escapepod_core::KeyCode;
use serde_json::Value;

use crate::output::Env;

/// The Orca settings file for this user, if a data directory is known.
pub fn settings_path(env: &Env) -> Option<PathBuf> {
    env.var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env.var("HOME").map(|h| Path::new(h).join(".local/share")))
        .map(|d| d.join("orca/user-settings.conf"))
}

/// The Orca modifier keys named in the settings file at `path`.
pub fn modifier_keys(path: &Path) -> Vec<KeyCode> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let profiles = v.get("profiles").and_then(Value::as_object);
    let lists = std::iter::once(v.pointer("/general/orcaModifierKeys")).chain(
        profiles
            .into_iter()
            .flat_map(|p| p.values().map(|p| p.get("orcaModifierKeys"))),
    );
    let mut keys: Vec<KeyCode> = lists
        .flatten()
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(KeyCode::from_name)
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_general_and_profile_modifiers() {
        let dir = std::env::temp_dir().join(format!("escapepod-orca-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("orca")).unwrap();
        let file = dir.join("orca/user-settings.conf");
        std::fs::write(
            &file,
            r#"{"general": {"orcaModifierKeys": ["Insert", "KP_Insert"]},
                "profiles": {"laptop": {"orcaModifierKeys": ["Caps_Lock"]}}}"#,
        )
        .unwrap();
        let keys = modifier_keys(&file);
        assert!(keys.contains(&KeyCode::CAPSLOCK));
        assert_eq!(keys.len(), 3);
        let env = Env::with(&[("XDG_DATA_HOME", dir.to_str().unwrap())]);
        assert_eq!(settings_path(&env), Some(file));
        assert!(modifier_keys(Path::new("/nonexistent/orca.conf")).is_empty());
    }
}

// Rust guideline compliant 2026-05-18
