// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The output layer: agent detection, the CLI Standard §5 output cascade,
//! the §6 JSON envelope, other formats, and severity-tagged diagnostics.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write};

use serde_json::{Map, Value, json};

use crate::error::AppError;
use crate::theme::{Theme, Token};

/// The process environment as the output layer sees it: variables and
/// whether each standard stream is a terminal. Tests build one by hand.
#[derive(Clone, Debug, Default)]
pub struct Env {
    vars: BTreeMap<String, String>,
    /// Standard output is a terminal.
    pub stdout_tty: bool,
    /// Standard input is a terminal.
    pub stdin_tty: bool,
}

impl Env {
    /// The environment of this process.
    pub fn from_process() -> Self {
        Self {
            vars: std::env::vars_os()
                .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
                .collect(),
            stdout_tty: io::stdout().is_terminal(),
            stdin_tty: io::stdin().is_terminal(),
        }
    }

    /// An environment with these variables, on a terminal.
    #[cfg(test)]
    pub fn tty(vars: &[(&str, &str)]) -> Self {
        Self {
            stdout_tty: true,
            stdin_tty: true,
            ..Self::with(vars)
        }
    }

    /// An environment with these variables and no terminal.
    #[cfg(test)]
    pub fn with(vars: &[(&str, &str)]) -> Self {
        Self {
            vars: vars
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            stdout_tty: false,
            stdin_tty: false,
        }
    }

    /// A variable's value when it is set and not empty.
    pub fn var(&self, key: &str) -> Option<&str> {
        self.vars
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    /// Whether this is an agent environment: `AI_AGENT` or `AGENT` set to
    /// any non-empty value, or `CI` set to anything but `0` or `false`.
    /// Presence-based: real harnesses set descriptive strings, not `1`.
    /// Implements: ESC-SRS-026
    pub fn is_agent(&self) -> bool {
        self.var("AI_AGENT").is_some() || self.var("AGENT").is_some() || self.is_ci()
    }

    /// Whether `AI_AGENT` or `AGENT` (not merely `CI`) is set.
    pub fn is_ai_agent(&self) -> bool {
        self.var("AI_AGENT").is_some() || self.var("AGENT").is_some()
    }

    /// Whether `CI` is truthy.
    pub fn is_ci(&self) -> bool {
        self.var("CI")
            .is_some_and(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
    }

    /// Whether the user asked for accessible mode (`SPACECRAFT_A11Y`).
    pub fn wants_accessible(&self) -> bool {
        self.var("SPACECRAFT_A11Y")
            .is_some_and(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
    }
}

/// An output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Human-readable text on a terminal.
    Human,
    /// One JSON document (the envelope).
    Json,
    /// One JSON value per line.
    Jsonl,
    /// YAML.
    Yaml,
    /// CSV, for list data.
    Csv,
}

impl Format {
    /// The format's name as `--format` takes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Jsonl => "jsonl",
            Self::Yaml => "yaml",
            Self::Csv => "csv",
        }
    }

    /// Whether this is a machine format.
    pub fn is_machine(self) -> bool {
        self != Self::Human
    }
}

/// A diagnostic severity, lowest first (CLI Standard diagnostics §1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Narration, shown under `--verbose`.
    Info,
    /// A side effect completed.
    Ok,
    /// A degradation the caller should know about.
    Warn,
    /// A failure.
    Error,
}

impl Severity {
    fn tag(self) -> &'static str {
        match self {
            Self::Info => "[INFO]",
            Self::Ok => "[OK]",
            Self::Warn => "[WARN]",
            Self::Error => "[ERROR]",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn token(self) -> Token {
        match self {
            Self::Info => Token::Structure,
            Self::Ok => Token::Success,
            Self::Warn => Token::Warning,
            Self::Error => Token::Error,
        }
    }
}

/// The colour choice of `--color`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChoice {
    /// Decide from the environment and the terminal.
    Auto,
    /// Always colour human output.
    Always,
    /// Never colour.
    Never,
}

/// The output-relevant global flags, gathered from the command line.
#[derive(Clone, Debug, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is an independent CLI Standard flag"
)]
pub struct Flags {
    /// `--format`, or `json` for `--json`.
    pub format: Option<String>,
    /// `--fields`.
    pub fields: Option<String>,
    /// `--verbose`.
    pub verbose: bool,
    /// `--quiet`.
    pub quiet: bool,
    /// `--color`, with `--no-color` as `Never`.
    pub color: Option<ColorChoice>,
    /// `--print0`.
    pub print0: bool,
    /// `--accessible`.
    pub accessible: bool,
}

/// A warning found while resolving the mode: its code and message.
pub type Warning = (&'static str, String);

/// How this invocation writes its output.
#[derive(Clone, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facets of the CLI Standard §5 cascade"
)]
pub struct Mode {
    /// The selected format.
    pub format: Format,
    /// Whether human output carries ANSI colour.
    pub color: bool,
    /// The lowest severity written to standard error.
    pub floor: Severity,
    /// The `--fields` filter.
    pub fields: Option<Vec<String>>,
    /// NUL-delimited human list output.
    pub print0: bool,
    /// Accessible mode (Standard §18): linear output, high-contrast theme.
    pub accessible: bool,
    /// The invocation as parsed, for envelopes and errors.
    pub command: String,
    /// Whether the process runs in an agent environment.
    pub agent: bool,
    /// Whether prompts are allowed.
    pub interactive: bool,
    theme: Theme,
}

impl Mode {
    /// Resolves the CLI Standard §5 cascade: an explicit format, then an
    /// agent environment, then a terminal, else JSON. Returns the warnings
    /// to emit once the mode is known.
    ///
    /// # Errors
    ///
    /// A usage error for `--quiet` with `--verbose`, or an unknown format.
    pub fn resolve(
        flags: &Flags,
        env: &Env,
        command: String,
    ) -> Result<(Self, Vec<Warning>), AppError> {
        let mut warnings = Vec::new();
        if flags.quiet && flags.verbose {
            return Err(AppError::invalid(
                "--quiet and --verbose cannot be combined",
                format!("{command} --quiet"),
            ));
        }
        let agent = env.is_agent();
        let format = match flags.format.as_deref() {
            Some("json") => Format::Json,
            Some("jsonl") => Format::Jsonl,
            Some("yaml") => Format::Yaml,
            Some("csv") => Format::Csv,
            Some("explore") => {
                warnings.push((
                    "TUI_FALLBACK",
                    "interactive explore mode is not available; falling back to `--format json`"
                        .to_owned(),
                ));
                Format::Json
            }
            Some(other) => {
                return Err(AppError::invalid(
                    format!("unknown output format `{other}`"),
                    format!("{command} --format json"),
                ));
            }
            None if agent || !env.stdout_tty => Format::Json,
            None => Format::Human,
        };
        let accessible = flags.accessible || env.wants_accessible();
        let color = format == Format::Human && !agent && color_enabled(flags.color, env);
        let floor = if flags.quiet {
            Severity::Error
        } else if flags.verbose {
            Severity::Info
        } else if env.is_ai_agent() {
            Severity::Warn
        } else {
            Severity::Ok
        };
        let fields = flags.fields.as_ref().map(|f| {
            f.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        });
        let theme = Theme::resolve(env.var("SPACECRAFT_THEME"), accessible);
        if let Err(slug) = &theme {
            warnings.push((
                "UNKNOWN_THEME",
                format!("unknown theme `{slug}`; using `steelbore`"),
            ));
        }
        let mode = Self {
            format,
            color,
            floor,
            fields,
            print0: flags.print0,
            accessible,
            command,
            agent,
            interactive: !agent && env.stdin_tty && env.stdout_tty,
            theme: theme.unwrap_or_else(|_| Theme::fallback(accessible)),
        };
        Ok((mode, warnings))
    }

    /// The machine-mode metadata block.
    pub fn metadata(&self) -> Value {
        json!({
            "tool": "escapepod",
            "version": env!("CARGO_PKG_VERSION"),
            "command": self.command,
            "timestamp": now_utc(),
        })
    }

    /// Writes command data: the envelope in machine formats, `human` text
    /// otherwise.
    ///
    /// # Errors
    ///
    /// A usage error when `--fields` names a field the data lacks, or an
    /// I/O error writing standard output.
    pub fn emit(
        &self,
        out: &mut dyn Write,
        data: Value,
        human: impl FnOnce(&Value, &Self) -> String,
    ) -> Result<(), AppError> {
        self.emit_with(out, data, &[], human)
    }

    /// [`Mode::emit`], with `extra` keys added to the metadata block.
    ///
    /// # Errors
    ///
    /// As [`Mode::emit`].
    pub fn emit_with(
        &self,
        out: &mut dyn Write,
        data: Value,
        extra: &[(&str, &str)],
        human: impl FnOnce(&Value, &Self) -> String,
    ) -> Result<(), AppError> {
        let mut meta = self.metadata();
        if let Some(o) = meta.as_object_mut() {
            for (k, v) in extra {
                o.insert((*k).to_owned(), (*v).into());
            }
        }
        let data = self.select(data)?;
        let text = match self.format {
            Format::Human => human(&data, self),
            Format::Json => {
                let env = json!({ "metadata": meta, "data": data });
                format!("{}\n", serde_json::to_string(&env).unwrap_or_default())
            }
            Format::Jsonl => match &data {
                Value::Array(items) => items.iter().fold(String::new(), |mut s, i| {
                    let _ = writeln!(s, "{}", serde_json::to_string(i).unwrap_or_default());
                    s
                }),
                other => format!("{}\n", serde_json::to_string(other).unwrap_or_default()),
            },
            Format::Yaml => yaml(&json!({ "metadata": meta, "data": data })),
            Format::Csv => csv(&data),
        };
        write_all(out, text.as_bytes())
    }

    /// Applies `--fields` to list or record data.
    fn select(&self, data: Value) -> Result<Value, AppError> {
        let Some(fields) = &self.fields else {
            return Ok(data);
        };
        let pick = |rec: &Map<String, Value>| -> Result<Value, AppError> {
            let mut m = Map::new();
            for f in fields {
                let v = rec.get(f).ok_or_else(|| {
                    AppError::invalid(
                        format!("unknown field `{f}`"),
                        format!(
                            "{} --fields {}",
                            self.command,
                            rec.keys().cloned().collect::<Vec<_>>().join(",")
                        ),
                    )
                })?;
                m.insert(f.clone(), v.clone());
            }
            Ok(Value::Object(m))
        };
        match data {
            Value::Array(items) => items
                .iter()
                .map(|i| i.as_object().map_or_else(|| Ok(i.clone()), pick))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Value::Object(m) => pick(&m),
            other => Ok(other),
        }
    }

    /// Writes an error: one compact JSON object in machine mode, a tagged
    /// line with a `hint:` continuation otherwise. Errors bypass the floor.
    pub fn error(&self, err: &mut dyn Write, e: &AppError) {
        let text = if self.format.is_machine() {
            format!(
                "{}\n",
                serde_json::to_string(&e.to_json(&self.command, &now_utc())).unwrap_or_default()
            )
        } else {
            format!(
                "{} {}\n  {} {}\n",
                self.paint(Token::Error, Severity::Error.tag()),
                e.message(),
                self.paint(Token::Accent, "hint:"),
                e.hint()
            )
        };
        let _ = err.write_all(text.as_bytes());
    }

    /// Writes a diagnostic of `severity` when it is at or above the floor.
    pub fn diag(
        &self,
        err: &mut dyn Write,
        severity: Severity,
        code: &str,
        message: &str,
        hint: Option<&str>,
    ) {
        if severity < self.floor {
            return;
        }
        let text = if self.format.is_machine() {
            let mut d = json!({
                "severity": severity.name(),
                "code": code,
                "message": message,
                "timestamp": now_utc(),
                "command": self.command,
            });
            if let (Some(h), Some(o)) = (hint, d.as_object_mut()) {
                o.insert("hint".into(), h.into());
            }
            format!("{}\n", json!({ "diagnostic": d }))
        } else {
            let mut s = format!(
                "{} {message}\n",
                self.paint(severity.token(), severity.tag())
            );
            if let Some(h) = hint {
                let _ = writeln!(s, "  {} {h}", self.paint(Token::Accent, "hint:"));
            }
            s
        };
        let _ = err.write_all(text.as_bytes());
    }

    /// Paints `text` with a theme token when colour is on.
    pub fn paint(&self, token: Token, text: &str) -> String {
        if self.color {
            self.theme.paint(token, text)
        } else {
            text.to_owned()
        }
    }

    /// The line separator for human list output: NUL under `--print0`.
    pub fn record_end(&self) -> char {
        if self.print0 { '\0' } else { '\n' }
    }
}

/// The CLI Standard colour precedence chain (output-modes §6), for human
/// output only. Implements: ESC-SRS-090
fn color_enabled(choice: Option<ColorChoice>, env: &Env) -> bool {
    match choice {
        Some(ColorChoice::Never) => false,
        Some(ColorChoice::Always) => true,
        Some(ColorChoice::Auto) | None => {
            if env.var("FORCE_COLOR").is_some() {
                true
            } else if env.var("NO_COLOR").is_some()
                || env.var("CLICOLOR") == Some("0")
                || env.var("TERM") == Some("dumb")
            {
                false
            } else {
                env.stdout_tty
            }
        }
    }
}

/// Writes bytes, mapping a closed pipe to a quiet failure.
///
/// # Errors
///
/// An `INTERNAL_ERROR` when standard output cannot be written.
pub fn write_all(out: &mut dyn Write, bytes: &[u8]) -> Result<(), AppError> {
    out.write_all(bytes)
        .and_then(|()| out.flush())
        .map_err(|e| {
            AppError::new(
                "INTERNAL_ERROR",
                crate::error::Exit::Failure,
                format!("cannot write output: {e}"),
                "escapepod describe",
            )
        })
}

/// The current time as ISO 8601 UTC with a `Z` suffix, whole seconds.
pub fn now_utc() -> String {
    jiff::Timestamp::now()
        .strftime("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

/// A small YAML emitter for JSON values: block style, every string quoted.
fn yaml(v: &Value) -> String {
    let mut s = String::from("---\n");
    yaml_into(&mut s, v, 0);
    s
}

fn yaml_into(s: &mut String, v: &Value, indent: usize) {
    let pad = " ".repeat(indent);
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, val) in m {
                if matches!(val, Value::Object(o) if !o.is_empty())
                    || matches!(val, Value::Array(a) if !a.is_empty())
                {
                    let _ = writeln!(s, "{pad}{k}:");
                    yaml_into(s, val, indent + 2);
                } else {
                    let _ = writeln!(s, "{pad}{k}: {}", scalar(val));
                }
            }
        }
        Value::Array(a) if !a.is_empty() => {
            for item in a {
                if matches!(item, Value::Object(o) if !o.is_empty())
                    || matches!(item, Value::Array(x) if !x.is_empty())
                {
                    let _ = writeln!(s, "{pad}-");
                    yaml_into(s, item, indent + 2);
                } else {
                    let _ = writeln!(s, "{pad}- {}", scalar(item));
                }
            }
        }
        other => {
            let _ = writeln!(s, "{pad}{}", scalar(other));
        }
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::Object(_) => "{}".to_owned(),
        Value::Array(_) => "[]".to_owned(),
        Value::String(t) => serde_json::to_string(t).unwrap_or_default(),
        other => other.to_string(),
    }
}

/// CSV of list data (RFC 4180 quoting); a record becomes one row, nested
/// values are written as JSON.
fn csv(data: &Value) -> String {
    let rows: Vec<&Map<String, Value>> = match data {
        Value::Array(a) => a.iter().filter_map(Value::as_object).collect(),
        Value::Object(m) => vec![m],
        _ => Vec::new(),
    };
    let Some(first) = rows.first() else {
        return String::new();
    };
    let header: Vec<&String> = first.keys().collect();
    let cell = |v: Option<&Value>| -> String {
        let raw = match v {
            Some(Value::String(t)) => t.clone(),
            Some(Value::Null) | None => String::new(),
            Some(other) => other.to_string(),
        };
        if raw.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", raw.replace('"', "\"\""))
        } else {
            raw
        }
    };
    let mut s = header
        .iter()
        .map(|h| cell(Some(&Value::String((*h).clone()))))
        .collect::<Vec<_>>()
        .join(",");
    s.push_str("\r\n");
    for r in rows {
        s.push_str(
            &header
                .iter()
                .map(|h| cell(r.get(*h)))
                .collect::<Vec<_>>()
                .join(","),
        );
        s.push_str("\r\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(flags: &Flags, env: &Env) -> Mode {
        Mode::resolve(flags, env, "escapepod test".into())
            .unwrap()
            .0
    }

    // Verifies: ESC-SRS-026
    #[test]
    fn agent_detection_is_presence_based() {
        let tty = |vars: &[(&str, &str)]| Env {
            stdout_tty: true,
            stdin_tty: true,
            ..Env::with(vars)
        };
        for vars in [
            &[("AI_AGENT", "claude-code_2-1-218_agent")][..],
            &[("AGENT", "1")],
            &[("CI", "true")],
        ] {
            assert_eq!(
                mode(&Flags::default(), &tty(vars)).format,
                Format::Json,
                "{vars:?}"
            );
        }
        for vars in [
            &[("AI_AGENT", "")][..],
            &[("CI", "false")],
            &[("CI", "0")],
            &[],
        ] {
            assert_eq!(
                mode(&Flags::default(), &tty(vars)).format,
                Format::Human,
                "{vars:?}"
            );
        }
        assert_eq!(
            mode(&Flags::default(), &Env::with(&[])).format,
            Format::Json,
            "a pipe selects JSON"
        );
        let explicit = Flags {
            format: Some("yaml".into()),
            ..Flags::default()
        };
        assert_eq!(
            mode(&explicit, &tty(&[("AI_AGENT", "x")])).format,
            Format::Yaml
        );
    }

    // Verifies: ESC-SRS-090
    #[test]
    fn agents_get_no_colour() {
        let tty = |vars: &[(&str, &str)]| Env {
            stdout_tty: true,
            stdin_tty: true,
            ..Env::with(vars)
        };
        assert!(mode(&Flags::default(), &tty(&[])).color);
        let always = Flags {
            color: Some(ColorChoice::Always),
            ..Flags::default()
        };
        assert!(!mode(&always, &tty(&[("AI_AGENT", "codex")])).color);
        assert!(!mode(&Flags::default(), &tty(&[("NO_COLOR", "1")])).color);
        assert!(!mode(&Flags::default(), &tty(&[("TERM", "dumb")])).color);
        assert!(
            mode(
                &Flags::default(),
                &tty(&[("NO_COLOR", "1"), ("FORCE_COLOR", "1")])
            )
            .color
        );
    }

    // Verifies: ESC-SRS-091
    #[test]
    fn agents_are_never_prompted() {
        let tty = Env {
            stdout_tty: true,
            stdin_tty: true,
            ..Env::with(&[("AGENT", "goose")])
        };
        assert!(!mode(&Flags::default(), &tty).interactive);
        let human = Env {
            stdout_tty: true,
            stdin_tty: true,
            ..Env::with(&[])
        };
        assert!(mode(&Flags::default(), &human).interactive);
    }

    #[test]
    fn floors_follow_flags_then_environment() {
        let q = Flags {
            quiet: true,
            ..Flags::default()
        };
        assert_eq!(mode(&q, &Env::with(&[])).floor, Severity::Error);
        let v = Flags {
            verbose: true,
            ..Flags::default()
        };
        assert_eq!(
            mode(&v, &Env::with(&[("AI_AGENT", "x")])).floor,
            Severity::Info
        );
        assert_eq!(
            mode(&Flags::default(), &Env::with(&[("AI_AGENT", "x")])).floor,
            Severity::Warn
        );
        assert_eq!(
            mode(&Flags::default(), &Env::with(&[("CI", "1")])).floor,
            Severity::Ok
        );
        let both = Flags {
            quiet: true,
            verbose: true,
            ..Flags::default()
        };
        Mode::resolve(&both, &Env::with(&[]), String::new()).unwrap_err();
    }

    #[test]
    fn formats_render() {
        let data = json!([{ "a": 1, "b": "x,y" }, { "a": 2, "b": "z" }]);
        assert_eq!(csv(&data), "a,b\r\n1,\"x,y\"\r\n2,z\r\n");
        assert!(yaml(&json!({ "k": [1, { "n": "v" }] })).contains("  -\n    n: \"v\""));
        let m = mode(
            &Flags {
                fields: Some("b".into()),
                ..Flags::default()
            },
            &Env::with(&[]),
        );
        assert_eq!(
            m.select(data.clone()).ok(),
            Some(json!([{ "b": "x,y" }, { "b": "z" }]))
        );
        let bad = mode(
            &Flags {
                fields: Some("zz".into()),
                ..Flags::default()
            },
            &Env::with(&[]),
        );
        bad.select(data).unwrap_err();
    }
}

// Rust guideline compliant 2026-05-18
