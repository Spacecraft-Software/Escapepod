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

MVP: M0–M7 — Escape Pod v0.1

Executable tasks for [`PLAN.md`](PLAN.md); each cites its work package. One task is one
branch and one PR unless noted. **Done rule:** a task is ticked when its PR is squash-merged
to `main` with the local gate (`nu tools/ci.nu`) green; a task that lands a test ticks only
when that test passes. Identifiers are permanent.

## M0 — Foundation and compliance

- [x] T-001 P-001 Run the §20.4 gate over ESC-SRS-001–102; record findings in
      `COMPLIANCE.md` "Gate records / G1"
- [x] T-002 P-001 Write `doc/requirements.toml` from the `Requirements` chapter (102 rows,
      `milestone` per PRD)
- [x] T-003 P-001 Port Operator `tools/srs.nu`; render `doc/requirements.texi`; replace the
      chapter in `doc/escapepod.texi` with `@include requirements.texi`; manual builds clean
- [x] T-004 P-001 Flip every status to `baselined`; push signed tag `g1-requirements-0.1`
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
- [x] T-020 P-006 `.github/workflows/ci.yml` (jobs: rust, posture, docs, trace)
- [x] T-021 P-006 `tools/ci.nu` mirroring every CI gate; documented in `AGENTS.md`
- [x] T-022 P-007 `tools/trace.nu` reading `doc/requirements.toml`; unit-tested on a
      fixture with an unknown id and a markerless `verified` row
- [x] T-023 P-007 `tools/progress.nu` + `doc/progress-map.toml` mapping `P-`/`T-` items to
      requirements; prints the full §17.1 block
- [x] T-024 P-008 `Design` node in the manual: architecture, serial trade-off (§3.2),
      safety model, privacy model, device-I/O seam, poll primitive choice
- [x] T-025 P-008 `DEPENDENCIES.md` trust-path rows (eight §24.1 fields each)
- [x] T-026 P-008 `Interfaces` node skeleton: seven classes, identity/version/stability
      per interface, `schemas/` directory
- [x] T-027 P-008 Terminology: reference machine (ThinkPad T490s, i7-8665U, 32 GiB, NVMe,
      NixOS 26.05) and workload (`taskset -c 0,1`, committed input script);
      ESC-SRS-079/080 text updated via TOML
- [x] T-028 P-008 `COMPLIANCE.md`: §13 N/A, §3.3 PQC N/A, gate record G2; signed tag
      `g2-design-0.1`

## M1 — Tap/hold engine

- [x] T-029 P-009 `KeyCode` (Linux `KEY_*` newtype), `Event`, `Instant` (µs), `Output`
- [x] T-030 P-009 `Rules`: dual-role list, remap map, protected set, tap timeout
      (default 200 ms), emergency chord; `RulesError` via `thiserror`
- [x] T-031 P-010 `Engine::step` hold-key-at-once and tap on lone quick release
      (`Verifies: ESC-SRS-009, 010`)
- [x] T-032 P-010 Used-flag on key press and pointer press; timeout; own autorepeat
      (`ESC-SRS-011, 012, 013, 014`)
- [x] T-033 P-010 Overlapping dual-role keys; default timeout (`ESC-SRS-015, 019`)
- [x] T-034 P-011 Remap all three event kinds; pass-through order; protected keys
      (`ESC-SRS-016, 017, 020`)
- [x] T-035 P-012 `proptest` strategies for key/pointer sequences; balanced-output and
      order properties (`ESC-SRS-018`)
- [x] T-036 P-012 Reference model `tests/oracle.rs` and differential test
- [x] T-037 P-013 Chord tracker → `Output::Emergency`; configurable set (`ESC-SRS-049`)
- [x] T-038 P-014 `benches/step.rs` (std-only harness: criterion would add some 40 crates);
      number recorded in `doc/budgets.md`
- [x] T-039 P-009 Core doc-tests on every public item; `cargo doc` with intra-doc links as
      errors

## M2 — CLI, config and agent surface

- [x] T-040 P-015 clap tree and `run` alias; `--help` with two examples per command and the
      project URL footer (`ESC-SRS-022`)
- [x] T-041 P-015 Global flags §3 incl. `--quiet`/`--verbose` exclusivity (`ESC-SRS-023`)
- [x] T-042 P-015 Presence-based agent detection, the output cascade and colour precedence
      (`output.rs`)
      (`ESC-SRS-026, 090, 091`)
- [x] T-043 P-015 The JSON envelope (`output.rs`), one compact document
      (`ESC-SRS-024`)
- [x] T-044 P-015 `error.rs` `AppError` {code, exit_code, message, hint, timestamp, command,
      docs_url}; hints written per error after the agentic-CLI catalogue; `human:` prefix
      (`ESC-SRS-025`)
- [x] T-045 P-015 `ExitCode` enum; 6 only from emergency path (`ESC-SRS-027, 095`)
- [x] T-046 P-015 `--version` text and JSON attribution (`ESC-SRS-028`)
- [x] T-047 P-016 Config model with `deny_unknown_fields`; `toml` span → line/column in the
      error (`ESC-SRS-030, 092`)
- [x] T-048 P-016 Search order `--config` → XDG → `/etc` (`ESC-SRS-029`)
- [x] T-049 P-016 Tap-timeout range and `--tap-timeout` override (`ESC-SRS-031, 033`)
- [x] T-050 P-016 `--map` parser (`KEY=TAP[;…]`, xcape key names → `KeyCode`)
      (`ESC-SRS-032`)
- [x] T-051 P-016 Per-device rules by name or `vendor:product` (`ESC-SRS-034`)
- [x] T-052 P-016 `config check` with no device I/O (`ESC-SRS-035`)
- [x] T-053 P-017 `device list` from sysfs (G2 design); `--fields`, `jsonl` (`ESC-SRS-036`)
- [x] T-054 P-018 `--yes` guard off TTY / under agent (`ESC-SRS-037`)
- [x] T-055 P-018 `--dry-run` plan output, no grab (`ESC-SRS-039, 094`)
- [x] T-056 P-018 Orca modifier discovery and protection with warning
      (`ESC-SRS-038, 093`); discovery path recorded in `Design`
- [x] T-057 P-019 `theme.rs` binding §11.1 tokens read from `steelbore.toml`; `NO_COLOR`
      → no colour, `steelbore-mono` by slug; `SPACECRAFT_THEME` honoured
- [x] T-058 P-019 Accessible mode toggle and linear `event monitor` (`ESC-SRS-040`)
- [x] T-059 P-020 `schema` built from one command table; test validates against the 2020-12
      meta-schema
      (`ESC-SRS-041`)
- [x] T-060 P-020 `describe` manifest: commands, flags, exit codes, examples
- [ ] T-108 P-044 `schema --format` provider wrappers (`ESC-SRS-042`) (optional)
- [x] T-061 P-021 `tests/cli/` suite: schema, exit codes, TTY/non-TTY, agent env (real
      `AI_AGENT` value), idempotency, input validation (control chars), Nushell
      roundtrip, UTF-8, schema/describe smoke, context-file presence

## M3 — evdev backend and safety

- [x] T-062 P-022 Traits `Keyboard`, `Pointer`, `Sink`, `Opener` (time passed in, hotplug
      in the loop); fakes in `escapepod-evdev/src/fake.rs`
- [x] T-063 P-022 Real `evdev`/uinput implementations, sysfs + inotify hotplug (G2); poll
      primitive `nix::poll`, since `evdev` already links `nix`
- [x] T-064 P-023 uinput device created first; exit 4 if it fails (`ESC-SRS-043`)
- [x] T-065 P-023 Exclusive grab of matched keyboards; own-device exclusion by uinput
      vendor/product/name tag (`ESC-SRS-044, 045`)
- [x] T-066 P-024 `poll(2)` loop over devices, inotify, signal fd, socket; no timeout
      when idle; `doc/records/idle-analysis.md` (`ESC-SRS-057`)
- [x] T-067 P-024 Pointer devices opened read-only without grab (original `ESC-SRS-053`;
      verification of the amended requirement moves to T-113)
- [x] T-068 P-025 `HeldKeys` ledger and `release_all()`; SIGTERM/SIGINT path ≤ 100 ms
      (`ESC-SRS-046`)
- [x] T-069 P-025 Drop guard on the daemon releasing held keys during a panic's unwind
      (`ESC-SRS-047`)
- [x] T-070 P-025 Emergency chord → release every grab ≤ 100 ms, exit 6
      (`ESC-SRS-048, 096`)
- [x] T-071 P-025 Removal releases that device's held keys (`ESC-SRS-052`)
- [x] T-072 P-026 Read error → release device, warn, continue (`ESC-SRS-050, 097, 098`)
- [x] T-073 P-026 Hotplug grab within 1 s (`ESC-SRS-051`)
- [x] T-074 P-027 SIGHUP reload: validate, apply, keep grabs, warn on failure
      (`ESC-SRS-054, 055, 056, 099`)
- [x] T-075 P-028 Status socket creation (0660, `escapepod-ctl`), JSON Lines frames,
      redaction (`ESC-SRS-059, 060`)
- [x] T-076 P-028 `daemon status` client; `doc/records/privacy-review.md` for `ESC-SRS-058`
- [x] T-077 P-029 `event monitor` default redaction; `--all-keys` guard (`ESC-SRS-061, 062`)
- [x] T-078 P-030 `hardware-tests` feature: uinput-loopback keyboard fixture
- [x] T-079 P-030 CI job with `/dev/uinput` access running the hardware tests; same job in
      `nu tools/ci.nu --only hardware`

## M4 — X11 fallback backend

- [x] T-080 P-031 Backend probe and `auto` selection; `--backend`; exit 4 paths
      (`ESC-SRS-063, 064, 100`)
- [x] T-081 P-032 `escapepod-x11`: XRecord context over key events via `x11rb`
- [x] T-082 P-032 XTest tap injection only; remap rejection with hint
      (`ESC-SRS-065, 066, 088, 101`)
- [x] T-083 P-032 Connection-loss handling: error naming display, exit 1
      (`ESC-SRS-068, 102`)
- [x] T-084 P-032 Runs with no device access (`ESC-SRS-067`): tested with the uinput
      probe and every device directory pointing at nothing
- [x] T-085 P-033 Xvfb harness in `tools/ci.nu` and CI job

## M5 — Privileges, packaging and manual

- [x] T-086 P-034 `escapepod.service`, `sysusers.d/escapepod.conf`,
      `tmpfiles.d/escapepod.conf` (`ESC-SRS-069, 072, 073`)
- [x] T-087 P-034 `70-escapepod.rules` scoped to group `escapepod`; `escapepod-ctl` for the
      socket; packaging test asserting no `input` membership (`ESC-SRS-070, 071`)
- [x] T-088 P-035 NixOS module `services.escapepod`
- [x] T-089 P-035 `nixosTests.escapepod` VM test: service up, udev rule applied, tap works
      via uinput fixture (`ESC-SRS-074`)
- [x] T-090 P-036 `packaging/default.nix` (`ESC-SRS-076`)
- [x] T-091 P-036 `packaging/guix.scm` (`ESC-SRS-075`)
- [x] T-092 P-036 `packaging/PKGBUILD` (`ESC-SRS-077`)
- [x] T-093 P-037 Manual user chapters; xcape migration appendix
- [x] T-094 P-037 `Interfaces` node complete; `schemas/*.json` committed and validated in
      tests (`ESC-SRS-089`)
- [x] T-095 P-037 `make -C doc` builds Info/HTML/PDF with zero warnings; CI docs job
      (`ESC-SRS-078`)

## M6 — Mouse buttons and test safety

- [ ] T-109 P-045 ESC-SRS-105–112 and ESC-NEED-009 in `doc/requirements.toml` and the manual,
      ESC-SRS-053 amended, release gate renumbered M7; G1 re-entry record
- [ ] T-110 P-046 Button map in `Rules` with validation; `Session` button path through the
      ledger (`ESC-SRS-105, 106`)
- [ ] T-111 P-046 Property test: output stays balanced with button shortcuts interleaved
- [ ] T-112 P-047 `UinputPointer` created before any grab; exit 4 if it fails (`ESC-SRS-109`)
- [ ] T-113 P-047 Grab matched pointers once idle; pass unmapped events through in order;
      unmatched pointers read-only (`ESC-SRS-053, 107, 108`)
- [ ] T-114 P-047 Pointer faults, removal, hotplug and reload on the keyboard paths
- [ ] T-115 P-048 `[[button]]` config, global and per device, with limits (`ESC-SRS-112`);
      `config check`, `--dry-run`, `device list`, `schema`, ICD, `schemas/config.schema.json`
- [ ] T-116 P-048 X11 backend refuses button rules (`ESC-SRS-110`)
- [ ] T-117 P-049 `--only-device` allowlist (`ESC-SRS-111`); the hardware tests pass it
- [ ] T-118 P-049 Hardware tests in the NixOS VM by default; host run only with
      `ESCAPEPOD_HW_ON_HOST=1`
- [ ] T-119 P-049 VM test: mouse fixture, side button → Super+Control+Right, motion passes
- [ ] T-120 P-050 Manual: mouse buttons section, xremap migration note, NixOS example
- [ ] T-121 P-050 Maintainer's configuration: side buttons mapped; xremap ignore rule dropped
      once xremap is removed

## M7 — Verification and release gate

- [ ] T-096 P-038 `tools/latency-gate.nu` and the reference workload script
      (`ESC-SRS-079`)
- [ ] T-097 P-038 `tools/rss-gate.nu`, `tools/size-gate.nu`; `doc/budgets.md` with bands
      (`ESC-SRS-080, 103, 104`)
- [ ] T-098 P-039 `fuzz/` targets `config_parse`, `map_parse`; 10⁷ executions recorded
      (`ESC-SRS-081, 082`)
- [ ] T-099 P-040 `cargo llvm-cov` ≥ 80 % in CI (`ESC-SRS-083`)
- [ ] T-100 P-040 `cargo mutants` run and `doc/mutants.md` triage (`ESC-SRS-084`)
- [ ] T-101 P-041 First green GitHub run of `ci.yml` incl. trace gate (`ESC-SRS-085`)
- [ ] T-102 P-041 `release.yml`: SBOM (CycloneDX) + SHA-256, artifacts, checksums
      (`ESC-SRS-086`)
- [ ] T-103 P-042 Clean-install validation from NixOS VM, Guix container, Arch container;
      notes in the manifest (`ESC-SRS-087`)
- [ ] T-104 P-042 Validation of ESC-NEED-001–009 on the reference machine from installed
      packaging
- [ ] T-105 P-042 `CHANGELOG.md`, `doc/release-0.1.md` manifest, anomalies filed,
      `COMPLIANCE.md` re-read, G3 record
- [ ] T-106 P-042 Signed tag `v0.1.0`
- [ ] T-107 P-043 `PROJECTS.md` row updates (Projects repo PRs at M1 start and at release)

---

*— Built by [Spacecraft Software](https://SpacecraftSoftware.org/) —*
