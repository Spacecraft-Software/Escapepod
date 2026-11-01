// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command tree and the CLI Standard §3 global flags.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// The help footer: project URL and maintainer (CLI Standard §3).
const FOOTER: &str = "Project: https://EscapePod.SpacecraftSoftware.org/
Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>";

/// Escape Pod: tap a key for one key, hold it for another.
#[derive(Debug, Parser)]
#[command(
    name = "escapepod",
    about = "Tap a key for Escape, hold it for Control (the successor to xcape)",
    disable_version_flag = true,
    disable_help_subcommand = true,
    after_help = concat!(
        "Examples:\n",
        "  escapepod daemon run --map 'Caps_Lock=Escape' --yes\n",
        "  escapepod device list --json\n\n",
        "Project: https://EscapePod.SpacecraftSoftware.org/\n",
        "Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    )
)]
pub struct Cli {
    /// The global flags.
    #[command(flatten)]
    pub global: Global,
    /// The command.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The CLI Standard §3 global flags, accepted by every command.
/// Implements: ESC-SRS-023
#[derive(Debug, Args)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent switch the Standard defines"
)]
pub struct Global {
    /// Alias for --format json
    #[arg(long, global = true)]
    pub json: bool,
    /// Output format: json, jsonl, yaml, csv or explore
    #[arg(long, global = true, value_name = "FMT")]
    pub format: Option<String>,
    /// Restrict output to these fields (comma-separated)
    #[arg(long, global = true, value_name = "F1,F2")]
    pub fields: Option<String>,
    /// Report the plan and change nothing
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Lower the diagnostic floor to info
    #[arg(short, long, global = true)]
    pub verbose: bool,
    /// Raise the diagnostic floor to errors only
    #[arg(short, long, global = true)]
    pub quiet: bool,
    /// Disable ANSI colour (same as --color never)
    #[arg(long, global = true)]
    pub no_color: bool,
    /// When to colour human output: never, always or auto
    #[arg(long, global = true, value_name = "WHEN")]
    pub color: Option<ColorArg>,
    /// Print the version and attribution
    #[arg(long, global = true)]
    pub version: bool,
    /// Show absolute timestamps in human output (the default here)
    #[arg(long, global = true)]
    pub absolute_time: bool,
    /// Separate human list records with NUL
    #[arg(short = '0', long, global = true)]
    pub print0: bool,
    /// Confirm an action that needs it off a terminal
    #[arg(long, global = true, visible_alias = "force")]
    pub yes: bool,
    /// Accessible mode: linear output and the high-contrast theme
    #[arg(long, global = true)]
    pub accessible: bool,
    /// Configuration file (default: XDG, then /etc/escapepod/config.toml)
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

/// The values of `--color`.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorArg {
    /// Never colour.
    Never,
    /// Always colour human output.
    Always,
    /// Colour on a terminal unless the environment says otherwise.
    Auto,
}

/// The commands. Implements: ESC-SRS-022
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run or inspect the daemon
    #[command(subcommand)]
    Daemon(DaemonCmd),
    /// Watch key events and Escape Pod's decisions
    #[command(subcommand)]
    Event(EventCmd),
    /// List input devices
    #[command(subcommand)]
    Device(DeviceCmd),
    /// Validate the configuration
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Print the JSON Schema of every command
    #[command(after_help = concat!(
        "Examples:\n  escapepod schema\n  escapepod schema daemon run --json\n\n",
        "Project: https://EscapePod.SpacecraftSoftware.org/\n",
        "Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    ))]
    Schema {
        /// Limit to one noun: daemon, event, device, config
        noun: Option<String>,
        /// Limit to one verb of the noun
        verb: Option<String>,
    },
    /// Print the capability manifest
    #[command(after_help = concat!(
        "Examples:\n  escapepod describe\n  escapepod describe --json\n\n",
        "Project: https://EscapePod.SpacecraftSoftware.org/\n",
        "Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    ))]
    Describe,
    /// Alias of `daemon run` (human use only)
    Run(RunArgs),
}

/// `escapepod daemon ...`
#[derive(Debug, Subcommand)]
pub enum DaemonCmd {
    /// Grab the matched keyboards and apply the mapping
    #[command(after_help = concat!(
        "Examples:\n",
        "  escapepod daemon run --map 'Caps_Lock=Escape' --yes\n",
        "  escapepod daemon run --dry-run --json\n\n",
        "Project: https://EscapePod.SpacecraftSoftware.org/\n",
        "Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    ))]
    Run(RunArgs),
    /// Report the running daemon's state
    #[command(after_help = concat!(
        "Examples:\n  escapepod daemon status\n  escapepod daemon status --json\n\n",
        "Project: https://EscapePod.SpacecraftSoftware.org/\n",
        "Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    ))]
    Status {
        /// The status socket (default: the service's, else the session's)
        #[arg(long, value_name = "PATH")]
        socket: Option<PathBuf>,
    },
}

/// `escapepod event ...`
#[derive(Debug, Subcommand)]
pub enum EventCmd {
    /// Print key events of managed keys and the decisions made
    #[command(after_help = concat!(
        "Examples:\n  escapepod event monitor\n  escapepod event monitor --format jsonl\n\n",
        "Project: https://EscapePod.SpacecraftSoftware.org/\n",
        "Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    ))]
    Monitor {
        /// Also show the key codes of unmanaged keys (terminal only)
        #[arg(long)]
        all_keys: bool,
        /// The mapping options.
        #[command(flatten)]
        mapping: MappingArgs,
    },
}

/// `escapepod device ...`
#[derive(Debug, Subcommand)]
pub enum DeviceCmd {
    /// List input devices and whether the configuration matches them
    #[command(after_help = concat!(
        "Examples:\n  escapepod device list\n  escapepod device list --json --fields name,path\n\n",
        "Project: https://EscapePod.SpacecraftSoftware.org/\n",
        "Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    ))]
    List,
}

/// `escapepod config ...`
#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Validate the configuration without touching any device
    #[command(after_help = concat!(
        "Examples:\n",
        "  escapepod config check --config ~/.config/escapepod/config.toml\n",
        "  escapepod config check --map 'Caps_Lock=Escape' --json\n\n",
        "Project: https://EscapePod.SpacecraftSoftware.org/\n",
        "Maintained by Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>"
    ))]
    Check {
        /// The mapping options.
        #[command(flatten)]
        mapping: MappingArgs,
    },
}

/// The arguments of `daemon run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// The mapping options.
    #[command(flatten)]
    pub mapping: MappingArgs,
    /// The backend: auto, evdev or x11
    #[arg(long, value_enum, default_value_t = BackendArg::Auto)]
    pub backend: BackendArg,
}

/// The values of `--backend`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum BackendArg {
    /// evdev when devices are accessible, else X11, else fail
    Auto,
    /// The kernel input layer (exclusive grab, uinput)
    Evdev,
    /// The X11 fallback (RECORD, XTEST)
    X11,
}

/// Options that change the mapping, shared by run, check and monitor.
#[derive(Debug, Args, Clone, Default)]
pub struct MappingArgs {
    /// xcape mapping string: KEY=TAP[;KEY=TAP]...
    #[arg(long, value_name = "MAP")]
    pub map: Option<String>,
    /// Tap timeout in milliseconds, 50 to 2000 (xcape's -t)
    #[arg(long, value_name = "MS")]
    pub tap_timeout: Option<u64>,
}

/// The footer, for callers that render help themselves.
pub fn footer() -> &'static str {
    FOOTER
}

// Rust guideline compliant 2026-05-18
