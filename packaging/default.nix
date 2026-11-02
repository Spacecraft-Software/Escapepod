# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The Escape Pod package (Standard §5.5; ESC-SRS-076): the binary, the
# systemd unit, the udev rule, sysusers and tmpfiles entries, and the Info
# manual. Built from the locked Cargo inputs.
{
  lib,
  rustPlatform,
  texinfo,
  acl,
  nushell,
  testers,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "escapepod";
  version = "0.0.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
      ../doc
      ../schemas
      ../packaging
      ../AGENTS.md
      ../CLAUDE.md
      ../SKILL.md
      ../CONTRIBUTING.md
      ../clippy.toml
      ../rustfmt.toml
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ texinfo ];
  # The CLI suite round-trips JSON through Nushell when it is present.
  nativeCheckInputs = [ nushell ];

  postBuild = ''
    makeinfo --no-split -o escapepod.info doc/escapepod.texi
  '';

  postInstall = ''
    install -Dm644 escapepod.info $out/share/info/escapepod.info
    install -Dm644 packaging/systemd/escapepod.service $out/lib/systemd/system/escapepod.service
    substituteInPlace $out/lib/systemd/system/escapepod.service --replace-fail @BINDIR@ $out/bin
    install -Dm644 packaging/udev/70-escapepod.rules $out/lib/udev/rules.d/70-escapepod.rules
    substituteInPlace $out/lib/udev/rules.d/70-escapepod.rules --replace-fail @SETFACL@ ${acl}/bin/setfacl
    install -Dm644 packaging/sysusers.d/escapepod.conf $out/lib/sysusers.d/escapepod.conf
    install -Dm644 packaging/tmpfiles.d/escapepod.conf $out/lib/tmpfiles.d/escapepod.conf
    install -Dm644 doc/config.example.toml $out/share/doc/escapepod/config.example.toml
  '';

  passthru.tests.version = testers.testVersion {
    package = finalAttrs.finalPackage;
    command = "escapepod --version --format yaml";
  };

  meta = {
    description = "Tap a key for Escape, hold it for Control (the xcape successor)";
    homepage = "https://EscapePod.SpacecraftSoftware.org/";
    license = lib.licenses.gpl3Plus;
    maintainers = [ ];
    mainProgram = "escapepod";
    platforms = lib.platforms.linux;
  };
})
