<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Escape Pod — key-data destination review

Verifies: ESC-SRS-058, by review of design (Standard §21.1).

**Requirement.** The daemon shall write key event data only to its virtual device and to the
status socket.

**Reviewed** 2026-11-01 at the M3 branch, by Claude Code for the maintainer, against the manual's
`Privacy model` and every write path in `crates/escapepod/src` and `crates/escapepod-evdev/src`
(found with `rg 'println!|eprintln!|write!|writeln!|File::create|OpenOptions|fs::write|\.emit\(|send\('`).

| Path | What it writes | Key data? |
|---|---|---|
| `escapepod-evdev/src/real.rs` `UinputKeyboard::emit` | Engine output to the virtual keyboard | Yes: the permitted destination |
| `daemon.rs` `flush` and `Drop` | The same, through the `Sink` trait | Yes: the virtual keyboard |
| `service.rs` `Clients::accept` | One status line: devices, grab state, *count* of held keys | No key codes (test `socket_records_carry_managed_keys_only` checks the status line) |
| `service.rs` `Clients::publish` | `monitor::Record`s of **managed** keys only | Yes: the permitted socket; unmanaged keys are never recorded (ESC-SRS-060) |
| `daemon.rs` notes, written by `mode.diag` to standard error | Device names and paths, error text, `EMERGENCY_RELEASE` | No: no diagnostic formats a key |
| `output.rs`, `app.rs` `mode.emit` | Command data of `config check`, `device list`, `schema`, `describe`, `--version`, `--dry-run`, `daemon status` | Configured key *names* only, never events |
| `service.rs` `monitor` loop | `event monitor` records on standard output | Not the daemon: a separate command that reads devices itself, redacts unmanaged keys, and refuses `--all-keys` off a terminal (ESC-SRS-061, 062) |
| `config.rs`, `orca.rs` file writes | Test fixtures only (`#[cfg(test)]`) | No |

No path writes to a file, a log, the journal (beyond standard error, which carries no key data),
or the network (the service allows only `AF_UNIX` and `AF_NETLINK`, ESC-SRS-072).

**Result:** pass. Re-run this review whenever a write path is added (AGENTS.md: "Logging or
printing a key code that is not managed" is forbidden).
