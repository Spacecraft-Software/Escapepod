# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# NixOS VM test of services.escapepod. Verifies: ESC-SRS-074, ESC-SRS-073
# The
# service comes up as the escapepod account, the udev rule grants access,
# a uinput fixture keyboard is grabbed and tapped, the status socket answers
# a member of escapepod-ctl, and the emergency chord exits 6 with no restart.
{ pkgs, module, package }:

pkgs.testers.runNixOSTest {
  name = "escapepod";

  nodes.machine = {
    imports = [ module ];
    services.escapepod = {
      enable = true;
      inherit package;
      statusUsers = [ "alice" ];
      settings = {
        grab = "listed";
        device = [
          {
            id = "5350:5445";
            dual_role = [
              {
                key = "Caps_Lock";
                hold = "Control_L";
                tap = "Escape";
              }
            ];
          }
        ];
      };
    };
    users.users.alice.isNormalUser = true;
    environment.systemPackages = [ (pkgs.python3.withPackages (p: [ p.evdev ])) ];
  };

  testScript = ''
    import json

    fixture = """
    import evdev, sys, time
    from evdev import ecodes as e, UInput
    caps = [e.KEY_CAPSLOCK, e.KEY_BACKSPACE, e.KEY_ESC, e.KEY_ENTER, e.KEY_A, e.KEY_Z, e.KEY_SPACE, e.KEY_LEFTCTRL]
    ui = UInput({e.EV_KEY: caps}, name="vm-fixture", vendor=0x5350, product=0x5445)
    time.sleep(1.5)
    out = [d for d in map(evdev.InputDevice, evdev.list_devices()) if d.name.startswith("Escape Pod virtual keyboard")][0]
    out.grab()
    def key(c, v):
        ui.write(e.EV_KEY, c, v); ui.syn(); time.sleep(0.02)
    key(e.KEY_CAPSLOCK, 1); key(e.KEY_CAPSLOCK, 0)
    time.sleep(0.3)
    got = []
    while True:
        ev = out.read_one()
        if ev is None: break
        if ev.type == e.EV_KEY: got.append((ev.code, ev.value))
    print(got)
    assert got == [(e.KEY_LEFTCTRL, 1), (e.KEY_LEFTCTRL, 0), (e.KEY_ESC, 1), (e.KEY_ESC, 0)], got
    if sys.argv[1:] == ["chord"]:
        for c in (e.KEY_BACKSPACE, e.KEY_ESC, e.KEY_ENTER):
            key(c, 1)
        time.sleep(0.5)
    """
    machine.succeed("cat > /tmp/fixture.py <<'PY'\n" + fixture + "\nPY")

    machine.wait_for_unit("escapepod.service")
    assert machine.succeed("systemctl show -p User --value escapepod").strip() == "escapepod"
    machine.succeed("test -c /dev/uinput && stat -c %G /dev/uinput | grep -qx escapepod")
    machine.fail("id -nG alice | grep -qw input")
    machine.fail("id -nG escapepod | grep -qw input")

    # A tap through the real kernel path.
    machine.succeed("python3 /tmp/fixture.py >&2")

    # The status socket: 0660, group escapepod-ctl, readable by alice.
    machine.succeed("stat -c '%a %G' /run/escapepod/escapepod.sock | grep -qx '660 escapepod-ctl'")
    status = json.loads(machine.succeed("su alice -c 'escapepod daemon status --json'"))
    assert status["data"]["backend"] == "evdev", status

    # The emergency chord: exit 6 and no restart.
    machine.succeed("python3 /tmp/fixture.py chord >&2")
    machine.wait_until_succeeds("test \"$(systemctl show -p ExecMainStatus --value escapepod)\" = 6")
    machine.sleep(3)
    assert machine.succeed("systemctl show -p NRestarts --value escapepod").strip() == "0"
    machine.fail("systemctl is-active escapepod")
  '';
}
