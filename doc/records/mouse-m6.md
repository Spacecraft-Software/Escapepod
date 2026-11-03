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
