<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# AGENTS.md — Escape Pod

This file is the authoritative agent context (Standard §5.7). New project
knowledge goes here, not in `CLAUDE.md`.

## Project identity

Escape Pod is a Linux daemon that gives one key two jobs: tapped alone it
sends one key (Escape), held with other keys it is a modifier (Control). It is
the Rust successor to xcape. It works on the kernel input layer (evdev grab,
uinput output), so it covers X11, Wayland and the console, and it falls back
to X11 (XRecord + XTest) when there is no `/dev/input` access. It is a
four-crate Cargo workspace plus a Texinfo manual that holds the SRS.

**Assurance: Category B** (Standard §19). The daemon grabs every keyboard, so
a fault can leave the user unable to type. Tailoring is in `COMPLIANCE.md`.

## Build, test, lint

Enter the pinned toolchain first: `nix develop`. The flake pins rustc through
`flake.lock`, and the MSRV is `rust-version` in `Cargo.toml`.

- Full local gate (mirrors CI; run before every PR): `nu tools/ci.nu`
- Build: `cargo build --locked`
- Test: `cargo test --workspace --locked`
- Lint: `cargo clippy --all-targets --all-features --locked -- -D warnings`
- Format check: `cargo fmt --all --check`
- Supply chain: `cargo audit` and `cargo deny check`
- Licensing: `reuse lint`

## Layout

| Path | Role |
|---|---|
| `crates/escapepod-core` | Pure engine: `step(event, now) -> outputs`. No I/O, no clock |
| `crates/escapepod-evdev` | evdev read + exclusive grab, uinput write, udev hotplug |
| `crates/escapepod-x11` | X11 fallback: XRecord observe, XTest inject (dual-role only) |
| `crates/escapepod` | Binary: CLI, config, the single-thread `poll(2)` loop |
| `doc/escapepod.texi` | Manual and SRS (`Needs`, `Requirements`, later `Design`, `Interfaces`) |
| `tools/` | Nushell gates; `ci.nu` is the local CI mirror |
| `PRD.md`, `PLAN.md`, `TODO.md` | §17.5 tracking documents; tick items in the PR that completes them |

## Architectural invariants

- `escapepod-core` is `#![forbid(unsafe_code)]`. It does no I/O and never reads a
  clock. Time comes in as a monotonic microsecond argument, so every decision
  can be replayed in tests.
- The hold key goes down at once. The tap key is added only on a lone, quick
  release. The engine never buffers or delays an event.
- Every key Escape Pod presses on the virtual device is recorded in a held-keys
  ledger. Every exit path releases it: SIGTERM/SIGINT, panic (hook and Drop
  guard), device removal, read error, and the emergency chord. A new exit path
  without a release is a defect.
- The uinput device is created **before** any grab. If it cannot be created,
  exit 4 and grab nothing. Never open a device Escape Pod created itself.
- The daemon is serial by design: one thread, one `poll(2)`, no timeout while
  idle. The trade-off (Standard §3.2) is recorded in the manual's `Design`
  node; do not add threads or an async runtime.
- Key data never leaves the process except through uinput. Logs, the status
  socket and `event monitor` (without `--all-keys`) show managed keys and
  decisions only; everything else is `other`.
- Exit code 6 is reserved for the emergency chord and is produced only there.
- `panic = "unwind"` in the release profile is deliberate: releasing keys on
  panic needs unwinding. Never switch it to abort.

## Forbidden patterns

- `unwrap()`, `expect()`, or `panic!` outside tests (denied workspace-wide).
- `unsafe` anywhere (forbidden workspace-wide; lowering it needs a
  `COMPLIANCE.md` row).
- Membership in the `input` group, in any unit, rule, module or doc. Device
  access goes to the `escapepod` account through the udev rule alone.
- Logging or printing a key code that is not managed.
- Treating `AI_AGENT`/`AGENT` as implicit `--yes` for `daemon run`: an agent
  must pass `--yes` explicitly (ESC-SRS-037; tailored in `COMPLIANCE.md`).
- Local time in any output. Timestamps are ISO 8601 UTC with `Z`.
- `rust-toolchain.toml` or rustup pins (NixOS; the flake is the pin).
- Pushing to `main` or pushing tags without the maintainer's word. Use branch →
  signed commits → PR → squash-merge.

## Requirements and traceability

Requirements are `ESC-SRS-nnn` in the manual's `Requirements` node and, from
G1, in `doc/requirements.toml`. Mark evidence in code with `Verifies:
ESC-SRS-nnn` on tests and `Implements: ESC-SRS-nnn` on the implementing item.
Never change a requirement's text from code work. Change the TOML in its own
PR.

## Environment expectations

- Linux only. The evdev backend needs `/dev/uinput` and readable
  `/dev/input/event*`. Hardware tests are behind the `hardware-tests` feature
  and are never part of the default `cargo test`.
- X11 tests run under `xvfb-run` from the devShell.
- Never `sudo`. Never use a system package manager. Missing tools come from
  `nix develop` or `nix run`.

## Where to look

- Requirements and design: `doc/escapepod.texi`
- Work packages and tasks: `PLAN.md`, `TODO.md`
- Tailoring and gate records: `COMPLIANCE.md`
- Local gate: `tools/ci.nu`
