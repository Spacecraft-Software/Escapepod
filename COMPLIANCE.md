<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->
<!-- GFM Document
     title:      Escape Pod — Compliance
     author:     Mohamed Hammad & Spacecraft Software
     date:       2026-10-08
     license:    CC-BY-SA-4.0
     project:    Escape Pod
-->

# Escape Pod — Compliance

Escape Pod declares assurance **Category B** under The Steelbore Standard §19. This file is the
§19.5 tailoring register. A clause absent from the register is applied in full; every entry is
re-read at the G3 release gate.


## Tailoring register

| Clause | Status | Justification | Date |
|--------|--------|---------------|------|
| `spacecraft-agentic-cli` §4 (implicit `--yes` under `AI_AGENT` / `AGENT`) | tailored | `daemon run` grabs every keyboard, so an agent must pass `--yes` explicitly (ESC-SRS-037); the same skill's §7 threat model takes precedence over its §4 default | 2026-10-08 |
| §24.2 toolchain pin in `rust-toolchain.toml` | tailored | The release toolchain is pinned by `flake.lock` (nixpkgs `6774f7bc`, the revision Operator pins); `nix develop -c rustc -V` names the exact rustc, and CI pins the same version. The MSRV is a separate figure, `rust-version = "1.88.0"` in `Cargo.toml`, checked by CI's own msrv job. A rustup-downloaded toolchain does not run on NixOS without an FHS loader, so a `rust-toolchain.toml` pin would break the maintainer's own builds, which is the environment §21.5 validation runs in. | 2026-10-10 |


## Gate records

### G1 — Requirements (2026-11-01)

The §20.4 characteristics gate was run over ESC-SRS-001 to ESC-SRS-102, one requirement at a
time and then over the set. The set moved from the hand-written `Requirements` chapter into
`doc/requirements.toml`; `tools/srs.nu` renders it back into the manual, and CI fails when the
two differ. Every requirement now carries an identifier, a rationale, a source, a priority, one
§21.1 method and the status `baselined`. Category B is declared in `README.md`, `AGENTS.md`
and `PROJECTS.md` (ESC-SRS-004).

| # | Requirement | Characteristic | Finding | Resolution |
|---|---|---|---|---|
| G1-1 | ESC-SRS-079, 080 | Complete | The term *reference machine* deferred the exact model to G2, a to-be-decided inside a budget requirement | Terminology now names the machine decided on 2026-10-08 (ThinkPad T490s, daemon pinned to two cores). The workload is still recorded at G2 (T-027), which is design, not requirement text |
| G1-2 | ESC-SRS-023 | Unambiguous | "every global flag of CLI Standard §3" depended on whichever CLI Standard version is current | Source pinned to CLI Standard v1.1.0 |
| G1-3 | ESC-SRS-038, 093 | Feasible | How Orca's modifier setting is discovered varies by Orca version | Accepted. The discovery path is a design item for the `Design` node (T-056); the requirement states the observable behaviour only |
| G1-4 | ESC-SRS-075 | Feasible | The reference machine has no Guix, so `guix build -f` cannot run there | Accepted. Verified in a Guix container at G3 (T-103) |
| G1-5 | ESC-SRS-046, 048 | Singular | Each pairs a deadline (100 ms) with the action | Not a defect: one obligation with a constraint (§20.6 constrained shape) |
| G1-6 | ESC-SRS-002, 003, 005 | Singular | Each lists several files | Not a defect: one obligation (presence of a fixed set), one inspection, one pass or fail |
| G1-7 | Set | Able to be validated | Needs ESC-NEED-001 to 008 each checked for a tracing requirement | Every need has at least one: 001 (009–016), 002 (044, 051, 063–068), 003 (018, 043, 046–056), 004 (036, 058–062, 069–071), 005 (009, 017, 057, 079, 080), 006 (022–028), 007 (020, 038), 008 (032, 033) |
| G1-8 | Set | Complete | Error, degraded and shutdown behaviour checked | Covered: signal, panic, removal, read error, failed reload, lost X connection, emergency exit |

No requirement was withdrawn. The baseline is the commit tagged `g1-requirements-0.1`.

---

*— Built by [Spacecraft Software](https://SpacecraftSoftware.org/) —*
