<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Credits

This file lists third-party work that Escape Pod substantially builds on (The
Steelbore Standard §15.3). It leaves out routine dependency use, whose licence
`Cargo.lock` and `cargo deny` already surface mechanically. Only work whose
design Escape Pod leans on directly is listed.

## Prior art

These works shaped the design. Escape Pod copies no code from any of them.

| Name | Author(s) | Source | Scope |
|---|---|---|---|
| xcape | The xcape authors | <https://github.com/ollef/xcape> | **The behaviour Escape Pod succeeds.** It contributes the dual-role tap/hold model, the tap timeout, and the `--map 'KEY=TAP'` and `-t` command-line grammar, which Escape Pod accepts for compatibility (ESC-SRS-032). |

## Interface data

| Name | Source | Scope |
|---|---|---|
| Linux input event codes | `include/uapi/linux/input-event-codes.h`, Linux 7.2 | The `KEY_*` and `BTN_*` names and numbers in `crates/escapepod-core/src/keytable.rs`, generated from the header. They are the kernel's user-space ABI, which the header's Linux-syscall-note places outside the kernel's GPL-2.0 for programs that use it. |
| X keysym names | X11 `keysymdef.h` | The xcape-compatible names (`Caps_Lock`, `Control_L`) that `KeyCode::from_name` accepts, mapped by hand to kernel keys. |

## Engineering references

These are the references behind the Category B process (§19–§26) that the manual's SRS
follows:

- **ISO/IEC/IEEE 29148** — requirement characteristics and the needs →
  requirements structure.
- **ECSS-E-ST-40C** (European Cooperation for Space Standardization, software
  engineering) — the requirements → design → verification gate sequence and
  traceability.
