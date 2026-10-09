<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Security Policy

Escape Pod reads every keystroke on the machine and can take exclusive control of
the keyboard, so the keystroke data and the device permissions are the main
attack surface. This document is The Steelbore Standard §26.1 statement of how
to report a finding and what to expect (ESC-SRS-005).

## Reporting channel

Report privately through **GitHub's private vulnerability reporting** on this
repository (Security → Report a vulnerability). If that is unavailable to you,
email **Mohamed.Hammad@SpacecraftSoftware.org**; a PGP key is available on
request. **Never open a public issue** for a security finding.

## Acknowledgement target

This is a hobby project (Standard §5.1). You will receive an acknowledgement
within **14 days**.

## Scope

In scope: the `escapepod`, `escapepod-core`, `escapepod-evdev` and
`escapepod-x11` crates in this repository, the systemd, udev, sysusers and
tmpfiles units, the NixOS module, the packaging definitions, and the CI
workflows. These findings are of particular interest:

- a keystroke, or anything derived from one, that leaves the process other
  than by the uinput device. This includes logs, the status socket, `event monitor`
  output without `--all-keys`, and error messages;
- device access granted to any account other than `escapepod`, or status
  socket access that also grants device access;
- a path that leaves a keyboard grabbed, or a key held down, after the daemon
  exits, crashes, loses a device, or sees the emergency chord;
- a way for an agent environment to start a grab without an explicit `--yes`,
  or to obtain `--all-keys` output;
- a way to make the daemon read a device it created itself (a feedback loop).

Out of scope: the kernel input subsystem, udev, and the X server themselves.
Report those upstream.

## Supported versions

Best effort on the **latest release only**. No backports. Pre-release
snapshots receive fixes on `main`.

## Coordinated disclosure

The maintainer honours a **90-day** embargo from acknowledgement, extendable
by mutual agreement. After that the finding is published regardless of fix
status. A fixed vulnerability is published as a GitHub Security Advisory that
names the affected versions, the fixed version, the impact and the
workaround, and cites the release SBOM (Standard §26.2).

Security anomalies are **S1 by default** (Standard §25.2).

## Credit

Reporters are credited in the advisory and in `CREDITS.md` unless they ask
not to be.
