<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# CLAUDE.md

@AGENTS.md

> Record project knowledge in `AGENTS.md`, not here. This file holds only
> Claude-Code-only context (Standard §5.7).

## Skills to load

1. `engram-context escapepod`, then `spacecraft-steelbore-standard`.
2. `microsoft-rust-guidelines` before any `.rs` file. Add `spacecraft-rust-guidelines` on top
   for the `poll(2)` loop and the latency budget.
3. `spacecraft-cli-standard` + `spacecraft-agentic-cli` for the CLI (M2), and before
   editing `SKILL.md`.
4. `spacecraft-accessibility-support` + `steelbore-color-palette` for the theme and
   the accessible `event monitor`.
5. `spacecraft-nu-guidelines` for every `.nu` under `tools/`.
6. `spacecraft-nix-guidelines` for `flake.nix` and `packaging/default.nix`, and
   `spacecraft-guile-guidelines` for `packaging/guix.scm`.
7. `spacecraft-texinfo-document` for `doc/*.texi`.
8. `spacecraft-cli-shell` + `spacecraft-cli-preference` before shell commands;
   `spacecraft-missing-pkg` when a tool is absent.

Give the maintainer shell commands in Nushell syntax.
