<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Escape Pod — M5 package builds

The package definitions build and test the working tree on the reference machine,
2026-11-02. The release form of each (the signed tag's tarball and its SHA-256) is
re-checked by the clean-install validation at G3 (ESC-SRS-087, T-103).

| Requirement | Command | Result |
|---|---|---|
| Verifies: ESC-SRS-076 | `nix build .#default` (`packaging/default.nix`) | Built; the check phase ran the workspace tests (42 + 25 + 30 + 9 doc-tests and the property tests), all passing; the output holds `bin/escapepod`, the unit, the udev rule, sysusers, tmpfiles, the Info manual and the example configuration |
| Verifies: ESC-SRS-077 | `makepkg --nodeps` in `packaging/` (pacman 7.0.0 from nixpkgs, `MAKEPKG_CONF` from it) | Built `escapepod-0.0.0-1-x86_64.pkg.tar.gz`; `check()` passed; the package lists `usr/bin/escapepod`, the unit, the rule, sysusers, tmpfiles, the manual and the example |
| ESC-SRS-075 (not verified) | `guix build -f packaging/guix.scm` | Not run: the reference machine has no Guix daemon. `tools/guix-crates.nu --check` confirms the crate inputs match `Cargo.lock`. Verified in a Guix container at G3 (T-103) |

The NixOS VM test (`nix build .#checks.x86_64-linux.nixos`, `packaging/nixos/test.nix`)
passed the same day in 20 s of test time: service as `escapepod`, `/dev/uinput` group
`escapepod`, no account in `input`, a tap through the kernel path, the socket `0660`
group `escapepod-ctl` readable by a member, and the emergency chord ending the
service with status 6 and no restart.
