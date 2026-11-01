// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CLI compliance suite (CLI Standard testing-compliance): schema
//! validity, exit codes, the agent environment with the host's real
//! `AI_AGENT` value, stream separation, input validation and the context
//! files. Runs the built binary; touches no real device: sysfs is a fake
//! tree whose event nodes are unreadable.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly by design"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

/// The `AI_AGENT` value of the harness running the tests, or a descriptive
/// stand-in. Detection must work for descriptive strings, not only `1`.
fn agent_value() -> String {
    std::env::var("AI_AGENT")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "claude-code_2-1-218_agent".into())
}

/// A scratch directory unique to one test.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("escapepod-cli-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// A fake `/sys/class/input` with a keyboard, a pointer, a power button and
/// an Escape Pod virtual keyboard, and a `/dev/input` whose nodes are
/// unreadable files, so any attempt to open one fails.
fn fake_devices(root: &Path) -> (PathBuf, PathBuf) {
    let sys = root.join("sys");
    let dev = root.join("dev");
    fs::create_dir_all(&dev).unwrap();
    let devices = [
        (
            "event0",
            "AT Translated Set 2 keyboard",
            "0001",
            "0001",
            "402000000 3803078f800d001 feffffdfffefffff fffffffffffffffe",
        ),
        (
            "event1",
            "Elan Touchpad",
            "04f3",
            "002f",
            "6420 30000 0 0 0 0",
        ),
        ("event2", "Power Button", "0000", "0001", "10000000000000 0"),
        (
            "event3",
            "Escape Pod virtual keyboard",
            "5350",
            "4550",
            "402000000 3803078f800d001 feffffdfffefffff fffffffffffffffe",
        ),
    ];
    for (node, name, vendor, product, keys) in devices {
        let d = sys.join(node).join("device");
        fs::create_dir_all(d.join("id")).unwrap();
        fs::create_dir_all(d.join("capabilities")).unwrap();
        fs::write(d.join("name"), format!("{name}\n")).unwrap();
        fs::write(d.join("id/vendor"), format!("{vendor}\n")).unwrap();
        fs::write(d.join("id/product"), format!("{product}\n")).unwrap();
        fs::write(d.join("capabilities/key"), format!("{keys}\n")).unwrap();
        let n = dev.join(node);
        if n.exists() {
            continue;
        }
        fs::write(&n, b"").unwrap();
        let mut perm = fs::metadata(&n).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perm, 0o000);
        fs::set_permissions(&n, perm).unwrap();
    }
    (sys, dev)
}

/// The binary with a clean environment: no agent, no config, fake devices.
fn cmd(root: &Path) -> Command {
    let (sys, dev) = fake_devices(root);
    let mut c = Command::cargo_bin("escapepod").unwrap();
    c.env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("ESCAPEPOD_SYSFS", sys)
        .env("ESCAPEPOD_DEV_DIR", dev)
        // No test creates a real virtual keyboard: the uinput probe fails.
        .env("ESCAPEPOD_UINPUT", root.join("no-uinput"));
    c
}

fn json_lines(bytes: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

fn stdout_doc(o: &Output) -> Value {
    let docs = json_lines(&o.stdout);
    assert_eq!(
        docs.len(),
        1,
        "exactly one JSON document on stdout: {}",
        String::from_utf8_lossy(&o.stdout)
    );
    docs.into_iter().next().unwrap()
}

fn error_of(o: &Output) -> Value {
    let lines = json_lines(&o.stderr);
    lines
        .into_iter()
        .find(|l| l.get("error").is_some())
        .unwrap_or_else(|| panic!("no error object in {}", String::from_utf8_lossy(&o.stderr)))
}

const DATA_COMMANDS: [&[&str]; 6] = [
    &["describe"],
    &["schema"],
    &["schema", "daemon", "run"],
    &["device", "list"],
    &["config", "check", "--map", "Caps_Lock=Escape"],
    &["daemon", "run", "--map", "Caps_Lock=Escape", "--dry-run"],
];

// Verifies: ESC-SRS-022
#[test]
fn command_tree_is_complete() {
    let root = scratch("tree");
    let help = cmd(&root).arg("--help").output().unwrap();
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    for noun in [
        "daemon", "event", "device", "config", "schema", "describe", "run",
    ] {
        assert!(text.contains(noun), "{noun} missing from --help");
    }
    assert!(
        text.contains("https://EscapePod.SpacecraftSoftware.org/")
            && text.contains("Mohamed Hammad")
    );
    let d = stdout_doc(&cmd(&root).arg("describe").output().unwrap());
    let names: Vec<&str> = d["data"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();
    for want in [
        "daemon run",
        "daemon status",
        "event monitor",
        "device list",
        "config check",
        "schema",
        "describe",
    ] {
        assert!(
            names.contains(&format!("escapepod {want}").as_str()),
            "{want} not in describe"
        );
    }
    assert!(
        !names.contains(&"escapepod run"),
        "the human alias stays out of describe"
    );
    for sub in [
        &["daemon", "run"][..],
        &["daemon", "status"],
        &["event", "monitor"],
        &["device", "list"],
        &["config", "check"],
    ] {
        let o = cmd(&root).args(sub).arg("--help").output().unwrap();
        let t = String::from_utf8_lossy(&o.stdout);
        assert!(
            o.status.success() && t.matches("  escapepod ").count() >= 2,
            "{sub:?} needs two examples:\n{t}"
        );
    }
}

// Verifies: ESC-SRS-023
#[test]
fn every_global_flag_is_accepted() {
    let root = scratch("flags");
    let flags: [&[&str]; 16] = [
        &["--json"],
        &["--format", "json"],
        &["--format", "jsonl"],
        &["--format", "yaml"],
        &["--format", "csv"],
        &["--fields", "tool,version"],
        &["--dry-run"],
        &["--verbose"],
        &["-q"],
        &["--no-color"],
        &["--color", "never"],
        &["--absolute-time"],
        &["--print0"],
        &["--yes"],
        &["--force"],
        &["--accessible"],
    ];
    for f in flags {
        let o = cmd(&root).arg("describe").args(f).output().unwrap();
        assert!(
            o.status.success(),
            "describe {f:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    let fields = stdout_doc(
        &cmd(&root)
            .args(["describe", "--fields", "tool,version"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        fields["data"].as_object().map(serde_json::Map::len),
        Some(2)
    );
    let both = cmd(&root)
        .args(["describe", "--quiet", "--verbose"])
        .output()
        .unwrap();
    assert_eq!(
        both.status.code(),
        Some(2),
        "--quiet with --verbose is a usage error"
    );
    let v = cmd(&root).arg("--version").output().unwrap();
    assert!(v.status.success());
    let yaml = cmd(&root)
        .args(["describe", "--format", "yaml"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&yaml.stdout).starts_with("---\n"));
    let csv = cmd(&root)
        .args(["device", "list", "--format", "csv"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&csv.stdout)
            .starts_with("kind,name,path,product,status,vendor\r\n")
    );
    let explore = cmd(&root)
        .args(["describe", "--format", "explore"])
        .output()
        .unwrap();
    assert!(
        explore.status.success()
            && String::from_utf8_lossy(&explore.stderr).contains("TUI_FALLBACK")
    );
}

// Verifies: ESC-SRS-024
#[test]
fn data_commands_write_one_envelope() {
    let root = scratch("envelope");
    let schema: Value =
        serde_json::from_str(&fs::read_to_string("../../schemas/envelope.schema.json").unwrap())
            .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for c in DATA_COMMANDS {
        let o = cmd(&root).args(c).arg("--json").output().unwrap();
        assert!(
            o.status.success(),
            "{c:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let doc = stdout_doc(&o);
        assert!(
            validator.is_valid(&doc),
            "{c:?} envelope fails schemas/envelope.schema.json: {doc}"
        );
        let ts = doc["metadata"]["timestamp"].as_str().unwrap();
        assert!(ts.ends_with('Z') && ts.len() == 20, "UTC timestamp: {ts}");
    }
}

// Verifies: ESC-SRS-025
#[test]
fn errors_are_one_json_object_with_a_runnable_hint() {
    let root = scratch("errors");
    let cases: [&[&str]; 5] = [
        &["bogus"],
        &["config", "check", "--map", "Nope=Escape"],
        &["config", "check", "--config", "/nonexistent/escapepod.toml"],
        &["schema", "nope"],
        &["daemon", "run", "--map", "Caps_Lock=Escape"],
    ];
    for c in cases {
        let o = cmd(&root).args(c).output().unwrap();
        assert!(!o.status.success(), "{c:?}");
        assert!(o.stdout.is_empty(), "{c:?}: stdout stays empty on error");
        let e = &error_of(&o)["error"];
        assert_eq!(
            e["exit_code"].as_i64(),
            o.status.code().map(i64::from),
            "{c:?}"
        );
        for k in ["code", "message", "hint", "timestamp", "command"] {
            assert!(e[k].as_str().is_some_and(|s| !s.is_empty()), "{c:?}: {k}");
        }
        let hint = e["hint"].as_str().unwrap();
        assert!(
            hint.starts_with("escapepod ") || hint.starts_with("human:"),
            "{c:?}: hint `{hint}`"
        );
        assert!(
            !hint.contains("sudo"),
            "{c:?}: no elevated privileges in hints"
        );
        if let Some(rest) = hint.strip_prefix("escapepod ") {
            let words: Vec<&str> = rest
                .split_whitespace()
                .take_while(|w| !w.starts_with('-'))
                .collect();
            if !words.is_empty() && words != ["describe"] {
                let h = cmd(&root).args(&words).arg("--help").output().unwrap();
                assert!(
                    h.status.success(),
                    "{c:?}: hint names a real command: {hint}"
                );
            }
        }
    }
}

// Verifies: ESC-SRS-026, ESC-SRS-090
#[test]
fn agent_environment_selects_json_without_colour() {
    let root = scratch("agent");
    for (k, v) in [
        ("AI_AGENT", agent_value()),
        ("AGENT", "goose".into()),
        ("CI", "true".into()),
    ] {
        let o = cmd(&root)
            .env(k, &v)
            .args(["describe", "--color", "always"])
            .output()
            .unwrap();
        let doc = stdout_doc(&o);
        assert_eq!(doc["data"]["mode"]["format"], "json", "{k}={v}");
        assert_eq!(doc["data"]["mode"]["color"], false, "{k}={v}");
        assert_eq!(doc["data"]["mode"]["interactive"], false, "{k}={v}");
        assert!(
            !o.stdout.contains(&0x1b) && !o.stderr.contains(&0x1b),
            "{k}={v}: no ANSI escape"
        );
        let e = cmd(&root)
            .env(k, &v)
            .args(["config", "check", "--map", "Bad=Escape"])
            .output()
            .unwrap();
        assert!(!e.stderr.contains(&0x1b) && error_of(&e)["error"]["code"] == "INVALID_ARGUMENT");
    }
    let o = cmd(&root)
        .env("AI_AGENT", agent_value())
        .arg("describe")
        .output()
        .unwrap();
    assert_eq!(
        stdout_doc(&o)["data"]["mode"]["profile"],
        "agent",
        "AI_AGENT={}",
        agent_value()
    );
}

// Verifies: ESC-SRS-091
#[test]
fn agents_are_never_prompted() {
    let root = scratch("prompt");
    // Standard input is closed: a prompt would read EOF or hang. Every
    // command must finish on its own.
    for c in DATA_COMMANDS {
        let o = cmd(&root)
            .env("AI_AGENT", agent_value())
            .args(c)
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        assert!(o.status.code().is_some(), "{c:?} finished");
    }
    let o = cmd(&root)
        .env("AI_AGENT", agent_value())
        .args(["daemon", "run", "--map", "Caps_Lock=Escape"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(2),
        "an agent gets an error, not a prompt"
    );
}

// Verifies: ESC-SRS-027, ESC-SRS-095
#[test]
fn exit_codes_are_canonical_and_6_is_never_used_here() {
    let root = scratch("exits");
    let cases: [(&[&str], i32); 6] = [
        (&["describe"], 0),
        (
            &["config", "check", "--config", "/nonexistent/escapepod.toml"],
            3,
        ),
        (&["schema", "nope"], 3),
        (&["config", "check", "--map", "x"], 2),
        (&["bogus"], 2),
        (&["daemon", "run", "--map", "Caps_Lock=Escape", "--yes"], 4),
    ];
    for (c, want) in cases {
        assert_eq!(
            cmd(&root).args(c).output().unwrap().status.code(),
            Some(want),
            "{c:?}"
        );
    }
    let doc = stdout_doc(&cmd(&root).arg("schema").output().unwrap());
    let codes = doc["data"]["exit_codes"].as_object().unwrap();
    assert_eq!(
        codes.keys().map(String::as_str).collect::<Vec<_>>(),
        ["0", "1", "2", "3", "4", "5", "6"]
    );
    assert!(codes["6"].as_str().unwrap().contains("emergency"));
}

// Verifies: ESC-SRS-028
#[test]
fn version_carries_the_attribution() {
    let root = scratch("version");
    let o = cmd(&root)
        .args(["--version", "--format", "yaml"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.contains("Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"));
    let doc = stdout_doc(&cmd(&root).args(["--version", "--json"]).output().unwrap());
    assert_eq!(
        doc["metadata"]["maintainer"],
        "Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    );
    assert_eq!(
        doc["metadata"]["website"],
        "https://EscapePod.SpacecraftSoftware.org/"
    );
}

// Verifies: ESC-SRS-035
#[test]
fn config_check_opens_no_device() {
    let root = scratch("check");
    // The device nodes are unreadable and no uinput path exists; sysfs is
    // removed too, so even discovery would fail. The check must still pass.
    let config = root.join("c.toml");
    fs::write(
        &config,
        "[[dual_role]]\nkey = \"Caps_Lock\"\ntap = \"Escape\"\n",
    )
    .unwrap();
    let mut c = cmd(&root);
    fs::remove_dir_all(root.join("sys")).unwrap();
    let o = c
        .args(["config", "check", "--json", "--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        stdout_doc(&o)["data"]["mappings"]["dual_role"][0]["key"],
        "KEY_CAPSLOCK"
    );
}

// Verifies: ESC-SRS-036
#[test]
fn device_list_reads_sysfs_only() {
    let root = scratch("devices");
    let o = cmd(&root)
        .args(["device", "list", "--json"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let data = stdout_doc(&o)["data"].clone();
    let rows = data.as_array().unwrap();
    assert_eq!(rows.len(), 4);
    for k in ["name", "vendor", "product", "path", "kind", "status"] {
        assert!(rows.iter().all(|r| r.get(k).is_some()), "{k}");
    }
    let status: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r["kind"].as_str().unwrap(), r["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        status,
        [
            ("keyboard", "grab"),
            ("pointer", "read"),
            ("other", "none"),
            ("keyboard", "own")
        ]
    );
    assert_eq!(rows[0]["vendor"], "0001");
}

// Verifies: ESC-SRS-037
#[test]
fn run_needs_yes_off_a_terminal_or_under_an_agent() {
    let root = scratch("yes");
    let piped = cmd(&root)
        .args(["daemon", "run", "--map", "Caps_Lock=Escape"])
        .output()
        .unwrap();
    assert_eq!(piped.status.code(), Some(2));
    assert!(
        error_of(&piped)["error"]["hint"]
            .as_str()
            .unwrap()
            .ends_with("--yes")
    );
    let agent = cmd(&root)
        .env("AI_AGENT", agent_value())
        .args(["run", "--map", "Caps_Lock=Escape"])
        .output()
        .unwrap();
    assert_eq!(
        agent.status.code(),
        Some(2),
        "the human alias is guarded too"
    );
    let yes = cmd(&root)
        .args(["daemon", "run", "--map", "Caps_Lock=Escape", "--yes"])
        .output()
        .unwrap();
    assert_ne!(yes.status.code(), Some(2), "--yes passes the guard");
}

// Verifies: ESC-SRS-038, ESC-SRS-093
#[test]
fn orca_modifier_is_protected_with_a_warning() {
    let root = scratch("orca");
    fs::create_dir_all(root.join("data/orca")).unwrap();
    fs::write(
        root.join("data/orca/user-settings.conf"),
        r#"{"general": {"orcaModifierKeys": ["Caps_Lock"]}}"#,
    )
    .unwrap();
    let o = cmd(&root)
        .args([
            "daemon",
            "run",
            "--map",
            "Caps_Lock=Escape;Shift_L=Tab",
            "--dry-run",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let plan = stdout_doc(&o);
    assert_eq!(
        plan["data"]["mappings"]["protected"],
        serde_json::json!(["KEY_CAPSLOCK"])
    );
    let warn = json_lines(&o.stderr)
        .into_iter()
        .find(|l| l["diagnostic"]["code"] == "SCREEN_READER_KEY");
    let warn =
        warn.unwrap_or_else(|| panic!("no warning in {}", String::from_utf8_lossy(&o.stderr)));
    assert_eq!(warn["diagnostic"]["severity"], "warn");
    assert!(
        warn["diagnostic"]["message"]
            .as_str()
            .unwrap()
            .contains("KEY_CAPSLOCK")
    );
}

// Verifies: ESC-SRS-039, ESC-SRS-094
#[test]
fn dry_run_reports_the_plan_and_grabs_nothing() {
    let root = scratch("dry");
    for agent in [false, true] {
        let mut c = cmd(&root);
        if agent {
            c.env("AI_AGENT", agent_value());
        }
        let o = c
            .args(["daemon", "run", "--map", "Caps_Lock=Escape", "--dry-run"])
            .output()
            .unwrap();
        assert_eq!(
            o.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let plan = &stdout_doc(&o)["data"];
        let grabs: Vec<&str> = plan["devices"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["status"] == "grab")
            .filter_map(|d| d["path"].as_str())
            .collect();
        assert_eq!(grabs.len(), 1, "{plan}");
        assert_eq!(plan["mappings"]["dual_role"][0]["tap"], "KEY_ESC");
    }
    // The device nodes are unreadable, so any attempt to open one would
    // have failed the command: the plan came from sysfs alone.
}

// Verifies: ESC-SRS-041
#[test]
fn schema_is_valid_json_schema_2020_12() {
    let root = scratch("schema");
    for args in [
        &["schema"][..],
        &["schema", "daemon"],
        &["schema", "config", "check"],
    ] {
        let doc = stdout_doc(&cmd(&root).args(args).output().unwrap());
        let s = &doc["data"];
        assert_eq!(s["$schema"], "https://json-schema.org/draft/2020-12/schema");
        jsonschema::meta::validate(s).unwrap_or_else(|e| panic!("{args:?}: {e}"));
        let commands: Vec<&Value> = s["commands"]
            .as_array()
            .map_or_else(|| vec![s], |a| a.iter().collect());
        for c in commands {
            for part in ["parameters", "output_schema"] {
                let mut sub = c[part].clone();
                sub["$schema"] = Value::from("https://json-schema.org/draft/2020-12/schema");
                jsonschema::meta::validate(&sub)
                    .unwrap_or_else(|e| panic!("{} {part}: {e}", c["command"]));
            }
            assert!(c["examples"].as_array().unwrap().len() >= 2);
        }
    }
}

/// The repository's context files are present (agentic CLI audit 1-3).
#[test]
fn context_files_are_present() {
    for f in ["AGENTS.md", "CLAUDE.md", "SKILL.md", "CONTRIBUTING.md"] {
        assert!(Path::new("../..").join(f).is_file(), "{f}");
    }
}

/// Control characters in string arguments are rejected (agentic threat
/// model), and non-UTF-8 output never appears.
#[test]
fn control_characters_are_rejected() {
    let root = scratch("ctrl");
    let o = cmd(&root)
        .args(["config", "check", "--map", "Caps_Lock=Esc\u{7}ape"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    std::str::from_utf8(&o.stderr).unwrap();
}

// Verifies: ESC-SRS-095
/// Exit code 6 is produced only by the emergency path: outside the exit
/// table, `Exit::Emergency` appears only where the loop handles the
/// daemon's `Flow::Emergency`.
#[test]
fn exit_6_has_one_producer() {
    let mut users = Vec::new();
    for entry in fs::read_dir("src").unwrap() {
        let p = entry.unwrap().path();
        let text = fs::read_to_string(&p).unwrap();
        if text.contains("Exit::Emergency") && !p.ends_with("error.rs") {
            users.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(
        users.iter().all(|f| f == "service.rs"),
        "Exit::Emergency used in {users:?}"
    );
}

/// The same invocation twice gives the same data (CLI Standard rule 3).
#[test]
fn read_commands_are_idempotent() {
    let root = scratch("idem");
    for c in DATA_COMMANDS {
        let a = stdout_doc(&cmd(&root).args(c).output().unwrap());
        let b = stdout_doc(&cmd(&root).args(c).output().unwrap());
        assert_eq!(a["data"], b["data"], "{c:?}");
    }
}

/// JSON output round-trips through a structured shell: Nushell parses
/// the envelope and reads a field. Skipped where `nu` is not installed.
#[test]
fn json_round_trips_through_nushell() {
    let Ok(nu) = std::process::Command::new("nu").arg("--version").output() else {
        return;
    };
    if !nu.status.success() {
        return;
    }
    let root = scratch("nu");
    let o = cmd(&root)
        .args(["device", "list", "--json"])
        .output()
        .unwrap();
    let mut child = std::process::Command::new("nu")
        .args([
            "--stdin",
            "-c",
            "$in | from json | get data | where kind == keyboard | length",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), &o.stdout).unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "2");
}

// Verifies: ESC-SRS-043
/// Without a virtual keyboard the daemon exits 4 and has grabbed nothing:
/// the uinput probe fails before any device is opened (the fake device
/// nodes are unreadable, so an open would also have failed loudly).
#[test]
fn no_virtual_keyboard_means_exit_4_before_any_grab() {
    let root = scratch("uinput");
    let o = cmd(&root)
        .args(["daemon", "run", "--map", "Caps_Lock=Escape", "--yes"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(4));
    let e = &error_of(&o)["error"];
    assert_eq!(e["code"], "BACKEND_UNAVAILABLE");
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("no device was grabbed")
    );
    let lines = String::from_utf8_lossy(&o.stderr);
    assert!(
        !lines.contains("DEVICE_GRABBED") && !lines.contains("DEVICE_OPEN_FAILED"),
        "{lines}"
    );
}

// Verifies: ESC-SRS-062
#[test]
fn all_keys_is_refused_off_a_terminal_and_under_agents() {
    let root = scratch("allkeys");
    for agent in [None, Some(agent_value())] {
        let mut c = cmd(&root);
        if let Some(v) = &agent {
            c.env("AI_AGENT", v);
        }
        let o = c
            .args([
                "event",
                "monitor",
                "--all-keys",
                "--map",
                "Caps_Lock=Escape",
            ])
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(4), "{agent:?}");
        assert_eq!(error_of(&o)["error"]["code"], "PERMISSION_DENIED");
        assert!(o.stdout.is_empty());
    }
}

// Verifies: ESC-SRS-066, ESC-SRS-101
#[test]
fn x11_refuses_remaps_with_a_setxkbmap_hint() {
    let root = scratch("x11remap");
    let config = root.join("c.toml");
    fs::write(
        &config,
        "[[remap]]\nfrom = \"Caps_Lock\"\nto = \"Control_L\"\n",
    )
    .unwrap();
    let o = cmd(&root)
        .env("DISPLAY", ":999")
        .args(["daemon", "run", "--yes", "--backend", "x11", "--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    let e = &error_of(&o)["error"];
    assert_eq!(e["code"], "X11_REMAP_UNSUPPORTED");
    assert_eq!(e["hint"], "setxkbmap -option ctrl:nocaps");
}

// Verifies: ESC-SRS-064, ESC-SRS-100
#[test]
fn a_forced_backend_never_falls_back() {
    let root = scratch("forced");
    let evdev = cmd(&root)
        .env("DISPLAY", ":999")
        .args([
            "daemon",
            "run",
            "--yes",
            "--map",
            "Caps_Lock=Escape",
            "--backend",
            "evdev",
        ])
        .output()
        .unwrap();
    assert_eq!(
        evdev.status.code(),
        Some(4),
        "no uinput: no fallback to X11 though DISPLAY is set"
    );
    assert!(
        error_of(&evdev)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("uinput")
    );
    let x11 = cmd(&root)
        .args([
            "daemon",
            "run",
            "--yes",
            "--map",
            "Control_L=Escape",
            "--backend",
            "x11",
        ])
        .output()
        .unwrap();
    assert_eq!(x11.status.code(), Some(4), "no DISPLAY");
    assert!(
        error_of(&x11)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("X11 backend is unavailable")
    );
}

// Verifies: ESC-SRS-063
#[test]
fn auto_with_no_backend_exits_4() {
    let root = scratch("nobackend");
    let o = cmd(&root)
        .args(["daemon", "run", "--yes", "--map", "Caps_Lock=Escape"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(4));
    assert_eq!(error_of(&o)["error"]["code"], "BACKEND_UNAVAILABLE");
}
