<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Escape Pod — M0 inspection record

Standard §21.1 counts a manual check as inspection, recorded with what was inspected and when.
This file is that record for the M0 requirements whose method is inspection. Each line names
the evidence; where a `tools/ci.nu` gate repeats the inspection on every change, the gate is the
standing evidence and this record is the dated first reading.

Inspected 2026-11-01 at `main` after PR #5 (`2b20c38`), by Claude Code for the maintainer.

| Requirement | Inspected | Result | Standing gate |
|---|---|---|---|
| Verifies: ESC-SRS-001 | `reuse lint` over the tree | Compliant, every file licensed | `ci.nu` gate `reuse` |
| Verifies: ESC-SRS-002 | `README.md`, `NOTICE.md`, `CONTRIBUTING.md`, `LICENSE` (regular file), `LICENSES/` | All present at the root | `ci.nu` gate `posture` |
| Verifies: ESC-SRS-003 | `AGENTS.md`, `CLAUDE.md`, `SKILL.md` | All present at the root | `ci.nu` gate `posture` |
| Verifies: ESC-SRS-004 | `README.md` and `AGENTS.md` text; `PROJECTS.md` line 76 in Spacecraft-Software/Projects | All three say "Category B" | `ci.nu` gate `category` (the two local files) |
| Verifies: ESC-SRS-005 | `SECURITY.md` sections | Channel, 14-day acknowledgement, scope, supported versions, 90-day disclosure, credit: all present | `ci.nu` gate `posture` (presence) |
| Verifies: ESC-SRS-006 | `COMPLIANCE.md` | Holds the §19.5 tailoring register | `ci.nu` gate `posture` (presence) |
| Verifies: ESC-SRS-007 | `.github/workflows/ci.yml` and `tools/ci.nu` | The workflow runs `fmt`, `clippy -D warnings`, `test`, `audit` and `deny` as required jobs; the same gates run locally before every PR (AGENTS.md) | `ci.nu` gates `fmt` to `deny` |

GitHub Actions is billing-blocked as of this date, so the workflow has not yet produced a run;
the merge block is held by the local mirror, which AGENTS.md requires before every PR. The
first green GitHub run is checked at G3 (T-101).
