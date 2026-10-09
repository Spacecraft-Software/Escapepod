---
name: escapepod
description: >
  Escape Pod, a Linux dual-role key daemon (the xcape successor): tap a key for
  one key, hold it for a modifier, plus one-to-one remaps, via evdev/uinput with
  an X11 fallback. Use when the user wants Caps Lock or Control to send Escape
  on tap, wants to inspect or validate an escapepod config, list keyboards and
  whether they match, check daemon status, or watch Escape Pod's tap/hold
  decisions. Subcommands: daemon run, daemon status, event monitor, device
  list, config check, schema, describe. Starting a grab needs --yes under an
  agent.
license: GPL-3.0-or-later
version: 0.0.0
---

<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Escape Pod — capability surface

> **Status:** pre-release. The surface below is the designed one (PRD,
> ESC-SRS-022 to 042); the commands land at M2 and M3. Until then the binary
> exits 1.

## Sub-command tree

- `escapepod daemon run` — grab matched keyboards and apply the rules. Needs
  `--yes` off a terminal or under an agent; `--dry-run` prints the plan and
  grabs nothing. Human alias: `escapepod run`.
- `escapepod daemon status` — read the running daemon's state from its status
  socket (needs group `escapepod-ctl`).
- `escapepod event monitor` — stream Escape Pod's decisions. Unmanaged keys
  show as `other`. `--all-keys` needs a real terminal and is refused under
  an agent.
- `escapepod device list` — list input devices from udev metadata and
  whether the rules match them. Opens no event device.
- `escapepod config check` — validate a config file and print errors with
  line and column. Opens no device.
- `escapepod schema` — JSON Schema (2020-12) of every command's input and
  output.
- `escapepod describe` — capability manifest.

## Output formats

json (default when piped or under an agent), jsonl (streams), human (tty).

## Global flags

--json, --format, --fields, --dry-run, --verbose, --quiet, --no-color,
--color, --help, --version, --yes, --accessible

Daemon flags: --config PATH, --backend auto|evdev|x11,
--map 'KEY=TAP[;KEY=TAP…]', --tap-timeout MS (50–2000).

## Exit codes

0 success, 1 general failure, 2 usage or config error, 3 not found,
4 permission denied or backend unavailable, 5 conflict,
6 emergency release (emergency chord pressed; the service does not restart).

## Examples

```
escapepod config check --config ~/.config/escapepod/config.toml --json
escapepod device list --fields name,matched --format jsonl
escapepod daemon run --map 'Control_L=Escape' --dry-run --json
escapepod daemon run --map 'Control_L=Escape' --yes
escapepod daemon status --json
```
