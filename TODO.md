<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->
<!-- GFM Document
     title:      Escape Pod — TODO
     author:     Mohamed Hammad & Spacecraft Software
     date:       2026-10-08
     version:    0.1.0
     license:    CC-BY-SA-4.0
     project:    Escape Pod
     website:    https://EscapePod.SpacecraftSoftware.org/
-->

# Escape Pod — TODO

MVP: M0–M6 — Escape Pod v0.1

Executable tasks for [`PLAN.md`](PLAN.md); each cites its work package. One task is one
branch and one PR unless noted. **Done rule:** a task is ticked when its PR is squash-merged
to `main` with the local gate (`nu tools/ci.nu`) green; a task that lands a test ticks only
when that test passes. Identifiers are permanent.

## M0 — Foundation and compliance

- [ ] T-001 P-001 Run the §20.4 gate over ESC-SRS-001–102; record findings in
      `COMPLIANCE.md` "Gate records / G1"
- [ ] T-002 P-001 Write `doc/requirements.toml` from the `Requirements` chapter (102 rows,
      `milestone` per PRD)
- [ ] T-003 P-001 Port Operator `tools/srs.nu`; render `doc/requirements.texi`; replace the
      chapter in `doc/escapepod.texi` with `@include requirements.texi`; manual builds clean
- [ ] T-004 P-001 Flip every status to `baselined`; push signed tag `g1-requirements-0.1`
- [x] T-005 P-002 Copy `LICENSE` (GPL-3.0-or-later) from `/spacecraft-software/license/`;
      `LICENSES/GPL-3.0-or-later.txt -> ../LICENSE`; add `LICENSES/CC-BY-SA-4.0.txt`
- [x] T-006 P-002 `README.md`: description, Project Posture (Personal/Hobby, Category B,
      §19.6 claim "tailored, see COMPLIANCE.md"), §13 N/A, support window, build/run
- [x] T-007 P-002 `NOTICE.md`, `CONTRIBUTING.md` from `/spacecraft-software/license/`,
      specialised
- [x] T-008 P-002 `SECURITY.md`: private channel, acknowledgement target (days), scope,
      supported versions, disclosure terms, credit
- [x] T-009 P-002 `CREDITS.md`: xcape (prior art), ISO/IEC/IEEE 29148, ECSS-E-ST-40C
- [x] T-010 P-002 `REUSE.toml` for `chat/`-free tree; `reuse lint` exits 0
- [x] T-011 P-003 `AGENTS.md` per `agents-md-authoring.md` (identity, build/test/lint,
      invariants, forbidden patterns, environment, where-to-look); Category B declared
- [x] T-012 P-003 `CLAUDE.md` = `@AGENTS.md` + skills to load + `.claude/` notes
- [x] T-013 P-003 `SKILL.md`: command tree, formats, global flags, exit codes, examples;
      description ≤ 1000 characters
- [x] T-014 P-004 `.gitattributes`, `.editorconfig`; `git ls-files --eol` CRLF check in
      `tools/ci.nu`
- [x] T-015 P-005 `Cargo.toml` workspace with four crates and `workspace.package`
      (edition 2024, license, repository, homepage, authors, `publish = false`)
- [x] T-016 P-005 Release profile: `lto = "fat"`, `codegen-units = 1`, `panic = "unwind"`
      (release-on-panic needs unwinding), `strip = true`; each flag commented, `-march`
      left off and why
- [x] T-017 P-005 `clippy.toml`, `rustfmt.toml`, `deny.toml` (licences, advisories, bans,
      sources), `.cargo/audit.toml`
- [x] T-018 P-005 `flake.nix` devShell + `flake.lock`; `COMPLIANCE.md` row for the
      `rust-toolchain.toml` tailoring; `rust-version` = MSRV in `Cargo.toml`
- [x] T-019 P-005 Crate stubs with SPDX headers, `escapepod-core` `#![forbid(unsafe_code)]`,
      `cargo build` and `cargo clippy -- -D warnings` clean
- [ ] T-020 P-006 `.github/workflows/ci.yml` (jobs: rust, posture, docs, trace)
- [ ] T-021 P-006 `tools/ci.nu` mirroring every CI gate; documented in `AGENTS.md`
- [ ] T-022 P-007 `tools/trace.nu` reading `doc/requirements.toml`; unit-tested on a
      fixture with an unknown id and a markerless `verified` row
- [ ] T-023 P-007 `tools/progress.nu` + `doc/progress-map.toml` mapping `P-`/`T-` items to
      requirements; prints the full §17.1 block
- [ ] T-024 P-008 `Design` node in the manual: architecture, serial trade-off (§3.2),
      safety model, privacy model, device-I/O seam, poll primitive choice
- [ ] T-025 P-008 `DEPENDENCIES.md` trust-path rows (eight §24.1 fields each)
- [ ] T-026 P-008 `Interfaces` node skeleton: seven classes, identity/version/stability
      per interface, `schemas/` directory
- [ ] T-027 P-008 Terminology: reference machine (ThinkPad T490s, i7-8665U, 32 GiB, NVMe,
      NixOS 26.05) and workload (`taskset -c 0,1`, committed input script);
      ESC-SRS-079/080 text updated via TOML
- [ ] T-028 P-008 `COMPLIANCE.md`: §13 N/A, §3.3 PQC N/A, gate record G2; signed tag
      `g2-design-0.1`

## M1 — Tap/hold engine

- [ ] T-029 P-009 `KeyCode` (Linux `KEY_*` newtype), `Event`, `Instant` (µs), `Output`
- [ ] T-030 P-009 `Rules`: dual-role list, remap map, protected set, tap timeout
      (default 200 ms), emergency chord; `RulesError` via `thiserror`
- [ ] T-031 P-010 `Engine::step` hold-key-at-once and tap on lone quick release
      (`Verifies: ESC-SRS-009, 010`)
- [ ] T-032 P-010 Used-flag on key press and pointer press; timeout; own autorepeat
      (`ESC-SRS-011, 012, 013, 014`)
- [ ] T-033 P-010 Overlapping dual-role keys; default timeout (`ESC-SRS-015, 019`)
- [ ] T-034 P-011 Remap all three event kinds; pass-through order; protected keys
      (`ESC-SRS-016, 017, 020`)
- [ ] T-035 P-012 `proptest` strategies for key/pointer sequences; balanced-output and
      order properties (`ESC-SRS-018`)
- [ ] T-036 P-012 Reference model `tests/oracle.rs` and differential test
- [ ] T-037 P-013 Chord tracker → `Output::Emergency`; configurable set (`ESC-SRS-049`)
- [ ] T-038 P-014 `benches/step.rs` (criterion); number recorded in `doc/budgets.md`
- [ ] T-039 P-009 Core doc-tests on every public item; `cargo doc` with intra-doc links as
      errors

## M2 — CLI, config and agent surface

- [ ] T-040 P-015 clap tree and `run` alias; `--help` with two examples per command and the
      project URL footer (`ESC-SRS-022`)
- [ ] T-041 P-015 Global flags §3 incl. `--quiet`/`--verbose` exclusivity (`ESC-SRS-023`)
- [ ] T-042 P-015 `agent_env.rs` (presence-based), `mode.rs` cascade, colour precedence
      (`ESC-SRS-026, 090, 091`)
- [ ] T-043 P-015 `envelope.rs` `Response<T>`, compact when not a TTY, `None` omitted
      (`ESC-SRS-024`)
- [ ] T-044 P-015 `error.rs` `AppError` {code, exit_code, message, hint, timestamp, command,
      docs_url}; hint catalogue from `assets/error-hint-catalog.json`; `human:` prefix
      (`ESC-SRS-025`)
- [ ] T-045 P-015 `ExitCode` enum; 6 only from emergency path (`ESC-SRS-027, 095`)
- [ ] T-046 P-015 `--version` text and JSON attribution (`ESC-SRS-028`)
- [ ] T-047 P-016 Config model with `deny_unknown_fields`; `toml` span → line/column in the
      error (`ESC-SRS-030, 092`)
- [ ] T-048 P-016 Search order `--config` → XDG → `/etc` (`ESC-SRS-029`)
- [ ] T-049 P-016 Tap-timeout range and `--tap-timeout` override (`ESC-SRS-031, 033`)
- [ ] T-050 P-016 `--map` parser (`KEY=TAP[;…]`, xcape key names → `KeyCode`)
      (`ESC-SRS-032`)
- [ ] T-051 P-016 Per-device rules by name or `vendor:product` (`ESC-SRS-034`)
- [ ] T-052 P-016 `config check` with no device I/O (`ESC-SRS-035`)
- [ ] T-053 P-017 `device list` from udev properties; `--fields`, `jsonl` (`ESC-SRS-036`)
- [ ] T-054 P-018 `--yes` guard off TTY / under agent (`ESC-SRS-037`)
- [ ] T-055 P-018 `--dry-run` plan output, no grab (`ESC-SRS-039, 094`)
- [ ] T-056 P-018 Orca modifier discovery and protection with warning
      (`ESC-SRS-038, 093`); discovery path recorded in `Design`
- [ ] T-057 P-019 `theme.rs` binding §11.1 tokens read from `steelbore.toml`; `NO_COLOR`
      → mono; `SPACECRAFT_THEME` honoured
- [ ] T-058 P-019 Accessible mode toggle and linear `event monitor` (`ESC-SRS-040`)
- [ ] T-059 P-020 `schema` via `schemars`; test validates against the 2020-12 meta-schema
      (`ESC-SRS-041`)
- [ ] T-060 P-020 `describe` manifest: commands, flags, exit codes, examples
- [ ] T-108 P-044 `schema --format` provider wrappers (`ESC-SRS-042`) (optional)
- [ ] T-061 P-021 `tests/cli/` suite: schema, exit codes, TTY/non-TTY, agent env (real
      `AI_AGENT` value), idempotency, input validation (control chars, paths),
      cross-shell roundtrip, UTF-8, schema/describe smoke, context-file presence

## M3 — evdev backend and safety

- [ ] T-062 P-022 Traits `InputSource`, `OutputSink`, `HotplugSource`, `Clock`; fakes in
      `escapepod-evdev/src/fake.rs`
- [ ] T-063 P-022 Real `evdev`/uinput/`udev` implementations; choose poll primitive
      (`nix::poll` if `evdev` already links `nix`, else `rustix`)
- [ ] T-064 P-023 uinput device created first; exit 4 if it fails (`ESC-SRS-043`)
- [ ] T-065 P-023 Exclusive grab of matched keyboards; own-device exclusion by uinput
      vendor/product/name tag (`ESC-SRS-044, 045`)
- [ ] T-066 P-024 `poll(2)` loop over devices, udev monitor, signal fd, socket; no timeout
      when idle; `doc/idle.md` analysis (`ESC-SRS-057`)
- [ ] T-067 P-024 Pointer devices opened read-only without grab (`ESC-SRS-053`)
- [ ] T-068 P-025 `HeldKeys` ledger and `release_all()`; SIGTERM/SIGINT path ≤ 100 ms
      (`ESC-SRS-046`)
- [ ] T-069 P-025 Panic hook + drop guard releasing held keys (`ESC-SRS-047`)
- [ ] T-070 P-025 Emergency chord → release every grab ≤ 100 ms, exit 6
      (`ESC-SRS-048, 096`)
- [ ] T-071 P-025 Removal releases that device's held keys (`ESC-SRS-052`)
- [ ] T-072 P-026 Read error → release device, warn, continue (`ESC-SRS-050, 097, 098`)
- [ ] T-073 P-026 Hotplug grab within 1 s (`ESC-SRS-051`)
- [ ] T-074 P-027 SIGHUP reload: validate, apply, keep grabs, warn on failure
      (`ESC-SRS-054, 055, 056, 099`)
- [ ] T-075 P-028 Status socket creation (0660, `escapepod-ctl`), JSON Lines frames,
      redaction (`ESC-SRS-059, 060`)
- [ ] T-076 P-028 `daemon status` client; `doc/privacy-review.md` for `ESC-SRS-058`
- [ ] T-077 P-029 `event monitor` default redaction; `--all-keys` guard (`ESC-SRS-061, 062`)
- [ ] T-078 P-030 `hardware-tests` feature: uinput-loopback keyboard fixture
- [ ] T-079 P-030 CI job with `/dev/uinput` access running the hardware tests; same job in
      `tools/ci.nu --hardware`

## M4 — X11 fallback backend

- [ ] T-080 P-031 Backend probe and `auto` selection; `--backend`; exit 4 paths
      (`ESC-SRS-063, 064, 100`)
- [ ] T-081 P-032 `escapepod-x11`: XRecord context over key events via `x11rb`
- [ ] T-082 P-032 XTest tap injection only; remap rejection with hint
      (`ESC-SRS-065, 066, 088, 101`)
- [ ] T-083 P-032 Connection-loss handling: error naming display, exit 1
      (`ESC-SRS-068, 102`)
- [ ] T-084 P-032 Runs with no device access (`ESC-SRS-067`) — test under a user with
      no `escapepod` group
- [ ] T-085 P-033 Xvfb harness in `tools/ci.nu` and CI job

## M5 — Privileges, packaging and manual

- [ ] T-086 P-034 `escapepod.service`, `sysusers.d/escapepod.conf`,
      `tmpfiles.d/escapepod.conf` (`ESC-SRS-069, 072, 073`)
- [ ] T-087 P-034 `70-escapepod.rules` scoped to group `escapepod`; `escapepod-ctl` for the
      socket; packaging test asserting no `input` membership (`ESC-SRS-070, 071`)
- [ ] T-088 P-035 NixOS module `services.escapepod`
- [ ] T-089 P-035 `nixosTests.escapepod` VM test: service up, udev rule applied, tap works
      via uinput fixture (`ESC-SRS-074`)
- [ ] T-090 P-036 `packaging/default.nix` (`ESC-SRS-076`)
- [ ] T-091 P-036 `packaging/guix.scm` (`ESC-SRS-075`)
- [ ] T-092 P-036 `packaging/PKGBUILD` (`ESC-SRS-077`)
- [ ] T-093 P-037 Manual user chapters; xcape migration appendix
- [ ] T-094 P-037 `Interfaces` node complete; `schemas/*.json` committed and validated in
      tests (`ESC-SRS-089`)
- [ ] T-095 P-037 `make -C doc` builds Info/HTML/PDF with zero warnings; CI docs job
      (`ESC-SRS-078`)

## M6 — Verification and release gate

- [ ] T-096 P-038 `tools/latency-gate.nu` and the reference workload script
      (`ESC-SRS-079`)
- [ ] T-097 P-038 `tools/rss-gate.nu`, `tools/size-gate.nu`; `doc/budgets.md` with bands
      (`ESC-SRS-080`)
- [ ] T-098 P-039 `fuzz/` targets `config_parse`, `map_parse`; 10⁷ executions recorded
      (`ESC-SRS-081, 082`)
- [ ] T-099 P-040 `cargo llvm-cov` ≥ 80 % in CI (`ESC-SRS-083`)
- [ ] T-100 P-040 `cargo mutants` run and `doc/mutants.md` triage (`ESC-SRS-084`)
- [ ] T-101 P-041 First green GitHub run of `ci.yml` incl. trace gate (`ESC-SRS-085`)
- [ ] T-102 P-041 `release.yml`: SBOM (CycloneDX) + SHA-256, artifacts, checksums
      (`ESC-SRS-086`)
- [ ] T-103 P-042 Clean-install validation from NixOS VM, Guix container, Arch container;
      notes in the manifest (`ESC-SRS-087`)
- [ ] T-104 P-042 Validation of ESC-NEED-001–008 on the reference machine from installed
      packaging
- [ ] T-105 P-042 `CHANGELOG.md`, `doc/release-0.1.md` manifest, anomalies filed,
      `COMPLIANCE.md` re-read, G3 record
- [ ] T-106 P-042 Signed tag `v0.1.0`
- [ ] T-107 P-043 `PROJECTS.md` row updates (Projects repo PRs at M1 start and at release)

---

*— Built by [Spacecraft Software](https://SpacecraftSoftware.org/) —*
