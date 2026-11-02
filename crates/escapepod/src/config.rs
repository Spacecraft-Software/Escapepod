// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The configuration file: its search order, its TOML model, and its
//! validation into per-device engine rules.
//!
//! ```toml
//! grab = "all"                             # or "listed": only [[device]] rules
//! tap_timeout_ms = 200                     # 50 to 2000
//! emergency_chord = ["BackSpace", "Escape", "Return"]
//! protected = []                           # keys never mapped
//!
//! [[dual_role]]
//! key = "Caps_Lock"                        # the physical key
//! hold = "Control_L"                       # optional; defaults to key
//! tap = "Escape"
//!
//! [[remap]]
//! from = "Menu"
//! to = "Super_R"
//!
//! [[device]]                               # per-device rules
//! name = "Kinesis Advantage2 Keyboard"     # or id = "29ea:0102"
//! dual_role = []                           # replaces the global list
//! ignore = false                           # true: never grab it
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use escapepod_core::{Chord, DEFAULT_CHORD, DualRole, KeyCode, Rules};
use escapepod_evdev::DeviceInfo;
use serde::Deserialize;
use toml::Spanned;

use crate::error::{AppError, Exit};
use crate::output::Env;

/// The shortest and longest accepted tap timeouts, in milliseconds
/// (ESC-SRS-031). Below 50 ms a tap is physically hard; above 2 s an
/// abandoned hold becomes a tap.
pub const TAP_TIMEOUT_RANGE_MS: std::ops::RangeInclusive<u64> = 50..=2000;

/// The system-wide configuration path, read by the service.
pub const SYSTEM_PATH: &str = "/etc/escapepod/config.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    grab: Option<Spanned<String>>,
    tap_timeout_ms: Option<Spanned<u64>>,
    emergency_chord: Option<Vec<Spanned<String>>>,
    #[serde(default)]
    protected: Vec<Spanned<String>>,
    #[serde(default)]
    dual_role: Vec<DualRoleEntry>,
    #[serde(default)]
    remap: Vec<RemapEntry>,
    #[serde(default)]
    button: Vec<ButtonEntry>,
    #[serde(default)]
    device: Vec<DeviceEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DualRoleEntry {
    key: Spanned<String>,
    hold: Option<Spanned<String>>,
    tap: Spanned<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemapEntry {
    from: Spanned<String>,
    to: Spanned<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ButtonEntry {
    button: Spanned<String>,
    keys: Vec<Spanned<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceEntry {
    name: Option<String>,
    id: Option<Spanned<String>>,
    #[serde(default)]
    ignore: bool,
    tap_timeout_ms: Option<Spanned<u64>>,
    dual_role: Option<Vec<DualRoleEntry>>,
    remap: Option<Vec<RemapEntry>>,
    button: Option<Vec<ButtonEntry>>,
}

/// How a device rule picks its device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Matcher {
    /// The kernel device name, exactly.
    Name(String),
    /// The vendor and product identifiers.
    Id(u16, u16),
}

impl Matcher {
    fn matches(&self, d: &DeviceInfo) -> bool {
        match self {
            Self::Name(n) => d.name == *n,
            Self::Id(v, p) => d.vendor == *v && d.product == *p,
        }
    }
}

/// The mapping lists before they become [`Rules`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Mapping {
    dual: Vec<DualRole>,
    remap: Vec<(KeyCode, KeyCode)>,
    buttons: Vec<(KeyCode, Vec<KeyCode>)>,
    tap_timeout: Option<Duration>,
}

/// One validated `[[device]]` rule.
#[derive(Clone, Debug)]
pub struct DeviceRule {
    /// Which device it applies to.
    pub matcher: Matcher,
    /// Whether the device is left alone.
    pub ignore: bool,
    /// The rules for the device.
    pub rules: Rules,
}

/// Command-line overrides of the configuration.
#[derive(Clone, Debug, Default)]
pub struct Overrides {
    /// `--map`: replaces the global dual-role list.
    pub map: Option<Vec<DualRole>>,
    /// `--tap-timeout`, in milliseconds: replaces every tap timeout.
    pub tap_timeout_ms: Option<u64>,
}

/// The validated configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// The file it came from, if any.
    pub source: Option<PathBuf>,
    /// The rules for any keyboard no device rule matches.
    pub global: Rules,
    /// The emergency chord.
    pub chord: Chord,
    /// The device rules, first match wins.
    pub devices: Vec<DeviceRule>,
    /// Managed keys left unmapped because they are protected.
    pub shadowed: Vec<KeyCode>,
    /// `grab = "listed"`: only keyboards a `[[device]]` rule names are
    /// grabbed; the default, `"all"`, grabs every keyboard.
    pub listed_only: bool,
}

impl Config {
    /// Whether any configured rule maps a pointer button.
    pub fn has_buttons(&self) -> bool {
        self.global.buttons().next().is_some()
            || self
                .devices
                .iter()
                .any(|d| d.rules.buttons().next().is_some())
    }
    /// The rules for `device`, or `None` when it is ignored.
    pub fn rules_for(&self, device: &DeviceInfo) -> Option<&Rules> {
        match self.devices.iter().find(|r| r.matcher.matches(device)) {
            Some(r) if r.ignore => None,
            Some(r) => Some(&r.rules),
            None if self.listed_only => None,
            None => Some(&self.global),
        }
    }

    /// Whether the configuration maps anything at all.
    pub fn is_empty(&self) -> bool {
        self.global.dual_roles().next().is_none()
            && self.global.remaps().next().is_none()
            && self.global.buttons().next().is_none()
            && self.devices.iter().all(|d| {
                d.rules.dual_roles().next().is_none()
                    && d.rules.remaps().next().is_none()
                    && d.rules.buttons().next().is_none()
            })
    }
}

/// Finds the configuration file: `--config`, else
/// `$XDG_CONFIG_HOME/escapepod/config.toml` (or `~/.config/...`), else
/// `/etc/escapepod/config.toml`. `None` when no file exists and none was
/// named. Implements: ESC-SRS-029
pub fn locate(explicit: Option<&Path>, env: &Env, system: &Path) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    let user = env
        .var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env.var("HOME").map(|h| Path::new(h).join(".config")))
        .map(|d| d.join("escapepod/config.toml"));
    user.into_iter()
        .chain([system.to_path_buf()])
        .find(|p| p.is_file())
}

/// Reads and validates the configuration at `path` (none: an empty
/// configuration), applies `overrides`, and protects `protected`.
///
/// # Errors
///
/// `NOT_FOUND` (exit 3) when a named file is missing, `INVALID_ARGUMENT`
/// (exit 2) with the line and column for anything invalid.
pub fn load(
    path: Option<&Path>,
    overrides: &Overrides,
    protected: &[KeyCode],
) -> Result<Config, AppError> {
    let (text, file) = match path {
        Some(p) => read(p)?,
        None => (String::new(), File::default()),
    };
    let ctx = Ctx { path, text: &text };
    let mut global = ctx.mapping(
        &file.dual_role,
        &file.remap,
        &file.button,
        file.tap_timeout_ms.as_ref(),
    )?;
    if let Some(map) = &overrides.map {
        global.dual.clone_from(map);
    }
    let chord = match &file.emergency_chord {
        Some(keys) => {
            let keys = keys
                .iter()
                .map(|k| ctx.key(k))
                .collect::<Result<Vec<_>, _>>()?;
            Chord::new(keys).map_err(|_empty| ctx.at(None, "the emergency chord has no keys"))?
        }
        None => Chord::new(DEFAULT_CHORD).unwrap_or_default(),
    };
    let mut protect: Vec<KeyCode> = file
        .protected
        .iter()
        .map(|k| ctx.key(k))
        .collect::<Result<_, _>>()?;
    protect.extend_from_slice(protected);
    let timeout_override = overrides
        .tap_timeout_ms
        .map(|ms| check_timeout(ms, None, &ctx))
        .transpose()?;
    let build = |m: &Mapping| -> Result<Rules, AppError> {
        let mut b = Rules::builder();
        for d in &m.dual {
            b = b.dual_role(d.key, d.hold, d.tap);
        }
        for &(s, t) in &m.remap {
            b = b.remap(s, t);
        }
        for (button, keys) in &m.buttons {
            b = b.button(*button, keys.clone());
        }
        for &p in &protect {
            b = b.protect(p);
        }
        if let Some(t) = timeout_override.or(m.tap_timeout) {
            b = b.tap_timeout(t);
        }
        b.build().map_err(|e| ctx.at(None, &e.to_string()))
    };
    let global_rules = build(&global)?;
    let mut devices = Vec::new();
    for d in &file.device {
        let matcher = match (&d.name, &d.id) {
            (Some(n), None) => Matcher::Name(n.clone()),
            (None, Some(id)) => parse_id(id).ok_or_else(|| {
                ctx.at(
                    Some(id.span().start),
                    "a device id is VVVV:PPPP in hexadecimal",
                )
            })?,
            _ => return Err(ctx.at(None, "a [[device]] needs exactly one of `name` or `id`")),
        };
        let own = ctx.mapping(
            d.dual_role.as_deref().unwrap_or(&file.dual_role),
            d.remap.as_deref().unwrap_or(&file.remap),
            d.button.as_deref().unwrap_or(&file.button),
            d.tap_timeout_ms.as_ref().or(file.tap_timeout_ms.as_ref()),
        )?;
        let mapping = if d.dual_role.is_none() && overrides.map.is_some() {
            Mapping {
                dual: global.dual.clone(),
                ..own
            }
        } else {
            own
        };
        devices.push(DeviceRule {
            matcher,
            ignore: d.ignore,
            rules: build(&mapping)?,
        });
    }
    let shadowed = shadowed(&global_rules, &devices);
    let listed_only = parse_grab(file.grab.as_ref(), &ctx)?;
    Ok(Config {
        listed_only,
        source: path.map(Path::to_path_buf),
        global: global_rules,
        chord,
        devices,
        shadowed,
    })
}

fn parse_grab(grab: Option<&Spanned<String>>, ctx: &Ctx<'_>) -> Result<bool, AppError> {
    match grab
        .as_ref()
        .map(|g| (g.get_ref().as_str(), g.span().start))
    {
        None | Some(("all", _)) => Ok(false),
        Some(("listed", _)) => Ok(true),
        Some((_, at)) => Err(ctx.at(Some(at), "`grab` is \"all\" or \"listed\"")),
    }
}

/// Reads and parses the file at `p`.
fn read(p: &Path) -> Result<(String, File), AppError> {
    let text = std::fs::read_to_string(p).map_err(|e| {
        let (code, exit) = match e.kind() {
            std::io::ErrorKind::NotFound => ("NOT_FOUND", Exit::NotFound),
            std::io::ErrorKind::PermissionDenied => ("PERMISSION_DENIED", Exit::Permission),
            _ => ("INTERNAL_ERROR", Exit::Failure),
        };
        AppError::new(
            code,
            exit,
            format!("cannot read {}: {e}", p.display()),
            "escapepod config check --config <path>",
        )
        .with("path", p.display().to_string())
    })?;
    let file: File = toml::from_str(&text).map_err(|e| toml_error(p, &text, &e))?;
    Ok((text, file))
}

/// Every managed key left inactive because it is protected, in any rule.
fn shadowed(global: &Rules, devices: &[DeviceRule]) -> Vec<KeyCode> {
    let mut keys: Vec<KeyCode> = global
        .shadowed()
        .chain(devices.iter().flat_map(|d| d.rules.shadowed()))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Validation context: the file, for line and column of a span.
struct Ctx<'a> {
    path: Option<&'a Path>,
    text: &'a str,
}

impl Ctx<'_> {
    fn mapping(
        &self,
        dual: &[DualRoleEntry],
        remap: &[RemapEntry],
        buttons: &[ButtonEntry],
        timeout: Option<&Spanned<u64>>,
    ) -> Result<Mapping, AppError> {
        let dual = dual
            .iter()
            .map(|d| {
                let key = self.key(&d.key)?;
                let hold = d
                    .hold
                    .as_ref()
                    .map(|h| self.key(h))
                    .transpose()?
                    .unwrap_or(key);
                Ok(DualRole {
                    key,
                    hold,
                    tap: self.key(&d.tap)?,
                })
            })
            .collect::<Result<_, AppError>>()?;
        let remap = remap
            .iter()
            .map(|r| Ok((self.key(&r.from)?, self.key(&r.to)?)))
            .collect::<Result<_, AppError>>()?;
        let buttons = buttons
            .iter()
            .map(|entry| {
                let button = self.key(&entry.button)?;
                if !(0x110..=0x11f).contains(&button.code()) {
                    return Err(self.at(
                        Some(entry.button.span().start),
                        "button must be a mouse button",
                    ));
                }
                let keys = entry
                    .keys
                    .iter()
                    .map(|key| {
                        let parsed = self.key(key)?;
                        if (0x100..0x160).contains(&parsed.code()) {
                            return Err(self.at(
                                Some(key.span().start),
                                "shortcut keys must be keyboard keys",
                            ));
                        }
                        Ok(parsed)
                    })
                    .collect::<Result<Vec<_>, AppError>>()?;
                if keys.is_empty() || keys.len() > 8 {
                    return Err(self.at(
                        Some(entry.button.span().start),
                        "a button shortcut needs one to eight keys",
                    ));
                }
                Ok((button, keys))
            })
            .collect::<Result<_, AppError>>()?;
        let tap_timeout = timeout
            .map(|t| check_timeout(*t.get_ref(), Some(t.span().start), self))
            .transpose()?;
        Ok(Mapping {
            dual,
            remap,
            buttons,
            tap_timeout,
        })
    }

    fn key(&self, name: &Spanned<String>) -> Result<KeyCode, AppError> {
        KeyCode::from_name(name.get_ref()).ok_or_else(|| {
            self.at(
                Some(name.span().start),
                &format!("unknown key name `{}`", name.get_ref()),
            )
        })
    }

    /// An `INVALID_ARGUMENT` error at byte `offset` of the file.
    fn at(&self, offset: Option<usize>, message: &str) -> AppError {
        let file = self.path.map_or_else(
            || "the command line".to_owned(),
            |p| p.display().to_string(),
        );
        let mut e = AppError::invalid(
            format!("{message} in {file}"),
            self.path.map_or_else(
                || "escapepod config check --help".to_owned(),
                |p| format!("escapepod config check --config {}", p.display()),
            ),
        );
        if let Some(p) = self.path {
            e = e.with("path", p.display().to_string());
        }
        if let Some(o) = offset {
            let (line, column) = line_column(self.text, o);
            e = e.with("line", line).with("column", column);
        }
        e
    }
}

fn check_timeout(ms: u64, offset: Option<usize>, ctx: &Ctx<'_>) -> Result<Duration, AppError> {
    if TAP_TIMEOUT_RANGE_MS.contains(&ms) {
        Ok(Duration::from_millis(ms))
    } else if offset.is_some() {
        Err(ctx.at(
            offset,
            &format!("tap timeout {ms} ms is outside 50 to 2000 ms"),
        ))
    } else {
        Err(AppError::invalid(
            format!("tap timeout {ms} ms is outside 50 to 2000 ms"),
            "escapepod daemon run --tap-timeout 200",
        ))
    }
}

fn parse_id(id: &Spanned<String>) -> Option<Matcher> {
    let (v, p) = id.get_ref().split_once(':')?;
    Some(Matcher::Id(
        u16::from_str_radix(v, 16).ok()?,
        u16::from_str_radix(p, 16).ok()?,
    ))
}

/// The one-based line and column (in characters) of byte `offset`.
pub fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let mut end = offset.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let before = &text[..end];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, column)
}

/// Turns a TOML parse error into an `INVALID_ARGUMENT` with the line and
/// column of the offending key (ESC-SRS-030, ESC-SRS-092).
fn toml_error(path: &Path, text: &str, e: &toml::de::Error) -> AppError {
    let first = e
        .message()
        .lines()
        .next()
        .unwrap_or("invalid TOML")
        .trim_end_matches('.')
        .to_owned();
    let mut err = AppError::invalid(
        format!("{first} in {}", path.display()),
        format!("escapepod config check --config {}", path.display()),
    )
    .with("path", path.display().to_string());
    if let Some(span) = e.span() {
        let (line, column) = line_column(text, span.start);
        err = AppError::invalid(
            format!(
                "{first} at line {line}, column {column} of {}",
                path.display()
            ),
            format!(
                "human: fix line {line} of {}, then run escapepod config check --config {}",
                path.display(),
                path.display()
            ),
        )
        .with("path", path.display().to_string())
        .with("line", line)
        .with("column", column);
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    // Verifies: ESC-SRS-112
    fn button_rules_inherit_and_validate() {
        let path = write(
            r#"
[[button]]
button = "BTN_SIDE"
keys = ["KEY_LEFTCTRL", "KEY_RIGHT"]
[[device]]
id = "1234:5678"
[[device.button]]
button = "BTN_EXTRA"
keys = ["KEY_LEFT"]
"#,
        );
        let cfg = load(Some(&path), &Overrides::default(), &[]).unwrap();
        let side = KeyCode::from_name("BTN_SIDE").unwrap();
        let extra = KeyCode::from_name("BTN_EXTRA").unwrap();
        assert_eq!(cfg.global.button(side).unwrap().len(), 2);
        assert!(cfg.devices[0].rules.button(side).is_none());
        assert_eq!(cfg.devices[0].rules.button(extra).unwrap().len(), 1);
        for content in [
            "[[button]]\nbutton='BTN_SIDE'\nkeys=[]\n",
            "[[button]]\nbutton='KEY_A'\nkeys=['KEY_B']\n",
            "[[button]]\nbutton='BTN_SIDE'\nkeys=['BTN_EXTRA']\n",
            "[[button]]\nbutton='BTN_SIDE'\nkeys=['KEY_A','KEY_A']\n",
        ] {
            let invalid = write(content);
            load(Some(&invalid), &Overrides::default(), &[]).unwrap_err();
        }
    }

    fn write(text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "escapepod-config-{}-{}",
            std::process::id(),
            text.len()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("config.toml");
        let _ = std::fs::File::create(&p).and_then(|mut f| f.write_all(text.as_bytes()));
        p
    }

    fn dev(name: &str, vendor: u16, product: u16) -> DeviceInfo {
        DeviceInfo {
            path: "/dev/input/event0".into(),
            name: name.into(),
            vendor,
            product,
            kind: escapepod_evdev::DeviceKind::Keyboard,
        }
    }

    #[test]
    fn full_file_validates() {
        let p = write(
            "tap_timeout_ms = 150\nemergency_chord = [\"F1\", \"F2\"]\n\n[[dual_role]]\nkey = \"Caps_Lock\"\nhold = \"Control_L\"\ntap = \"Escape\"\n\n[[remap]]\nfrom = \"Menu\"\nto = \"Super_R\"\n\n[[device]]\nid = \"29ea:0102\"\nignore = true\n",
        );
        let c = load(Some(&p), &Overrides::default(), &[]);
        let c = c.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(c.global.tap_timeout(), Duration::from_millis(150));
        assert!(c.global.is_managed(KeyCode::CAPSLOCK));
        assert!(c.rules_for(&dev("x", 0x29ea, 0x0102)).is_none());
        assert!(c.rules_for(&dev("x", 1, 2)).is_some());
        assert_eq!(c.chord.keys().count(), 2);
    }

    // Verifies: ESC-SRS-030, ESC-SRS-092
    #[test]
    fn unknown_keys_fail_with_line_and_column() {
        let p = write(
            "tap_timeout_ms = 200\n\n[[dual_role]]\nkey = \"Caps_Lock\"\ntapp = \"Escape\"\n",
        );
        let e = load(Some(&p), &Overrides::default(), &[]).err();
        let e = e.unwrap_or_else(|| AppError::invalid("", ""));
        assert_eq!(e.exit(), Exit::Usage);
        let v = e.to_json("c", "t");
        assert_eq!(v["error"]["line"], 5, "{v}");
        assert_eq!(v["error"]["column"], 1, "{v}");
        assert!(e.message().contains("tapp"), "{}", e.message());
        let top = write("tap_timeout = 200\n");
        let v = load(Some(&top), &Overrides::default(), &[])
            .err()
            .map(|e| e.to_json("c", "t"));
        assert_eq!(
            v.map(|v| (v["error"]["line"].clone(), v["error"]["column"].clone())),
            Some((1.into(), 1.into()))
        );
    }

    // Verifies: ESC-SRS-031
    #[test]
    fn tap_timeout_range_is_enforced() {
        for (ms, ok) in [(49, false), (50, true), (2000, true), (2001, false)] {
            let p = write(&format!("tap_timeout_ms = {ms}\n"));
            assert_eq!(
                load(Some(&p), &Overrides::default(), &[]).is_ok(),
                ok,
                "file {ms}"
            );
            let o = Overrides {
                tap_timeout_ms: Some(ms),
                ..Overrides::default()
            };
            let r = load(None, &o, &[]);
            assert_eq!(
                r.as_ref().map(|_| ()).map_err(AppError::exit),
                if ok { Ok(()) } else { Err(Exit::Usage) },
                "flag {ms}"
            );
        }
    }

    // Verifies: ESC-SRS-033
    #[test]
    fn tap_timeout_flag_overrides_the_file() {
        let p = write("tap_timeout_ms = 300\n[[device]]\nname = \"kb\"\ntap_timeout_ms = 400\n");
        let o = Overrides {
            tap_timeout_ms: Some(120),
            ..Overrides::default()
        };
        let c = load(Some(&p), &o, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(c.global.tap_timeout(), Duration::from_millis(120));
        assert_eq!(
            c.rules_for(&dev("kb", 0, 0)).map(Rules::tap_timeout),
            Some(Duration::from_millis(120))
        );
    }

    // Verifies: ESC-SRS-034
    #[test]
    fn device_rules_match_by_name_or_id() {
        let p = write(
            "[[dual_role]]\nkey = \"Caps_Lock\"\ntap = \"Escape\"\n[[device]]\nname = \"Laptop\"\ndual_role = []\n[[device]]\nid = \"046d:c52b\"\nremap = [{ from = \"Caps_Lock\", to = \"Control_L\" }]\ndual_role = []\n",
        );
        let c = load(Some(&p), &Overrides::default(), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            c.rules_for(&dev("Laptop", 1, 1))
                .is_some_and(|r| !r.is_managed(KeyCode::CAPSLOCK))
        );
        let logi = c.rules_for(&dev("Other", 0x046d, 0xc52b));
        assert!(logi.is_some_and(
            |r| r.role(KeyCode::CAPSLOCK) == escapepod_core::Role::Remap(KeyCode::LEFTCTRL)
        ));
        assert!(
            c.rules_for(&dev("Other", 1, 1))
                .is_some_and(|r| r.is_managed(KeyCode::CAPSLOCK))
        );
    }

    #[test]
    fn listed_grab_skips_unlisted_keyboards() {
        let p = write("grab = \"listed\"\n[[device]]\nid = \"5350:5445\"\n");
        let c = load(Some(&p), &Overrides::default(), &[]).unwrap();
        assert!(c.rules_for(&dev("x", 0x5350, 0x5445)).is_some());
        assert!(c.rules_for(&dev("laptop", 1, 1)).is_none());
        let bad = write("grab = \"some\"\n");
        assert!(
            load(Some(&bad), &Overrides::default(), &[]).is_err_and(|e| e.exit() == Exit::Usage)
        );
    }

    // Verifies: ESC-SRS-049
    #[test]
    fn configuration_replaces_the_emergency_chord() {
        let p = write("emergency_chord = [\"KEY_F11\", \"KEY_F12\"]\n");
        let c = load(Some(&p), &Overrides::default(), &[]).unwrap_or_else(|e| panic!("{e}"));
        let keys: Vec<String> = c.chord.keys().map(|k| k.to_string()).collect();
        assert_eq!(keys, ["KEY_F11", "KEY_F12"]);
        let empty = write("emergency_chord = []\n");
        load(Some(&empty), &Overrides::default(), &[]).unwrap_err();
    }

    // Verifies: ESC-SRS-029
    #[test]
    fn search_order_is_flag_then_xdg_then_etc() {
        let user = write("# user\n");
        let xdg = user
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let _ = std::fs::create_dir_all(xdg.join("escapepod"));
        let _ = std::fs::copy(&user, xdg.join("escapepod/config.toml"));
        let env = Env::with(&[("XDG_CONFIG_HOME", xdg.to_str().unwrap_or_default())]);
        let system = write("# system with a longer body\n");
        assert_eq!(
            locate(Some(Path::new("/x.toml")), &env, &system),
            Some(PathBuf::from("/x.toml"))
        );
        assert_eq!(
            locate(None, &env, &system),
            Some(xdg.join("escapepod/config.toml"))
        );
        assert_eq!(locate(None, &Env::with(&[]), &system), Some(system.clone()));
        assert_eq!(
            locate(
                None,
                &Env::with(&[]),
                Path::new("/nonexistent/escapepod.toml")
            ),
            None
        );
    }

    #[test]
    fn map_override_and_errors() {
        let o = Overrides {
            map: Some(crate::keymap::parse("Control_L=Escape").unwrap_or_default()),
            ..Overrides::default()
        };
        let c = load(None, &o, &[KeyCode::LEFTCTRL]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(c.shadowed, [KeyCode::LEFTCTRL]);
        let missing = load(
            Some(Path::new("/nonexistent/escapepod.toml")),
            &Overrides::default(),
            &[],
        );
        assert_eq!(missing.err().map(|e| e.exit()), Some(Exit::NotFound));
        let bad_name = write("[[remap]]\nfrom = \"Caps_Lok\"\nto = \"Escape\"\n");
        let v = load(Some(&bad_name), &Overrides::default(), &[])
            .err()
            .map(|e| e.to_json("c", "t"));
        assert_eq!(v.map(|v| v["error"]["line"].clone()), Some(2.into()));
        assert_eq!(line_column("ab\ncd", 4), (2, 2));
    }
}

// Rust guideline compliant 2026-05-18
