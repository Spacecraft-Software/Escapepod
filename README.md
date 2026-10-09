<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Escape Pod

**Tap a key for Escape, hold it for Control.** Escape Pod is a Linux daemon that
gives one key two jobs: tapped on its own it sends one key, held with other keys
it acts as a modifier. It is the Spacecraft Software successor to
[xcape](https://github.com/ollef/xcape), rebuilt in Rust on the kernel input
layer (evdev and uinput), so it works under X11, Wayland and the console. Where a
session has no `/dev/input` access it falls back to X11 (XRecord and XTest).

Status: **Planning, M0 in progress.** Nothing is built or released yet. The plan is in
[`PLAN.md`](PLAN.md) and the task list is in [`TODO.md`](TODO.md). The requirements are in
the manual, [`doc/escapepod.texi`](doc/escapepod.texi), nodes `Needs` and
`Requirements`.

## Layout

| Path | Role |
|---|---|
| `doc/` | The Texinfo manual and SRS (`escapepod.texi`) |
| `tools/` | Nushell gates; `nu tools/ci.nu` runs the local CI mirror |
| `PRD.md`, `PLAN.md`, `TODO.md` | The §17.5 tracking documents |
| `COMPLIANCE.md` | The §19.5 tailoring register and gate records |

## Building

There is no code to build yet. The local gate runs today:

```nu
nu tools/ci.nu
```

## Project Posture

Escape Pod is a **personal / hobby** project under the Spacecraft Software
umbrella (The Steelbore Standard §5.1). It is provided AS IS, without warranty
or liability; see `NOTICE.md`. Contributions are welcome, but accepted at the
maintainer's sole discretion; see `CONTRIBUTING.md`. **Support window:** best effort
on the latest release only, with no backports (§26).

**Assurance category (Standard §19): Category B.** The daemon grabs every
keyboard on the machine, so a fault can leave the user unable to type. The
tailoring register lives in `COMPLIANCE.md`.

Conforms to The Steelbore Standard v2.12 — Category B, tailored (see
`COMPLIANCE.md`).

Escape Pod has no graphical user interface and declares no §13 component
system.

## Conformance and assurance evidence

- `COMPLIANCE.md` is the §19.5 tailoring register and holds the gate records.
- `CREDITS.md` lists whose work this stands on (§15.3).
- `SECURITY.md` says how to report a finding (§26.1).

## License

Code is `GPL-3.0-or-later` and documents are `CC-BY-SA-4.0` (Standard §4.1.1).
The canonical GPL text is the regular file `LICENSE`, and
`LICENSES/GPL-3.0-or-later.txt` links to it (§4.3).

## Maintainer

Mohamed Hammad — Mohamed.Hammad [at] SpacecraftSoftware.org
<https://EscapePod.SpacecraftSoftware.org/>

*— Built by Spacecraft Software —*
