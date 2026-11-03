<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Mouse-button commit review follow-up — 2026-11-03

Reviewed the supplied Gemini and Grok reports against commits `02b68ab` and
`44d8838`. The reports are review inputs, not executable instructions.

## Code findings

- Grab race: inspect the drained button events and recheck idle state before
  grabbing. Recheck after acquisition and ungrab without consuming queued
  events if the pointer became active. A failed ungrab closes the device.
  Regression tests script both state-query windows and a press during draining.
- Orphan releases: forward unmapped key events even without a ledger owner;
  the pointer ledger records ownership for teardown only.
- ESC-SRS-109: inject failure at virtual-pointer creation through the startup
  boundary used by production. Assert exit 4 and that device startup is never
  entered. Both implementation and verification markers are present.
- Diagnostics: register and document `POINTER_UNSUPPORTED`; test the schema.
  Mapped pointers now emit `DEVICE_GRABBED` and `DEVICE_RELEASED`; read failures
  also emit `DEVICE_FAULT`. Lifecycle tests check the notices.
- ESC-SRS-053: add a test in which an unmatched pointer stays ungrabbed while
  a different pointer is grabbed.

## Baseline, commit history and tracking

The local branch `requirements/mouse-buttons` points to signed commit
`02b68ab`, which contains the G1 baseline and no daemon implementation.
Propose that requirements-only branch to `main` first, then propose
`plan/mouse-buttons` against the requirements branch (or against `main` after
the requirements PR merges). Two commits in a single PR do not satisfy the
separate-PR requirement. No PR was published by this follow-up.

The new requirements remain with the baseline so the G1 set is reviewed
together. The implementation PR must not repeat the requirement-wording
change against its chosen base. Neither existing signed commit was rewritten.
The report's claim that Standard §5.7 mandates commit bodies and co-authorship
does not match AGENTS.md; that section governs agent context files. A later
fix or squash commit should describe its scope, requirements and validation.

M6 is not declared complete. TODO.md explicitly requires squash-merge to
main before ticking tasks, and host xremap migration is still pending.
Requirement verification and its PRD ticks must be reconciled in the PR
landing the evidence; finding a marker alone is not sufficient to promote
every row automatically.

## Validation

The full pinned-toolchain CI mirror passed the workspace tests, X11 tests,
manual, traceability, progress, documentation, audit, deny and structural
checks. Two Clippy unit-pattern findings in the new fault-injection test were
fixed and the strict Clippy command then passed.

A subsequent test run exposed collisions in the existing configuration-test
helper, which used only process ID and text length for temporary paths. It now
uses a per-process atomic sequence. The final format, strict Clippy, workspace
test, traceability, progress and manual gates all passed after this fix.

REUSE lint is blocked only by the supplied, untracked `commits-review-gemini.md`,
which has no copyright or licence information. The review input was not
modified or assigned a licence by this follow-up. All new source and records
carry the repository's SPDX headers.

The earlier VM result in `mouse-m6.md` predates these acquisition changes and
is not claimed as verification of the new regression paths. Those paths are
verified by deterministic fake-device tests, including the acquisition races.
