<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Mouse-button integration evidence — 2026-11-02

The evdev daemon now connects button rules to the pure session and forwards
unmapped relative-pointer events through a virtual pointer. Both virtual
outputs exist before device admission. Unsupported pointers remain ungrabbed.

## Checks

- The default local mirror, `nix develop --command nu tools/ci.nu`, passed all
  20 gates, including workspace tests, X11, Clippy, audit and deny.
- Additional pointer tests passed after the output-failure path was added:
  idle waiting, reload, unsupported capabilities, read failure, unwind and
  virtual-keyboard write failure. No host device was used.
- Manual, PDF, traceability and progress gates passed after the manual changes.
- `nix build --no-link -L .#checks.x86_64-linux.nixos` passed. Its VM fixture
  verified BTN_SIDE → Left Meta + Left Control + Right, reverse-order release,
  relative motion and unmapped left-button pass-through. Existing keyboard,
  service-account, socket-permission and emergency-release checks also passed.
  The successful VM derivation was
  `/nix/store/fa0grdvjafkg1cn7rkrzd6pvmq6p3by1-vm-test-run-escapepod.drv`.

The first mouse fixture was correctly refused because Python evdev's default
nonzero force-feedback capacity advertised an unsupported capability. The
fixture now explicitly sets `max_effects=0`; production capability checks were
not relaxed.

## Scope and remaining work

Only relative mice with supported button, axis and scan capabilities can be
grabbed. Absolute touchpads and other unsupported devices remain ungrabbed
with a diagnostic. Hardware-gate invocations select the VM by default; the
host suite requires `ESCAPEPOD_HW_ON_HOST=1` and passes the fixture device ID.

No host service was installed or restarted, and xremap and the maintainer's
personal configuration were not changed. Verification preceded the implementation commit.
Tracking items retain their existing merge-based completion rule; this record
does not declare the whole M6 milestone complete or rebaseline requirements.

## Merge and verification reconciliation — 2026-11-03

The requirements baseline landed separately in
[PR #15](https://github.com/Spacecraft-Software/Escapepod/pull/15), merge
`ad6e69c4534a49df6f488162467c52a22d94abf5`. The implementation and review fixes
landed in [PR #16](https://github.com/Spacecraft-Software/Escapepod/pull/16),
merge `8bb17c6a0637a2f6705dc5f8ec14486ce9e4ab87`.

All seven jobs in [CI run 41598965058](https://github.com/Spacecraft-Software/Escapepod/actions/runs/41598965058)
passed on implementation head `4730e7f7f47987b1eef961387ced48b9274a6eb8`:
workspace tests, strict Clippy, formatting, Rust documentation, supply-chain
checks, structural and traceability checks, manual builds, X11 and hardware
loopback tests, and the MSRV checks. CodeRabbit skipped; it is not counted as
a completed review.

A fresh `nix build --no-link -L .#checks.x86_64-linux.nixos` also passed on
2026-11-03 after the acquisition-race fixes. The VM verified mouse shortcut
press and reverse release, relative motion, unmapped left-button press and
release, keyboard tap, service identity and permissions, status socket access,
and emergency exit 6 with no restart. It supplements the earlier VM result
above. The acquisition races and startup fault injection are covered by
deterministic tests, rather than by the VM.

The following passing tests support promoting the M6 rows to `verified`:

| Requirement | Evidence in the merged tree |
|---|---|
| ESC-SRS-053 | `daemon.rs`: unmatched pointer remains ungrabbed beside a mapped mouse; pointer presses still mark the dual-role key used |
| ESC-SRS-105, 106 | `session.rs`: ordered shortcut presses, reverse releases and shared ownership; interleaved-button property test; VM shortcut observation |
| ESC-SRS-107, 108 | `daemon.rs`: grab, motion and button pass-through, orphan release, idle acquisition races, reload, faults and unwind; VM motion and button observation |
| ESC-SRS-109 | `service.rs`: virtual-pointer creation failure returns 4 and never enters device startup |
| ESC-SRS-110 | CLI test: button config is visible and an X11 run returns 2 |
| ESC-SRS-111 | `daemon.rs`: allowlist survives hotplug and reload; CLI dry-run validation and isolated hardware fixture |
| ESC-SRS-112 | `config.rs`: invalid button rules rejected; core rule tests reject empty and more-than-eight-key shortcuts; direct `config check` checks return exit 2 for zero and nine distinct keys |

Verifies: ESC-SRS-085. The successful posture job ran both the matrix gate
and `tools/tests/trace.nu`. Its fixture tests require an unknown identifier,
a markerless verified row and a required release orphan to fail, while a
clean fixture passes. `.github/workflows/ci.yml` invokes these gates on every
push and PR. This supports T-101 and the traceability requirement; P-041
remains open because the release SBOM work is not complete.

T-109 through T-120 and P-045 through P-049 are now complete under the
merge-based done rule. The requirement statements and G1 baseline wording
are unchanged; only verification statuses and matching checklist ticks move.
T-121 and P-050 remain open: no host configuration or service was changed,
and xremap has not been retired. The whole M6 milestone remains incomplete.

After this reconciliation, `nix develop --command nu tools/ci.nu` passed all
20 default gates on 2026-11-03, including the regenerated SRS freshness,
traceability and progress checks. The direct shortcut-boundary checks used
temporary configs and `ESCAPEPOD_UINPUT` pointing to a missing path; they
started no daemon and created no input device.
