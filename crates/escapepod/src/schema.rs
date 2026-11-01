// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `escapepod schema` and `escapepod describe`: the command surface as JSON
//! Schema Draft 2020-12 and as a compact capability manifest.

use serde_json::{Map, Value, json};

use crate::error::{AppError, Exit};

/// The JSON Schema dialect every schema document declares.
pub const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// One command of the public surface. Aliases are absent by design.
struct Cmd {
    noun: &'static str,
    verb: &'static str,
    description: &'static str,
    parameters: fn() -> Value,
    output: fn() -> Value,
    examples: [(&'static str, &'static str); 2],
    dry_run: bool,
    idempotent: bool,
    destructive: bool,
}

const COMMANDS: &[Cmd] = &[
    Cmd {
        noun: "daemon",
        verb: "run",
        description: "Grab the matched keyboards exclusively and apply the dual-role and remap rules.",
        parameters: run_params,
        output: plan_output,
        examples: [
            (
                "escapepod daemon run --map 'Caps_Lock=Escape' --yes",
                "Run with the xcape mapping string",
            ),
            (
                "escapepod daemon run --dry-run --json",
                "Report the devices and mappings without grabbing",
            ),
        ],
        dry_run: true,
        idempotent: false,
        destructive: true,
    },
    Cmd {
        noun: "daemon",
        verb: "status",
        description: "Report the running daemon's devices, mappings and held keys from its status socket.",
        parameters: status_params,
        output: status_output,
        examples: [
            ("escapepod daemon status", "Show the daemon's state"),
            ("escapepod daemon status --json", "The same as JSON"),
        ],
        dry_run: false,
        idempotent: true,
        destructive: false,
    },
    Cmd {
        noun: "event",
        verb: "monitor",
        description: "Print each key event of a managed key and Escape Pod's decision; other keys show as other.",
        parameters: monitor_params,
        output: monitor_output,
        examples: [
            ("escapepod event monitor", "Watch decisions live"),
            (
                "escapepod event monitor --format jsonl",
                "One JSON object per event",
            ),
        ],
        dry_run: false,
        idempotent: true,
        destructive: false,
    },
    Cmd {
        noun: "device",
        verb: "list",
        description: "List input devices from sysfs with their match status, without opening any device.",
        parameters: no_params,
        output: device_output,
        examples: [
            ("escapepod device list", "Show every input device"),
            (
                "escapepod device list --json --fields name,path",
                "Names and paths as JSON",
            ),
        ],
        dry_run: false,
        idempotent: true,
        destructive: false,
    },
    Cmd {
        noun: "config",
        verb: "check",
        description: "Validate the configuration without opening any input device or creating any virtual device.",
        parameters: mapping_params,
        output: check_output,
        examples: [
            (
                "escapepod config check --config ~/.config/escapepod/config.toml",
                "Validate a file",
            ),
            (
                "escapepod config check --map 'Caps_Lock=Escape' --json",
                "Validate a mapping string as JSON",
            ),
        ],
        dry_run: false,
        idempotent: true,
        destructive: false,
    },
    Cmd {
        noun: "schema",
        verb: "",
        description: "Print the JSON Schema Draft 2020-12 document of every command, one noun, or one command.",
        parameters: schema_params,
        output: object_output,
        examples: [
            ("escapepod schema", "Every command"),
            ("escapepod schema daemon run --json", "One command"),
        ],
        dry_run: false,
        idempotent: true,
        destructive: false,
    },
    Cmd {
        noun: "describe",
        verb: "",
        description: "Print the capability manifest: commands, flags and the detected output mode.",
        parameters: no_params,
        output: object_output,
        examples: [
            ("escapepod describe", "The manifest"),
            ("escapepod describe --json", "The manifest as JSON"),
        ],
        dry_run: false,
        idempotent: true,
        destructive: false,
    },
];

fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn mapping_props() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("config".into(), string("Configuration file path"));
    m.insert(
        "map".into(),
        json!({ "type": "string", "pattern": "^[^=;]+=[^=;|]+(;[^=;]+=[^=;|]+)*;?$", "description": "xcape mapping KEY=TAP[;KEY=TAP]..." }),
    );
    m.insert(
        "tap_timeout".into(),
        json!({ "type": "integer", "minimum": 50, "maximum": 2000, "description": "Tap timeout in milliseconds" }),
    );
    m
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "callers build the map only to hand it over"
)]
fn object(props: Map<String, Value>) -> Value {
    json!({ "type": "object", "properties": props, "additionalProperties": false })
}

fn no_params() -> Value {
    object(Map::new())
}

fn mapping_params() -> Value {
    object(mapping_props())
}

fn run_params() -> Value {
    let mut p = mapping_props();
    p.insert("backend".into(), json!({ "type": "string", "enum": ["auto", "evdev", "x11"], "default": "auto", "description": "Backend" }));
    p.insert(
        "yes".into(),
        json!({ "type": "boolean", "description": "Required off a terminal or under an agent" }),
    );
    p.insert(
        "dry_run".into(),
        json!({ "type": "boolean", "description": "Report the plan; grab nothing" }),
    );
    object(p)
}

fn status_params() -> Value {
    let mut p = Map::new();
    p.insert("socket".into(), string("Status socket path"));
    object(p)
}

fn monitor_params() -> Value {
    let mut p = mapping_props();
    p.insert("all_keys".into(), json!({ "type": "boolean", "description": "Show unmanaged key codes; refused off a terminal or under an agent" }));
    object(p)
}

fn schema_params() -> Value {
    let mut p = Map::new();
    p.insert("noun".into(), json!({ "type": "string", "enum": ["daemon", "event", "device", "config", "schema", "describe"], "description": "Limit to one noun" }));
    p.insert("verb".into(), string("Limit to one verb of the noun"));
    object(p)
}

fn key_list() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

fn mappings() -> Value {
    json!({
        "type": "object",
        "properties": {
            "tap_timeout_ms": { "type": "integer" },
            "dual_role": { "type": "array", "items": { "type": "object", "properties": {
                "key": { "type": "string" }, "hold": { "type": "string" }, "tap": { "type": "string" } } } },
            "remap": { "type": "array", "items": { "type": "object", "properties": {
                "from": { "type": "string" }, "to": { "type": "string" } } } },
            "protected": key_list(),
        }
    })
}

fn device_record() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "vendor": { "type": "string", "pattern": "^[0-9a-f]{4}$" },
            "product": { "type": "string", "pattern": "^[0-9a-f]{4}$" },
            "path": { "type": "string" },
            "kind": { "type": "string", "enum": ["keyboard", "pointer", "other"] },
            "status": { "type": "string", "enum": ["grab", "read", "ignored", "own", "none"] },
        }
    })
}

fn device_output() -> Value {
    json!({ "type": "array", "items": device_record() })
}

fn plan_output() -> Value {
    json!({ "type": "object", "properties": {
        "backend": { "type": "string" },
        "devices": { "type": "array", "items": device_record() },
        "mappings": mappings(),
        "emergency_chord": key_list(),
    } })
}

fn check_output() -> Value {
    json!({ "type": "object", "properties": {
        "path": { "type": ["string", "null"] },
        "mappings": mappings(),
        "devices": { "type": "array" },
        "emergency_chord": key_list(),
        "shadowed": key_list(),
    } })
}

fn status_output() -> Value {
    json!({ "type": "object", "properties": {
        "backend": { "type": "string" },
        "devices": { "type": "array" },
        "held": { "type": "integer" },
    } })
}

fn monitor_output() -> Value {
    json!({ "type": "object", "properties": {
        "timestamp": { "type": "string" },
        "device": { "type": "string" },
        "key": { "type": "string", "description": "A managed key's name, or other" },
        "value": { "type": "string", "enum": ["press", "release", "repeat"] },
        "decision": { "type": "string" },
    } })
}

fn object_output() -> Value {
    json!({ "type": "object" })
}

fn name(c: &Cmd) -> String {
    if c.verb.is_empty() {
        format!("escapepod {}", c.noun)
    } else {
        format!("escapepod {} {}", c.noun, c.verb)
    }
}

fn exit_codes() -> Value {
    Value::Object(
        crate::error::Exit::table()
            .iter()
            .map(|(c, d)| (c.to_string(), Value::from(*d)))
            .collect(),
    )
}

fn command_doc(c: &Cmd) -> Value {
    json!({
        "$schema": DIALECT,
        "command": name(c),
        "description": c.description,
        "parameters": (c.parameters)(),
        "output_schema": { "type": "object", "properties": {
            "metadata": { "type": "object" }, "data": (c.output)() } },
        "exit_codes": exit_codes(),
        "examples": c.examples.iter().map(|(cmd, d)| json!({ "command": cmd, "description": d })).collect::<Vec<_>>(),
        "supports_json": true,
        "supports_dry_run": c.dry_run,
        "idempotent": c.idempotent,
        "destructive": c.destructive,
    })
}

/// The schema document for every command, one noun, or one command.
/// Implements: ESC-SRS-041
///
/// # Errors
///
/// `NOT_FOUND` (exit 3) for an unknown noun or verb.
pub fn schema(noun: Option<&str>, verb: Option<&str>) -> Result<Value, AppError> {
    let picked: Vec<&Cmd> = COMMANDS
        .iter()
        .filter(|c| noun.is_none_or(|n| c.noun == n) && verb.is_none_or(|v| c.verb == v))
        .collect();
    match picked.as_slice() {
        [] => Err(AppError::new(
            "NOT_FOUND",
            Exit::NotFound,
            format!(
                "no command `{}`",
                [noun, verb]
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            "escapepod describe --json",
        )),
        [one] if verb.is_some() => Ok(command_doc(one)),
        many => Ok(json!({
            "$schema": DIALECT,
            "$id": "https://EscapePod.SpacecraftSoftware.org/schema/cli/1",
            "title": "escapepod",
            "description": "Escape Pod command surface (interface version 1)",
            "version": env!("CARGO_PKG_VERSION"),
            "commands": many.iter().map(|c| command_doc(c)).collect::<Vec<_>>(),
            "exit_codes": exit_codes(),
            "error_codes": ["INVALID_ARGUMENT", "MISSING_ARGUMENT", "NOT_FOUND", "PERMISSION_DENIED", "CONFLICT", "BACKEND_UNAVAILABLE", "X11_REMAP_UNSUPPORTED", "DISPLAY_LOST", "INTERNAL_ERROR"],
            "diagnostic_codes": ["TUI_FALLBACK", "UNKNOWN_THEME", "SCREEN_READER_KEY", "NO_MAPPING", "DEVICE_FAULT", "RELOAD_FAILED", "RELOADED", "DEVICE_GRABBED", "DEVICE_RELEASED", "EMERGENCY_RELEASE"],
        })),
    }
}

/// The capability manifest, with the output mode this invocation detected.
pub fn describe(mode: &crate::output::Mode) -> Value {
    json!({
        "tool": "escapepod",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Tap a key for Escape, hold it for Control: a dual-role key daemon for Linux",
        "license": "GPL-3.0-or-later",
        "homepage": "https://EscapePod.SpacecraftSoftware.org/",
        "maintainer": "Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>",
        "commands": COMMANDS.iter().map(|c| json!({
            "name": name(c),
            "description": c.description,
            "supports_json": true,
            "supports_dry_run": c.dry_run,
            "idempotent": c.idempotent,
            "destructive": c.destructive,
        })).collect::<Vec<_>>(),
        "exit_codes": exit_codes(),
        "global_flags": ["--json", "--format", "--fields", "--dry-run", "--verbose", "--quiet", "--no-color",
                         "--color", "--version", "--absolute-time", "--print0", "--yes", "--force", "--accessible", "--config"],
        "mode": {
            "profile": if mode.agent { "agent" } else if mode.interactive { "human" } else { "pipe" },
            "format": mode.format.as_str(),
            "color": mode.color,
            "interactive": mode.interactive,
            "accessible": mode.accessible,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_two_examples_one_with_json() {
        for c in COMMANDS {
            assert!(
                c.examples
                    .iter()
                    .any(|(e, _)| e.contains("--json") || e.contains("--format")),
                "{}",
                name(c)
            );
            assert!(
                c.examples.iter().all(|(e, _)| e.starts_with(&name(c))),
                "{}",
                name(c)
            );
        }
    }

    #[test]
    fn lookups() {
        assert!(
            schema(Some("daemon"), Some("run"))
                .is_ok_and(|v| v["command"] == "escapepod daemon run")
        );
        assert!(
            schema(Some("daemon"), None)
                .is_ok_and(|v| v["commands"].as_array().map(Vec::len) == Some(2))
        );
        assert_eq!(
            schema(Some("nope"), None).err().map(|e| e.exit()),
            Some(Exit::NotFound)
        );
    }
}

// Rust guideline compliant 2026-05-18
