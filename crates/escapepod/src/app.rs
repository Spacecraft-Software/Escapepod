// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parsing, dispatch and the commands that need no device access.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;
use escapepod_core::{KeyCode, Rules};
use escapepod_evdev::{DeviceInfo, DeviceKind};
use serde_json::{Value, json};

use crate::cli::{
    BackendArg, Cli, ColorArg, Command, ConfigCmd, DaemonCmd, DeviceCmd, EventCmd, MappingArgs,
    RunArgs,
};
use crate::config::{self, Config, Overrides};
use crate::error::{AppError, Exit};
use crate::output::{ColorChoice, Env, Flags, Format, Mode, Severity};
use crate::theme::Token;
use crate::{keymap, orca, schema};

/// Command words a usage hint may name: never the caller's typo.
const KNOWN: [&str; 11] = [
    "daemon", "run", "status", "event", "monitor", "device", "list", "config", "check", "schema",
    "describe",
];

/// The standard streams and environment one invocation uses.
pub struct Io<'a> {
    /// Standard output.
    pub out: &'a mut dyn Write,
    /// Standard error.
    pub err: &'a mut dyn Write,
    /// The environment.
    pub env: &'a Env,
}

/// Runs one invocation and returns its exit code.
pub fn run(args: &[OsString], io: &mut Io<'_>) -> Exit {
    let command = render_command(args);
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => return parse_failure(&e, args, command, io),
    };
    let flags = flags_of(&cli);
    let (mode, warnings) = match Mode::resolve(&flags, io.env, command) {
        Ok(m) => m,
        Err(e) => {
            let fallback =
                Mode::resolve(&Flags::default(), io.env, render_command(args)).map(|(m, _)| m);
            if let Ok(m) = fallback {
                m.error(io.err, &e);
            }
            return e.exit();
        }
    };
    for (code, message) in &warnings {
        mode.diag(
            io.err,
            Severity::Warn,
            code,
            message,
            Some("escapepod describe --json"),
        );
    }
    match dispatch(&cli, &mode, io) {
        Ok(exit) => exit,
        Err(e) => {
            mode.error(io.err, &e);
            e.exit()
        }
    }
}

/// The invocation as one string, program name reduced to `escapepod`.
fn render_command(args: &[OsString]) -> String {
    std::iter::once("escapepod".to_owned())
        .chain(args.iter().skip(1).map(|a| {
            let s = a.to_string_lossy();
            if s.contains(char::is_whitespace) {
                format!("'{s}'")
            } else {
                s.into_owned()
            }
        }))
        .collect::<Vec<_>>()
        .join(" ")
}

fn flags_of(cli: &Cli) -> Flags {
    let g = &cli.global;
    Flags {
        format: if g.json {
            Some("json".into())
        } else {
            g.format.clone()
        },
        fields: g.fields.clone(),
        verbose: g.verbose,
        quiet: g.quiet,
        color: if g.no_color {
            Some(ColorChoice::Never)
        } else {
            g.color.map(|c| match c {
                ColorArg::Never => ColorChoice::Never,
                ColorArg::Always => ColorChoice::Always,
                ColorArg::Auto => ColorChoice::Auto,
            })
        },
        print0: g.print0,
        accessible: g.accessible,
    }
}

/// A clap failure: help goes to standard output with exit 0; anything else
/// is a usage error (exit 2) in the detected mode.
fn parse_failure(e: &clap::Error, args: &[OsString], command: String, io: &mut Io<'_>) -> Exit {
    use clap::error::ErrorKind;
    if e.kind() == ErrorKind::DisplayHelp {
        let _ = write!(io.out, "{e}");
        return Exit::Success;
    }
    let raw: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let flags = Flags {
        format: if raw.iter().any(|a| a == "--json") {
            Some("json".into())
        } else {
            raw.iter()
                .position(|a| a == "--format")
                .and_then(|i| raw.get(i + 1).cloned())
                .filter(|f| ["json", "jsonl", "yaml", "csv"].contains(&f.as_str()))
        },
        quiet: raw.iter().any(|a| a == "--quiet" || a == "-q"),
        color: raw
            .iter()
            .any(|a| a == "--no-color")
            .then_some(ColorChoice::Never),
        ..Flags::default()
    };
    let Ok((mode, _)) = Mode::resolve(&flags, io.env, command) else {
        return Exit::Usage;
    };
    let message = e.to_string();
    let first = message
        .lines()
        .next()
        .unwrap_or("invalid arguments")
        .trim_start_matches("error: ")
        .to_owned();
    let words: Vec<&str> = raw
        .iter()
        .skip(1)
        .map(String::as_str)
        .take_while(|a| !a.starts_with('-') && KNOWN.contains(a))
        .take(2)
        .collect();
    let hint = if words.is_empty() {
        "escapepod --help".to_owned()
    } else {
        format!("escapepod {} --help", words.join(" "))
    };
    mode.error(
        io.err,
        &AppError::new("USAGE_ERROR", Exit::Usage, first, hint),
    );
    Exit::Usage
}

fn dispatch(cli: &Cli, mode: &Mode, io: &mut Io<'_>) -> Result<Exit, AppError> {
    if cli.global.version {
        return version(mode, io);
    }
    let Some(cmd) = &cli.command else {
        if mode.format == Format::Human {
            let _ = writeln!(
                io.out,
                "escapepod: tap a key for Escape, hold it for Control\n\nRun `escapepod --help` for the commands.\n\n{}",
                crate::cli::footer()
            );
            return Ok(Exit::Success);
        }
        return Err(AppError::new(
            "MISSING_ARGUMENT",
            Exit::Usage,
            "no command given",
            "escapepod describe --json",
        ));
    };
    match cmd {
        Command::Daemon(DaemonCmd::Run(args)) | Command::Run(args) => {
            daemon_run(cli, args, mode, io)
        }
        Command::Daemon(DaemonCmd::Status { socket }) => {
            crate::service::status(socket.as_deref(), mode, io)
        }
        Command::Event(EventCmd::Monitor { all_keys, mapping }) => {
            let cfg = load_config(cli, mapping, io.env)?;
            crate::service::monitor(&cfg, *all_keys, mode, io)
        }
        Command::Device(DeviceCmd::List) => device_list(cli, mode, io),
        Command::Config(ConfigCmd::Check { mapping }) => config_check(cli, mapping, mode, io),
        Command::Schema { noun, verb } => {
            let doc = schema::schema(noun.as_deref(), verb.as_deref())?;
            mode.emit(io.out, doc, |v, _| {
                format!("{}\n", serde_json::to_string_pretty(v).unwrap_or_default())
            })?;
            Ok(Exit::Success)
        }
        Command::Describe => {
            mode.emit(io.out, schema::describe(mode), describe_text)?;
            Ok(Exit::Success)
        }
    }
}

/// `--version`: the version, the maintainer line and the project URL.
/// Implements: ESC-SRS-028
fn version(mode: &Mode, io: &mut Io<'_>) -> Result<Exit, AppError> {
    let maintainer = "Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>";
    let website = "https://EscapePod.SpacecraftSoftware.org/";
    let data = json!({ "version": env!("CARGO_PKG_VERSION"), "maintainer": maintainer,
                       "website": website, "license": "GPL-3.0-or-later" });
    mode.emit_with(
        io.out,
        data,
        &[("maintainer", maintainer), ("website", website)],
        |_, _| {
            format!(
                "escapepod {}\nMaintained by {maintainer}\n{website}\n",
                env!("CARGO_PKG_VERSION")
            )
        },
    )?;
    Ok(Exit::Success)
}

/// The configuration for a command, with `--map`, `--tap-timeout` and the
/// Orca modifier keys applied.
pub fn load_config(cli: &Cli, mapping: &MappingArgs, env: &Env) -> Result<Config, AppError> {
    let path = config::locate(
        cli.global.config.as_deref(),
        env,
        Path::new(config::SYSTEM_PATH),
    );
    let map = mapping
        .map
        .as_deref()
        .map(|m| {
            keymap::parse(m).map_err(|e| {
                AppError::invalid(
                    format!("--map: {e}"),
                    "escapepod daemon run --map 'Caps_Lock=Escape' --dry-run",
                )
                .with("position", e.position)
            })
        })
        .transpose()?;
    let overrides = Overrides {
        map,
        tap_timeout_ms: mapping.tap_timeout,
    };
    let orca = orca_keys(env);
    config::load(path.as_deref(), &overrides, &orca)
}

/// The Orca modifier keys of this user's session (ESC-SRS-038).
fn orca_keys(env: &Env) -> Vec<KeyCode> {
    orca::settings_path(env)
        .map(|p| orca::modifier_keys(&p))
        .unwrap_or_default()
}

/// Warns about each Orca modifier key the configuration maps
/// (ESC-SRS-093).
fn warn_screen_reader(cfg: &Config, env: &Env, mode: &Mode, io: &mut Io<'_>) {
    let orca = orca_keys(env);
    for key in cfg.shadowed.iter().filter(|k| orca.contains(k)) {
        mode.diag(
            io.err,
            Severity::Warn,
            "SCREEN_READER_KEY",
            &format!("{key} is the Orca modifier key, so its mapping is left inactive"),
            Some("human: map another key, or change Orca's modifier key in Orca's preferences"),
        );
    }
}

/// The sysfs class directory and device directory, overridable for tests
/// and containers through `ESCAPEPOD_SYSFS` (manual, `Interfaces`).
pub fn device_roots(env: &Env) -> (PathBuf, PathBuf) {
    let sysfs = env
        .var("ESCAPEPOD_SYSFS")
        .map_or_else(|| PathBuf::from("/sys/class/input"), PathBuf::from);
    let dev = env
        .var("ESCAPEPOD_DEV_DIR")
        .map_or_else(|| PathBuf::from("/dev/input"), PathBuf::from);
    (sysfs, dev)
}

/// The devices sysfs reports, without opening any of them.
pub fn scan_devices(env: &Env) -> Result<Vec<DeviceInfo>, AppError> {
    let (sysfs, dev) = device_roots(env);
    escapepod_evdev::scan(&sysfs, &dev).map_err(|e| {
        AppError::new(
            "NOT_FOUND",
            Exit::NotFound,
            format!("cannot read {}: {e}", sysfs.display()),
            "ls /sys/class/input",
        )
    })
}

/// What the daemon would do with a device under `cfg`.
pub fn status_of(d: &DeviceInfo, cfg: Option<&Config>) -> &'static str {
    if d.is_own() {
        return "own";
    }
    match d.kind {
        DeviceKind::Keyboard => match cfg.map(|c| c.rules_for(d)) {
            Some(None) => "ignored",
            _ => "grab",
        },
        DeviceKind::Pointer => "read",
        DeviceKind::Other => "none",
    }
}

fn device_json(d: &DeviceInfo, cfg: Option<&Config>) -> Value {
    json!({
        "name": d.name,
        "vendor": format!("{:04x}", d.vendor),
        "product": format!("{:04x}", d.product),
        "path": d.path.display().to_string(),
        "kind": d.kind.as_str(),
        "status": status_of(d, cfg),
    })
}

/// `device list`: sysfs only, so it needs no read access to any event
/// device. Implements: ESC-SRS-036
fn device_list(cli: &Cli, mode: &Mode, io: &mut Io<'_>) -> Result<Exit, AppError> {
    let cfg = load_config(cli, &MappingArgs::default(), io.env).ok();
    let devices = scan_devices(io.env)?;
    let data = Value::Array(
        devices
            .iter()
            .map(|d| device_json(d, cfg.as_ref()))
            .collect(),
    );
    mode.emit(io.out, data, |v, m| {
        table(
            v,
            m,
            &["status", "kind", "vendor", "product", "path", "name"],
        )
    })?;
    Ok(Exit::Success)
}

fn rules_json(r: &Rules) -> Value {
    json!({
        "tap_timeout_ms": u64::try_from(r.tap_timeout().as_millis()).unwrap_or(u64::MAX),
        "dual_role": r.dual_roles().map(|d| json!({ "key": d.key.to_string(), "hold": d.hold.to_string(), "tap": d.tap.to_string() })).collect::<Vec<_>>(),
        "remap": r.remaps().map(|(s, t)| json!({ "from": s.to_string(), "to": t.to_string() })).collect::<Vec<_>>(),
        "protected": r.protected().map(|k| k.to_string()).collect::<Vec<_>>(),
    })
}

/// `config check`: parse and validate only; opens no input device and
/// creates no virtual device. Implements: ESC-SRS-035
fn config_check(
    cli: &Cli,
    mapping: &MappingArgs,
    mode: &Mode,
    io: &mut Io<'_>,
) -> Result<Exit, AppError> {
    let cfg = load_config(cli, mapping, io.env)?;
    if cfg.source.is_none() && mapping.map.is_none() {
        return Err(AppError::new(
            "NOT_FOUND",
            Exit::NotFound,
            "no configuration file found and no --map given",
            "escapepod config check --map 'Caps_Lock=Escape'",
        ));
    }
    warn_screen_reader(&cfg, io.env, mode, io);
    let data = json!({
        "path": cfg.source.as_ref().map(|p| p.display().to_string()),
        "mappings": rules_json(&cfg.global),
        "devices": cfg.devices.iter().map(|d| json!({
            "match": match &d.matcher { config::Matcher::Name(n) => json!({ "name": n }),
                                        config::Matcher::Id(v, p) => json!({ "id": format!("{v:04x}:{p:04x}") }) },
            "ignore": d.ignore,
            "mappings": rules_json(&d.rules),
        })).collect::<Vec<_>>(),
        "emergency_chord": cfg.chord.keys().map(|k| k.to_string()).collect::<Vec<_>>(),
        "shadowed": cfg.shadowed.iter().map(ToString::to_string).collect::<Vec<_>>(),
    });
    mode.emit(io.out, data, |v, m| {
        let src = v["path"].as_str().unwrap_or("the command line");
        let mut s = format!(
            "{} configuration from {src} is valid\n",
            m.paint(Token::Success, "[OK]")
        );
        s.push_str(&mappings_text(&v["mappings"]));
        s
    })?;
    Ok(Exit::Success)
}

/// `daemon run`: the guards and the dry run. The run itself is the
/// backend's.
fn daemon_run(cli: &Cli, args: &RunArgs, mode: &Mode, io: &mut Io<'_>) -> Result<Exit, AppError> {
    let cfg = load_config(cli, &args.mapping, io.env)?;
    let g = &cli.global;
    // Implements: ESC-SRS-037
    if !g.dry_run && !g.yes && (!io.env.stdout_tty || io.env.is_agent()) {
        let why = if io.env.is_agent() {
            "an agent environment"
        } else {
            "a run whose output is not a terminal"
        };
        return Err(AppError::invalid(
            format!("daemon run grabs every matched keyboard; {why} must confirm with --yes"),
            format!("{} --yes", mode.command),
        ));
    }
    warn_screen_reader(&cfg, io.env, mode, io);
    if cfg.is_empty() {
        if !g.dry_run {
            return Err(AppError::new(
                "MISSING_ARGUMENT",
                Exit::Usage,
                "no mapping configured: no configuration file and no --map",
                "escapepod daemon run --map 'Caps_Lock=Escape' --dry-run",
            ));
        }
        mode.diag(
            io.err,
            Severity::Warn,
            "NO_MAPPING",
            "the plan maps no key",
            Some("escapepod daemon run --map 'Caps_Lock=Escape' --dry-run"),
        );
    }
    if g.dry_run {
        return dry_run(&cfg, args.backend, mode, io);
    }
    let env = io.env.clone();
    let reload = move || load_config(cli, &args.mapping, &env);
    crate::service::run(cfg, args.backend, mode, io, &reload)
}

/// `daemon run --dry-run`: the devices it would grab and the mappings it
/// would apply; reads sysfs only and grabs nothing.
/// Implements: ESC-SRS-039, ESC-SRS-094
fn dry_run(
    cfg: &Config,
    backend: BackendArg,
    mode: &Mode,
    io: &mut Io<'_>,
) -> Result<Exit, AppError> {
    let devices = scan_devices(io.env).unwrap_or_default();
    let data = json!({
        "backend": match backend { BackendArg::Auto => "auto", BackendArg::Evdev => "evdev", BackendArg::X11 => "x11" },
        "devices": devices.iter().filter(|d| d.kind != DeviceKind::Other).map(|d| device_json(d, Some(cfg))).collect::<Vec<_>>(),
        "mappings": rules_json(&cfg.global),
        "emergency_chord": cfg.chord.keys().map(|k| k.to_string()).collect::<Vec<_>>(),
    });
    mode.emit(io.out, data, |v, m| {
        let mut s = format!(
            "{} dry run: nothing is grabbed\n",
            m.paint(Token::Structure, "[INFO]")
        );
        s.push_str(&table(
            &v["devices"],
            m,
            &["status", "kind", "path", "name"],
        ));
        s.push_str(&mappings_text(&v["mappings"]));
        s
    })?;
    Ok(Exit::Success)
}

/// A plain table of list data: one line per record, fields separated by
/// two spaces; parseable with awk. Under `--print0` records end in NUL.
fn table(v: &Value, m: &Mode, columns: &[&str]) -> String {
    let Some(rows) = v.as_array() else {
        return String::new();
    };
    let cell = |r: &Value, c: &str| match &r[c] {
        Value::String(s) => s.clone(),
        Value::Null => "-".to_owned(),
        other => other.to_string(),
    };
    let widths: Vec<usize> = columns
        .iter()
        .map(|c| {
            rows.iter()
                .map(|r| cell(r, c).chars().count())
                .chain([c.len()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: Vec<String>| -> String {
        let last = cells.len().saturating_sub(1);
        cells
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                if i == last {
                    c
                } else {
                    format!("{c:<w$}", w = widths[i])
                }
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    let mut s = String::new();
    if !m.print0 {
        s.push_str(&line(columns.iter().map(|c| c.to_uppercase()).collect()));
        s.push('\n');
    }
    for r in rows {
        s.push_str(line(columns.iter().map(|c| cell(r, c)).collect()).trim_end());
        s.push(m.record_end());
    }
    s
}

fn mappings_text(v: &Value) -> String {
    let mut s = format!("tap timeout: {} ms\n", v["tap_timeout_ms"]);
    for d in v["dual_role"].as_array().into_iter().flatten() {
        let _ = writeln!(
            s,
            "dual-role: {} (hold {}, tap {})",
            str_of(&d["key"]),
            str_of(&d["hold"]),
            str_of(&d["tap"])
        );
    }
    for r in v["remap"].as_array().into_iter().flatten() {
        let _ = writeln!(s, "remap: {} -> {}", str_of(&r["from"]), str_of(&r["to"]));
    }
    for p in v["protected"].as_array().into_iter().flatten() {
        let _ = writeln!(s, "protected: {}", str_of(p));
    }
    s
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or("?")
}

fn describe_text(v: &Value, _mode: &Mode) -> String {
    let mut s = format!(
        "escapepod {} — {}\n\n",
        str_of(&v["version"]),
        str_of(&v["description"])
    );
    for c in v["commands"].as_array().into_iter().flatten() {
        let _ = writeln!(
            s,
            "  {:<24} {}",
            str_of(&c["name"]),
            str_of(&c["description"])
        );
    }
    let _ = writeln!(
        s,
        "\nmode: {} (format {})\n{}",
        str_of(&v["mode"]["profile"]),
        str_of(&v["mode"]["format"]),
        crate::cli::footer()
    );
    s
}

// Rust guideline compliant 2026-05-18
