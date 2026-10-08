<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->
<!-- GFM Document
     title:      Escape Pod — Plan
     author:     Mohamed Hammad & Spacecraft Software
     date:       2026-10-08
     version:    0.1.0
     license:    CC-BY-SA-4.0
     project:    Escape Pod
     website:    https://EscapePod.SpacecraftSoftware.org/
-->

# Escape Pod — Plan

MVP: M0–M6 — Escape Pod v0.1

This is the §17.5 plan for [`PRD.md`](PRD.md). It cuts the PRD's milestones into work
packages; the requirements themselves live in `doc/requirements.toml` (rendered into
`doc/escapepod.texi`) and are never restated here. Each `P-` item names the requirements it
discharges. **Done rule:** a requirement-backed item is ticked when every requirement it
cites is `verified` in the traceability matrix (`nu tools/trace.nu`); an infrastructure or
gate item is ticked when it has landed on `main` and its gate record is written. Executable
tasks are in [`TODO.md`](TODO.md).

Template: `/spacecraft-software/operator` (tools, CI, posture set, packaging triad).
Skills per milestone: `microsoft-rust-guidelines` before any `.rs`; `spacecraft-nix-guidelines`
for `flake.nix`, `packaging/default.nix` and the NixOS module; `spacecraft-texinfo-document`
for the manual and ICD; `steelbore-color-palette` and `spacecraft-accessibility-support` at M2;
`spacecraft-guile-guidelines` for `packaging/guix.scm`; `spacecraft-nu-guidelines` for `tools/`.

## Gates

| Gate | Closes with | Tag |
|---|---|---|
| G1 — Requirements | P-001 | `g1-requirements-0.1` |
| G2 — Design | P-008 | `g2-design-0.1` |
| G3 — Release | P-042 | `v0.1.0` |

## M0 — Foundation and compliance

- [ ] P-001 G1: §20.4 characteristics gate over ESC-SRS-001–102; migrate the `Requirements`
      chapter into `doc/requirements.toml` (id, title, text, rationale, source, method,
      priority, status, milestone); `tools/srs.nu` renders `doc/requirements.texi`,
      `@include`d by `doc/escapepod.texi`; statuses `draft → baselined`; signed tag
- [ ] P-002 Posture set: `README.md` (posture, Category B, §19.6 claim, §13 N/A),
      `NOTICE.md`, `CONTRIBUTING.md`, `LICENSE` + `LICENSES/GPL-3.0-or-later.txt` symlink,
      `LICENSES/CC-BY-SA-4.0.txt`, `REUSE.toml`, `SECURITY.md`, `CREDITS.md` (xcape, ISO
      29148, ECSS); `reuse lint` clean — ESC-SRS-001, 002, 004, 005, 006
- [ ] P-003 Agent context: `AGENTS.md` (authoritative), `CLAUDE.md` (`@AGENTS.md` + Claude-only),
      `SKILL.md` (CLI capability surface) — ESC-SRS-003, 004
- [ ] P-004 Text format: `.gitattributes` (`* text=auto eol=lf`), `.editorconfig`, index CRLF
      gate — ESC-SRS-008
- [ ] P-005 Workspace skeleton: `Cargo.toml` (crates `escapepod-core`, `escapepod-evdev`,
      `escapepod-x11`, `escapepod`; edition 2024; release profile with every flag noted),
      `clippy.toml`, `rustfmt.toml`, `deny.toml`, `.cargo/audit.toml`, `flake.nix` devShell
      (rustc, cargo-audit/deny/nextest/mutants/llvm-cov/cyclonedx/bloat, hyperfine, reuse,
      texinfo, nushell, Xvfb), `flake.lock` as the toolchain pin — ESC-SRS-007 (tooling), 021
- [ ] P-006 CI: `.github/workflows/ci.yml` (fmt, clippy `-D warnings`, test, audit, deny,
      REUSE, CRLF, posture set, manual build, SRS freshness, trace) and `tools/ci.nu` running
      the same gates locally — ESC-SRS-007, 008
- [ ] P-007 Traceability: `tools/trace.nu` (markers `Verifies:`/`Implements:` → matrix; fails
      on unknown id or `implemented`/`verified` without marker; `--release` orphans; emits the
      §17.1 block) and `tools/progress.nu` (TODO/PLAN rows via `doc/progress-map.toml`) —
      ESC-SRS-085 (verified at M6)
- [ ] P-008 G2: `Design` node (architecture, serial `poll(2)` trade-off, safety and privacy
      models, device-I/O seam), `DEPENDENCIES.md` trust path (`evdev`, `udev`, `x11rb`,
      `toml`, `serde`, `clap`, `signal-hook`, poll primitive), `Interfaces` node skeleton
      (seven classes), reference machine and workload named, ESC-SRS-079/080 declared with
      them, `COMPLIANCE.md` rows (`rust-toolchain.toml`, §13, §3.3 PQC) and gate record;
      signed tag — ESC-SRS-089 (draft)

## M1 — Tap/hold engine

- [ ] P-009 Core types: `KeyCode`, `Event {Press, Release, Repeat}`, monotonic `Instant`
      (µs), `Rules` (dual-role, remap, protected set, tap timeout, emergency chord);
      `#![forbid(unsafe_code)]`; `thiserror` canonical error structs — ESC-SRS-021
- [ ] P-010 `Engine::step(event, now) -> Vec<Output>` dual-role state machine: hold key at
      once, tap on lone quick release, used on key/pointer press, timeout, own autorepeat,
      overlapping dual-role keys — ESC-SRS-009, 010, 011, 012, 013, 014, 015, 019
- [ ] P-011 Remap, pass-through in order, protected keys — ESC-SRS-016, 017, 020
- [ ] P-012 Property tests (`proptest`): balanced output, input order, plus a differential
      oracle (tiny reference model) for §21.4 independence — ESC-SRS-018
- [ ] P-013 Emergency chord detection in core (`Output::Emergency` once every chord key is
      down) and chord configurability — ESC-SRS-049 (core half of 048)
- [ ] P-014 `criterion` bench of `step()` (ns per event) as the engine's share of the
      latency budget — supports ESC-SRS-079

## M2 — CLI, config and agent surface

- [ ] P-015 CLI skeleton (pattern: Operator `mode.rs`, `agent_env.rs`, `envelope.rs`,
      `error.rs`): clap tree `daemon run|status`, `event monitor`, `device list`,
      `config check`, `schema`, `describe`, alias `run`; every §3 global flag; §5 output
      cascade (presence-based agent detection); `Response<T>` envelope; `AppError` with
      runnable `hint`/`human:` prefix; exit-code enum with 6 reserved; `--version`
      attribution — ESC-SRS-022, 023, 024, 025, 026, 027, 028, 090, 091, 095
- [ ] P-016 Config: TOML model (`deny_unknown_fields`, span → line/column), search order,
      tap-timeout range 50–2000 ms, per-device rules (name | vendor:product), `--map` xcape
      grammar, `--tap-timeout`; `config check` opens nothing — ESC-SRS-029, 030, 031, 032,
      033, 034, 035, 092
- [ ] P-017 `device list` over udev metadata only (no event-device read) with match status —
      ESC-SRS-036
- [ ] P-018 Run guards: `--yes` off a terminal or under an agent; `--dry-run` plan that grabs
      nothing; Orca modifier discovery (gsettings/dconf `org.gnome.orca` keyboard layout →
      modifier keys) → protected + warning — ESC-SRS-037, 038, 039, 093, 094
- [ ] P-019 Human-mode rendering: `steelbore` theme binding §11.1 role tokens from
      `steelbore.toml` (no bare hex), `NO_COLOR` → mono, `SPACECRAFT_A11Y`/`--accessible`
      linear `event monitor` — ESC-SRS-040
- [ ] P-020 `schema` (JSON Schema 2020-12 via `schemars`, validated in test) and `describe`
      manifest — ESC-SRS-041
- [ ] P-044 `schema --format` provider wrappers — ESC-SRS-042 (optional)
- [ ] P-021 CLI compliance suite (`assert_cmd`, `predicates`, `insta`): the ten
      `testing-compliance.md` categories, run under the host's real `AI_AGENT` value —
      supports every M2 requirement

## M3 — evdev backend and safety

- [ ] P-022 Device-I/O seam in `escapepod-evdev`: traits `InputSource`, `OutputSink`,
      `HotplugSource`, `Clock`; fake implementations for tests; real ones over `evdev` +
      `udev` + uinput — design item (G2 `Design` node)
- [ ] P-023 Output-before-grab: create the uinput device first (exit 4 on failure), then
      exclusive grab of matched keyboards; never open an Escape Pod virtual device —
      ESC-SRS-043, 044, 045
- [ ] P-024 Single-thread event loop: one `poll(2)` over device fds, udev monitor, signal fd,
      status socket; blocking with no timeout while idle (analysis record `doc/idle.md`);
      pointers read without grab — ESC-SRS-053, 057
- [ ] P-025 Release discipline: `HeldKeys` ledger; release on SIGTERM/SIGINT within 100 ms,
      on panic (hook + drop guard), on device removal; emergency chord releases every grab
      and exits 6 — ESC-SRS-046, 047, 048, 052, 096
- [ ] P-026 Fault isolation and hotplug: read error → release that device, warn, keep
      serving others; udev add → grab within 1 s — ESC-SRS-050, 051, 097, 098
- [ ] P-027 Reload: SIGHUP → validate → apply, keep grabs on still-matched devices, failed
      reload keeps previous config and warns — ESC-SRS-054, 055, 056, 099
- [ ] P-028 Status socket `/run/escapepod/escapepod.sock` (0660, group `escapepod-ctl`),
      JSON Lines, redacted; `daemon status` client; key-data destination record
      `doc/privacy-review.md` — ESC-SRS-058, 059, 060
- [ ] P-029 `event monitor`: default redaction to `other`, `--all-keys` refused off a
      terminal or under an agent, reads devices directly — ESC-SRS-061, 062
- [ ] P-030 Hardware tests (feature `hardware-tests`): uinput-loopback keyboard drives the
      real backend end to end; CI job with `/dev/uinput` access, also runnable locally —
      verification for ESC-SRS-043–056

## M4 — X11 fallback backend

- [ ] P-031 Backend selection: `auto` probes device read + `/dev/uinput` write → evdev, else
      `DISPLAY` → x11, else exit 4; `--backend` override; forced-but-unavailable exits 4 —
      ESC-SRS-063, 064, 100
- [ ] P-032 `escapepod-x11`: XRecord observation, XTest injection of tap keys only, remap
      rejected (exit 2, hint `setxkbmap -option`), connection loss → error naming the
      display, exit 1; no device access — ESC-SRS-065, 066, 067, 068, 088, 101, 102
- [ ] P-033 Xvfb test harness (`xdotool`/XTest driver) in `tools/ci.nu` and CI —
      verification for ESC-SRS-065, 088

## M5 — Privileges, packaging and manual

- [ ] P-034 System integration: `packaging/systemd/escapepod.service` (`User=escapepod`,
      `RestrictAddressFamilies=AF_UNIX AF_NETLINK`, `RestartPreventExitStatus=6`),
      `sysusers.d`, `tmpfiles.d`, `packaging/udev/70-escapepod.rules` (group `escapepod`
      only), no `input` group membership anywhere — ESC-SRS-069, 070, 071, 072, 073
- [ ] P-035 NixOS module `services.escapepod` + `nixosTests.escapepod` VM test —
      ESC-SRS-074
- [ ] P-036 Packaging triad `packaging/guix.scm`, `packaging/default.nix`,
      `packaging/PKGBUILD`, each with version, SHA-256 and `install-info` hook —
      ESC-SRS-075, 076, 077
- [ ] P-037 Manual: user chapters (Invoking, Configuration, Service, Troubleshooting,
      xcape migration), `Interfaces` node completed for all seven classes with committed
      schemas under `schemas/`; builds Info, HTML, PDF with zero warnings — ESC-SRS-078, 089

## M6 — Verification and release gate

- [ ] P-038 Budget gates: `tools/latency-gate.nu` (uinput loopback, p99 ≤ 1 ms, `taskset -c
      0,1`), `tools/rss-gate.nu` (≤ 8 MiB), binary-size row declared; margins and bands in
      `doc/budgets.md` — ESC-SRS-079, 080
- [ ] P-039 Fuzzing: `cargo-fuzz` targets for the config parser and the `--map` grammar,
      10⁷ executions recorded, corpora committed, `fuzz.yml` — ESC-SRS-081, 082
- [ ] P-040 Coverage ≥ 80 % (`cargo llvm-cov`) and `cargo mutants` triage
      (`doc/mutants.md`) — ESC-SRS-083, 084
- [ ] P-041 Trace gate green on GitHub and SBOM (`cargo cyclonedx`) with checksum in
      `release.yml` — ESC-SRS-085, 086
- [ ] P-042 G3: validation of ESC-NEED-001–008 from installed packages in clean environments
      (NixOS VM, Guix container, Arch container), `CHANGELOG.md`, release manifest
      `doc/release-0.1.md`, open anomalies as GitHub issues, `COMPLIANCE.md` re-read, `v0.1.0`
      signed tag — ESC-SRS-087
- [ ] P-043 Registry: `PROJECTS.md` status `Planning → Active` at M1 and the release row at
      M6, with the §18.4 statement (Projects repo PR)

---

*— Built by [Spacecraft Software](https://SpacecraftSoftware.org/) —*
