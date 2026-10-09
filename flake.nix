# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The Escape Pod development environment (PLAN P-005). This flake is the
# release toolchain pin (Standard §24.2 / §25.1): flake.lock freezes nixpkgs
# 6774f7bc, the same revision Operator pins. The MSRV (Cargo.toml
# `rust-version`) is a separate figure, checked by CI's own msrv job. There is
# deliberately no rust-toolchain.toml: a rustup-downloaded toolchain does not
# run on NixOS without an FHS loader (tailoring row in COMPLIANCE.md).
# Package outputs and the NixOS module join at M5 (P-035, P-036).
{
  description = "Escape Pod — tap a key for Escape, hold it for Control";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = [
              # Rust: the pinned release toolchain and the §21/§24 gates.
              pkgs.cargo
              pkgs.rustc
              pkgs.clippy
              pkgs.rustfmt
              pkgs.rust-analyzer
              pkgs.cargo-audit
              pkgs.cargo-deny
              pkgs.cargo-nextest
              pkgs.cargo-mutants
              pkgs.cargo-llvm-cov
              pkgs.cargo-cyclonedx
              pkgs.cargo-bloat
              # Budgets (§22): latency and RSS measurement.
              pkgs.hyperfine
              # Posture, manual and the Nushell gates (tools/).
              pkgs.reuse
              pkgs.texinfo
              pkgs.gnumake
              pkgs.nushell
              pkgs.jaq
              pkgs.nixfmt
              # The X11 fallback's test harness (P-033).
              pkgs.xvfb-run
              pkgs.xdotool
              # The evdev backend's headers come through libudev (P-022).
              pkgs.pkg-config
              pkgs.udev
              pkgs.git
              pkgs.gh
            ];
            # Reproducibility (Standard §25.3): mkShell otherwise bakes the
            # checkout path into every binary's RUNPATH.
            NIX_NO_SELF_RPATH = true;
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          };
        }
      );

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
