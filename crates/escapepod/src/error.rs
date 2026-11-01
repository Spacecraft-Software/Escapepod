// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exit codes and the structured error every failing command reports.

use std::fmt;
use std::process;

use serde_json::{Map, Value};

/// The exit codes of CLI Standard §4, plus Escape Pod's one tool-specific
/// code. Implements: ESC-SRS-027
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// 0: success.
    Success,
    /// 1: general failure.
    Failure,
    /// 2: usage error; do not retry unchanged.
    Usage,
    /// 3: resource not found.
    NotFound,
    /// 4: permission denied or a device unavailable.
    Permission,
    /// 5: conflict.
    Conflict,
    /// 6: the emergency chord released every grab. Produced only by the
    /// emergency path (ESC-SRS-095); the service unit does not restart on
    /// it.
    Emergency,
}

impl Exit {
    /// The process exit status.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::Usage => 2,
            Self::NotFound => 3,
            Self::Permission => 4,
            Self::Conflict => 5,
            Self::Emergency => 6,
        }
    }

    /// Every exit code with its meaning, for `schema` and the manual.
    #[must_use]
    pub fn table() -> [(u8, &'static str); 7] {
        [
            (0, "SUCCESS: the command completed"),
            (
                1,
                "GENERAL_FAILURE: the command failed, for example the X server connection was lost",
            ),
            (
                2,
                "USAGE_ERROR: invalid arguments or configuration; fix the invocation, do not retry it unchanged",
            ),
            (
                3,
                "NOT_FOUND: a configuration file or device does not exist",
            ),
            (
                4,
                "PERMISSION_DENIED: no access to an input device or /dev/uinput, or no backend available",
            ),
            (
                5,
                "CONFLICT: another daemon already holds the status socket",
            ),
            (
                6,
                "EMERGENCY_RELEASE: the emergency chord released every grab; the service does not restart",
            ),
        ]
    }
}

impl From<Exit> for process::ExitCode {
    fn from(e: Exit) -> Self {
        Self::from(e.code())
    }
}

/// A failed command: everything CLI Standard §2 puts in the `error` object.
///
/// The hint is a command the caller can run without elevated privileges,
/// or `human:` followed by what a person must do (ESC-SRS-025).
#[derive(Clone, Debug, PartialEq)]
pub struct AppError {
    code: &'static str,
    exit: Exit,
    message: String,
    hint: String,
    extra: Map<String, Value>,
}

impl AppError {
    /// A new error with its upper-snake-case code, exit code, one-sentence
    /// message and hint.
    pub fn new(
        code: &'static str,
        exit: Exit,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            code,
            exit,
            message: message.into(),
            hint: hint.into(),
            extra: Map::new(),
        }
    }

    /// A usage error (`INVALID_ARGUMENT`, exit 2).
    pub fn invalid(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::new("INVALID_ARGUMENT", Exit::Usage, message, hint)
    }

    /// Adds an extended field next to `message` (CLI Standard §3).
    #[must_use]
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.extra.insert(key.to_owned(), value.into());
        self
    }

    /// The exit code.
    #[must_use]
    pub fn exit(&self) -> Exit {
        self.exit
    }

    /// The one-sentence message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The hint.
    #[must_use]
    pub fn hint(&self) -> &str {
        &self.hint
    }

    /// The `{"error": {...}}` object for machine mode.
    #[must_use]
    pub fn to_json(&self, command: &str, timestamp: &str) -> Value {
        let mut e = Map::new();
        e.insert("code".into(), self.code.into());
        e.insert("exit_code".into(), self.exit.code().into());
        e.insert("message".into(), self.message.clone().into());
        e.insert("hint".into(), self.hint.clone().into());
        for (k, v) in &self.extra {
            e.insert(k.clone(), v.clone());
        }
        e.insert("timestamp".into(), timestamp.into());
        e.insert("command".into(), command.into());
        e.insert(
            "docs_url".into(),
            "https://EscapePod.SpacecraftSoftware.org/manual/#Invoking".into(),
        );
        let mut top = Map::new();
        top.insert("error".into(), Value::Object(e));
        Value::Object(top)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: ESC-SRS-027
    #[test]
    fn exit_codes_follow_the_cli_standard() {
        let codes: Vec<u8> = [
            Exit::Success,
            Exit::Failure,
            Exit::Usage,
            Exit::NotFound,
            Exit::Permission,
            Exit::Conflict,
            Exit::Emergency,
        ]
        .iter()
        .map(|e| e.code())
        .collect();
        assert_eq!(codes, [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(Exit::table().map(|(c, _)| c), [0, 1, 2, 3, 4, 5, 6]);
    }

    // Verifies: ESC-SRS-025
    #[test]
    fn error_object_has_every_field() {
        let e = AppError::invalid("bad", "escapepod schema").with("line", 3);
        let v = e.to_json("escapepod config check", "2026-11-01T00:00:00Z");
        let o = &v["error"];
        for key in [
            "code",
            "exit_code",
            "message",
            "hint",
            "timestamp",
            "command",
            "docs_url",
            "line",
        ] {
            assert!(o.get(key).is_some(), "{key} missing");
        }
        assert_eq!(o["exit_code"], 2);
    }
}

// Rust guideline compliant 2026-05-18
