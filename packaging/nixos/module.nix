# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# NixOS module `services.escapepod` (ESC-SRS-074): the service, the udev
# rule, the escapepod account and escapepod-ctl group, and the
# configuration file. No login account is added to `input` (ESC-SRS-071).
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.escapepod;
  toml = pkgs.formats.toml { };
in
{
  options.services.escapepod = {
    enable = lib.mkEnableOption "the Escape Pod dual-role key daemon";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The Escape Pod package.";
    };

    settings = lib.mkOption {
      type = toml.type;
      default = { };
      example = {
        dual_role = [
          {
            key = "Caps_Lock";
            hold = "Control_L";
            tap = "Escape";
          }
        ];
      };
      description = "The configuration, written to /etc/escapepod/config.toml (manual: Configuration format).";
    };

    statusUsers = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = "Login accounts allowed to read the status socket (group escapepod-ctl). This grants no device access.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    environment.etc."escapepod/config.toml".source = toml.generate "escapepod.toml" cfg.settings;

    users.groups.escapepod = { };
    users.groups.escapepod-ctl.members = cfg.statusUsers;
    users.users.escapepod = {
      isSystemUser = true;
      group = "escapepod";
      extraGroups = [ "escapepod-ctl" ];
      description = "Escape Pod daemon";
    };

    boot.kernelModules = [ "uinput" ];
    services.udev.packages = [ cfg.package ];
    systemd.packages = [ cfg.package ];
    systemd.tmpfiles.packages = [ cfg.package ];
    systemd.services.escapepod = {
      wantedBy = [ "multi-user.target" ];
      restartTriggers = [ config.environment.etc."escapepod/config.toml".source ];
    };
  };
}
