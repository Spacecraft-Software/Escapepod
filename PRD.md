<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->
<!-- GFM Document
     title:      Escape Pod — PRD
     author:     Mohamed Hammad & Spacecraft Software
     date:       2026-10-08
     version:    0.1.0
     license:    CC-BY-SA-4.0
     project:    Escape Pod
     website:    https://EscapePod.SpacecraftSoftware.org/
-->

# Escape Pod — PRD

MVP: M0–M6 — Escape Pod v0.1

**Maintainer:** Mohamed Hammad — <Mohamed.Hammad@SpacecraftSoftware.org>\
**Repository:** `Spacecraft-Software/Escapepod` (GitHub) · `/spacecraft-software/escapepod` (local)\
**Assurance:** Category B (Standard §19) · **Posture:** Personal / Hobby (§5.1)\
**Requirement prefix:** `ESC`\
**Status:** Planning — requirements `draft`, awaiting G1

Escape Pod is a Linux daemon that gives one key two jobs: tapped on its own it sends one key
(Escape), held with other keys it acts as a modifier (Control). It is the Spacecraft Software
successor to [xcape](https://github.com/ollef/xcape), rebuilt in Rust on the kernel input layer so
it works under X11, Wayland, and the console, with an X11 fallback for sessions without
`/dev/input` access.

This file is the §17.5 tracking document. It sequences the work; it does not define it. **The
requirements themselves — with rationale, source, priority, verification method, and status — live
in [`doc/escapepod.texi`](doc/escapepod.texi), nodes `Needs` and `Requirements`** (Standard §20.1).
Every task item below carries the §20.3 identifier of the requirement it discharges.


## Contents

- [Decisions](#decisions)
- [Architecture](#architecture)
- [Interfaces](#interfaces)
- [Out of scope](#out-of-scope)
- [Open items for G2](#open-items-for-g2)
- [Milestones M0–M6](#m0--foundation-and-compliance)


## Decisions

Settled with the maintainer on 2026-10-08.

| Topic | Decision |
|-------|----------|
| Name | **Escape Pod** — binary and crates `escapepod`, `escapepod-core`, `escapepod-evdev`, `escapepod-x11` (all free on crates.io at decision time) |
| Scope | xcape+: dual-role keys and one-to-one remaps. No layers, macros, or chords — ever |
| Tap model | Pass-through only: the hold key goes down at once; the tap key is added on a lone, quick release. No buffering, no added latency |
| Platforms | Linux. evdev + uinput primary; X11 (XRecord + XTest) fallback |
| X11 scope | Backend auto-selected (evdev when device access exists, X11 otherwise; flag overrides). Dual-role only on X11; remaps rejected with a `setxkbmap` hint |
| Privileges | evdev backend runs as a dedicated `escapepod` system account; udev rules grant device access to that account alone. The status socket uses a separate `escapepod-ctl` group, so reading status never grants device access. X11 backend runs as the session user and needs no device access |
| `monitor` | Shows only managed keys and Escape Pod's decisions; everything else is a redacted `other`. `--all-keys` needs a real terminal and is refused under an agent environment |
| Screen readers | A key used as the Orca modifier is left unmapped, with a warning and a hint (§10 reserved chords) |
| Category | B for the whole project: Texinfo SRS, CI traceability, budgets, ICD, baselines |
| Budgets | Measured on a low-end baseline: 2-core x86-64, 4 GiB RAM |
| Requirement IDs | Prefix `ESC` (`ESC-NEED-nnn`, `ESC-SRS-nnn`); one obligation per requirement |
| `--yes` | Required for `daemon run` even under an agent, tailoring agentic CLI §4 (see `COMPLIANCE.md`) |
| Config | TOML file (per-device rules, tap timeout, reload on `SIGHUP`) plus xcape-compatible `--map 'Control_L=Escape'` |
| Agentic CLI | `spacecraft-agentic-cli` applies in full. No MCP surface (7 commands, below the 10-command threshold) |


## Architecture

```text
 physical keyboard(s) ──grab──┐                       ┌──▶ virtual keyboard (uinput) ──▶ X11 / Wayland / console
 pointer device(s) ──read─────┼──▶ escapepod-evdev ──▶ escapepod-core (step) ──┘
 udev hotplug, signals ───────┘         │
                                        └──▶ status socket (/run/escapepod/escapepod.sock, redacted)

 X11 fallback:  XRecord (observe) ──▶ escapepod-x11 ──▶ escapepod-core ──▶ XTest (inject tap only)
```

| Crate | Role | Rules |
|-------|------|-------|
| `escapepod-core` | Pure state machine: `step(event, timestamp) → output events`. No I/O, no clock, no threads | `#![forbid(unsafe_code)]`; carries the bulk of the test, property, and mutation evidence |
| `escapepod-evdev` | Device discovery, exclusive grab, uinput device, udev hotplug, pointer read-without-grab | The only crate touching `/dev/input` and `/dev/uinput` |
| `escapepod-x11` | XRecord observation and XTest injection through `x11rb` (pure Rust, no libX11) | Dual-role only; never remaps |
| `escapepod` | CLI (clap), config (serde + toml), event loop, signals, status socket, diagnostics | Built to `spacecraft-cli-standard` + `spacecraft-agentic-cli` |

**Concurrency (§3.2) — serial by design, recorded trade-off.** One thread runs one `poll(2)` loop
over every device descriptor, the udev monitor, a signal descriptor, and the status socket. The
workload is a handful of events per keystroke, each costing microseconds, and the modifier state is
global across devices; threads would add synchronisation and the risk of reordering events, and buy
nothing. Pass-through tapping needs no timers, so the daemon sleeps until input arrives. The
decision is benchmarked against the latency budget at G2 and revisited only if the budget fails.

**Safety model (§3.1).** The virtual device is created before any grab, so a failure never leaves
a grabbed keyboard with no output. Held output keys are released on signal, panic, device removal,
and emergency exit. Escape Pod never grabs its own virtual device. A configurable emergency chord
(default Backspace + Escape + Enter) releases every grab and exits with a code the service unit
will not restart on. If the process is killed outright, the kernel drops the grab when the
descriptor closes.

**Privacy model (§3.3, §9).** Only the `escapepod` account can read input devices. The daemon writes
key events only to the virtual device and the status socket. The socket never carries the key code
of an unmanaged key, and is reachable only by members of a separate `escapepod-ctl` group: being
allowed to read status never grants the device access held by group `escapepod`.
`monitor --all-keys` does not go through the daemon at all: it reads devices itself, so it can
never show more than the caller could already read.

**Dependencies (to be qualified at G2, §24).** `evdev` 0.13.2, `udev` 0.9.3, `x11rb` 0.14.0,
`clap` 4.6.7, `serde` 1.0.229, `toml` 1.1.7, `signal-hook` 0.4.5 — versions as of 2026-10-08.


## Interfaces

Seven interface classes are drafted into the ICD at G2 (§23), recorded in the manual's
`Interfaces` node (ESC-SRS-089):

| Interface | Shape |
|-----------|-------|
| CLI | `escapepod daemon run`, `daemon status`, `event monitor`, `device list`, `config check`, `schema`, `describe` — human alias `escapepod run` |
| Config file | TOML; `--config`, else `$XDG_CONFIG_HOME/escapepod/config.toml` (session) or `/etc/escapepod/config.toml` (service) |
| xcape flag grammar | `--map 'KEY=TAP[;KEY=TAP…]'`, `--tap-timeout <ms>` |
| JSON output | §6 envelope on every data command; schema from `escapepod schema` |
| Status socket | Unix socket, mode `0660`, group `escapepod-ctl`; JSON Lines, redacted |
| Exit codes | 0–5 canonical; 6 = emergency release (service unit does not restart on it) |
| Packaging surface | systemd unit, udev rules, NixOS module `services.escapepod`, Guix package, PKGBUILD |


## Out of scope

- Layers, macros, chords, home-row modifiers, per-application rules (decided: scope is xcape+).
- Wait-and-decide (buffered) tapping (decided: pass-through only).
- XKB remaps on X11, FreeBSD, macOS, Windows.
- An MCP server.


## Open items for G2

- Name the exact reference machine (low-end baseline: 2-core x86-64, 4 GiB RAM, decided
  2026-10-08) and its workload for the §22 budgets (ESC-SRS-079, ESC-SRS-080).
- Orca settings discovery path and format across Orca versions (ESC-SRS-038).
- Register `EscapePod.SpacecraftSoftware.org` in Standard §15.1 with the next Standard release;
  until then `PROJECTS.md` shows no subdomain (decided 2026-10-08).

Resolved 2026-10-08: compound requirements split into single obligations (ESC-SRS-090 to 102);
the status socket uses its own `escapepod-ctl` group; the explicit-`--yes` rule (ESC-SRS-037) is
recorded in [`COMPLIANCE.md`](COMPLIANCE.md); `PROJECTS.md` row added.


## M0 — Foundation and compliance

- [ ] ESC-SRS-001 `reuse lint` passes
- [ ] ESC-SRS-002 Posture files at repository root
- [ ] ESC-SRS-003 Agent context files at repository root
- [ ] ESC-SRS-004 Category B declared in all three places
- [ ] ESC-SRS-005 `SECURITY.md` present
- [ ] ESC-SRS-006 `COMPLIANCE.md` tailoring register present
- [ ] ESC-SRS-007 CI gates fmt, clippy, tests, audit, deny
- [ ] ESC-SRS-008 LF / UTF-8 text files enforced in CI


## M1 — Tap/hold engine

- [ ] ESC-SRS-009 Hold key pressed at once
- [ ] ESC-SRS-010 Tap key on lone quick release
- [ ] ESC-SRS-011 No tap after another key press
- [ ] ESC-SRS-012 No tap after a pointer button press
- [ ] ESC-SRS-013 No tap after the tap timeout
- [ ] ESC-SRS-014 Own autorepeat does not cancel the tap
- [ ] ESC-SRS-015 Overlapping dual-role keys cancel each other's tap
- [ ] ESC-SRS-016 One-to-one remap
- [ ] ESC-SRS-017 Unmapped keys pass through in order
- [ ] ESC-SRS-018 Every emitted press gets a release
- [ ] ESC-SRS-019 Default tap timeout 200 ms
- [ ] ESC-SRS-020 Protected keys left unmapped
- [ ] ESC-SRS-021 Core crate forbids `unsafe`


## M2 — CLI, config and agent surface

- [ ] ESC-SRS-022 Command tree
- [ ] ESC-SRS-023 Global flags
- [ ] ESC-SRS-024 JSON envelope on data commands
- [ ] ESC-SRS-025 Structured errors with runnable hints
- [ ] ESC-SRS-026 Agent environment selects JSON
- [ ] ESC-SRS-027 Exit code map
- [ ] ESC-SRS-028 `--version` attribution
- [ ] ESC-SRS-029 Config file location and `--config`
- [ ] ESC-SRS-030 Unknown config keys exit 2
- [ ] ESC-SRS-031 Tap timeout range enforced
- [ ] ESC-SRS-032 xcape `--map` grammar
- [ ] ESC-SRS-033 `--tap-timeout` flag
- [ ] ESC-SRS-034 Per-device match rules
- [ ] ESC-SRS-035 `config check` grabs nothing
- [ ] ESC-SRS-036 `device list` needs no device access
- [ ] ESC-SRS-037 `daemon run` needs `--yes` off a terminal
- [ ] ESC-SRS-038 Orca modifier key protected
- [ ] ESC-SRS-039 `daemon run --dry-run` reports the plan
- [ ] ESC-SRS-040 Accessible mode output is linear
- [ ] ESC-SRS-041 `schema` is valid JSON Schema 2020-12
- [ ] ESC-SRS-042 Provider-wrapped schema formats (optional)
- [ ] ESC-SRS-090 No colour under agents
- [ ] ESC-SRS-091 No prompts under agents
- [ ] ESC-SRS-092 Unknown key line and column reported
- [ ] ESC-SRS-093 Orca clash warning
- [ ] ESC-SRS-094 Dry run grabs nothing
- [ ] ESC-SRS-095 Exit code 6 reserved for emergency


## M3 — evdev backend and safety

- [ ] ESC-SRS-043 Virtual device before any grab
- [ ] ESC-SRS-044 Matched keyboards grabbed exclusively
- [ ] ESC-SRS-045 Own virtual devices never grabbed
- [ ] ESC-SRS-046 Held keys released on SIGTERM / SIGINT
- [ ] ESC-SRS-047 Held keys released on panic
- [ ] ESC-SRS-048 Emergency chord releases grabs
- [ ] ESC-SRS-049 Emergency chord is configurable
- [ ] ESC-SRS-050 Device read error releases that device
- [ ] ESC-SRS-051 Hotplugged keyboard grabbed within 1 s
- [ ] ESC-SRS-052 Removed device's held keys released
- [ ] ESC-SRS-053 Pointers read without grab
- [ ] ESC-SRS-054 `SIGHUP` applies a valid config
- [ ] ESC-SRS-055 Invalid reload keeps previous config
- [ ] ESC-SRS-056 Reload keeps grabs on matched devices
- [ ] ESC-SRS-057 No wake-ups while idle
- [ ] ESC-SRS-058 Key events written only to the virtual device
- [ ] ESC-SRS-059 Status socket mode and group
- [ ] ESC-SRS-060 Socket never carries unmanaged key codes
- [ ] ESC-SRS-061 `event monitor` default redaction
- [ ] ESC-SRS-062 `--all-keys` refused off a terminal or under agents
- [ ] ESC-SRS-096 Emergency release exits with code 6
- [ ] ESC-SRS-097 Device fault warning
- [ ] ESC-SRS-098 Other devices keep working after a fault
- [ ] ESC-SRS-099 Failed reload warning


## M4 — X11 fallback backend

- [ ] ESC-SRS-063 Backend auto-selection
- [ ] ESC-SRS-064 `--backend` override
- [ ] ESC-SRS-065 X11 tap injection
- [ ] ESC-SRS-066 X11 rejects remaps
- [ ] ESC-SRS-067 X11 needs no device access
- [ ] ESC-SRS-068 X11 connection loss exits 1
- [ ] ESC-SRS-088 X11 injects tap keys only
- [ ] ESC-SRS-100 Unavailable forced backend exits 4
- [ ] ESC-SRS-101 X11 remap error hints `setxkbmap`
- [ ] ESC-SRS-102 X11 loss names the display


## M5 — Privileges, packaging and manual

- [ ] ESC-SRS-069 Service runs as the `escapepod` account
- [ ] ESC-SRS-070 udev rules scoped to the `escapepod` group
- [ ] ESC-SRS-071 No login user added to `input`
- [ ] ESC-SRS-072 Service unit sandboxed
- [ ] ESC-SRS-073 Service not restarted after emergency exit
- [ ] ESC-SRS-074 NixOS module `services.escapepod`
- [ ] ESC-SRS-075 `packaging/guix.scm` builds
- [ ] ESC-SRS-076 `packaging/default.nix` builds
- [ ] ESC-SRS-077 `packaging/PKGBUILD` builds
- [ ] ESC-SRS-078 Texinfo manual builds Info, HTML, PDF
- [ ] ESC-SRS-089 Interfaces node (ICD) in the manual


## M6 — Verification and release gate

- [ ] ESC-SRS-079 Added latency within budget
- [ ] ESC-SRS-080 Resident memory within budget
- [ ] ESC-SRS-081 Config parser survives fuzzing
- [ ] ESC-SRS-082 `--map` parser survives fuzzing
- [ ] ESC-SRS-083 Line coverage at least 80%
- [ ] ESC-SRS-084 Surviving mutants triaged
- [ ] ESC-SRS-085 Traceability matrix gates CI
- [ ] ESC-SRS-086 SBOM shipped per release
- [ ] ESC-SRS-087 Clean install from all three packages

---

*— Built by [Spacecraft Software](https://SpacecraftSoftware.org/) —*
