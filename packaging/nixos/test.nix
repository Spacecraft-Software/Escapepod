# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# NixOS VM test of services.escapepod. Verifies: ESC-SRS-074, ESC-SRS-073
# The
# service comes up as the escapepod account, the udev rule grants access,
# a uinput fixture keyboard is grabbed and tapped, the status socket answers
# a member of escapepod-ctl, and the emergency chord exits 6 with no restart.
{
  pkgs,
  module,
  package,
}:

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
            id = "5350:5446";
            button = [
              {
                button = "BTN_SIDE";
                keys = [
                  "KEY_LEFTMETA"
                  "KEY_LEFTCTRL"
                  "KEY_RIGHT"
                ];
              }
            ];
          }
          {
            id = "5350:5447";
            gesture = [
              {
                fingers = 3;
                direction = "left";
                keys = [
                  "KEY_LEFTMETA"
                  "KEY_LEFTCTRL"
                  "KEY_RIGHT"
                ];
              }
            ];
          }
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

    mouse_fixture = """
    import errno, time, evdev
    from evdev import ecodes as e, UInput
    ui = UInput({e.EV_KEY: [e.BTN_LEFT, e.BTN_SIDE], e.EV_REL: [e.REL_X, e.REL_Y]}, name="vm-mouse", vendor=0x5350, product=0x5446, max_effects=0)
    print(ui.device.capabilities(), flush=True)
    time.sleep(1.5)
    outputs = list(map(evdev.InputDevice, evdev.list_devices()))
    keyboard = next(d for d in outputs if d.name.startswith("Escape Pod virtual keyboard"))
    pointer = next(d for d in outputs if d.name.startswith("Escape Pod virtual pointer"))
    keyboard.grab()
    pointer.grab()
    try:
        ui.device.grab()
    except OSError as error:
        assert error.errno == errno.EBUSY, error
    else:
        raise AssertionError("mouse was not grabbed")
    for kind, code, value in [(e.EV_KEY, e.BTN_SIDE, 1), (e.EV_REL, e.REL_X, 7), (e.EV_KEY, e.BTN_LEFT, 1), (e.EV_KEY, e.BTN_LEFT, 0), (e.EV_KEY, e.BTN_SIDE, 0)]:
        ui.write(kind, code, value)
        ui.syn()
        time.sleep(0.02)
    time.sleep(0.3)
    def drain(device):
        result = []
        while True:
            event = device.read_one()
            if event is None: return result
            if event.type != e.EV_SYN: result.append((event.type, event.code, event.value))
    assert drain(keyboard) == [(e.EV_KEY, code, value) for code, value in [(e.KEY_LEFTMETA, 1), (e.KEY_LEFTCTRL, 1), (e.KEY_RIGHT, 1), (e.KEY_RIGHT, 0), (e.KEY_LEFTCTRL, 0), (e.KEY_LEFTMETA, 0)]]
    assert drain(pointer) == [(e.EV_REL, e.REL_X, 7), (e.EV_KEY, e.BTN_LEFT, 1), (e.EV_KEY, e.BTN_LEFT, 0)]
    """
    machine.succeed("cat > /tmp/mouse.py <<'PY'\n" + mouse_fixture + "\nPY")

    # A three-finger touchpad: 30 units per millimetre on both axes.
    # Verifies: ESC-SRS-113, ESC-SRS-114, ESC-SRS-115, ESC-SRS-116, ESC-SRS-119, ESC-SRS-120
    touchpad_fixture = """
    import time, evdev
    from evdev import ecodes as e, UInput, AbsInfo
    caps = {
        e.EV_KEY: [e.BTN_LEFT, e.BTN_TOUCH, e.BTN_TOOL_FINGER, e.BTN_TOOL_DOUBLETAP, e.BTN_TOOL_TRIPLETAP, e.BTN_TOOL_QUADTAP],
        e.EV_ABS: [
            (e.ABS_X, AbsInfo(0, 0, 3000, 0, 0, 30)),
            (e.ABS_Y, AbsInfo(0, 0, 2000, 0, 0, 30)),
            (e.ABS_MT_SLOT, AbsInfo(0, 0, 4, 0, 0, 0)),
            (e.ABS_MT_POSITION_X, AbsInfo(0, 0, 3000, 0, 0, 30)),
            (e.ABS_MT_POSITION_Y, AbsInfo(0, 0, 2000, 0, 0, 30)),
            (e.ABS_MT_TRACKING_ID, AbsInfo(0, 0, 65535, 0, 0, 0)),
        ],
    }
    ui = UInput(caps, name="vm-touchpad", vendor=0x5350, product=0x5447,
                input_props=[e.INPUT_PROP_POINTER, e.INPUT_PROP_BUTTONPAD])
    time.sleep(1.5)
    # Never grabbed: our own exclusive grab succeeds.
    ui.device.grab()
    ui.device.ungrab()
    keyboard = next(d for d in map(evdev.InputDevice, evdev.list_devices())
                    if d.name.startswith("Escape Pod virtual keyboard"))
    keyboard.grab()
    def frame(events):
        for kind, code, value in events:
            ui.write(kind, code, value)
        ui.syn()
        time.sleep(0.01)
    land = []
    for slot in range(3):
        land += [(e.EV_ABS, e.ABS_MT_SLOT, slot), (e.EV_ABS, e.ABS_MT_TRACKING_ID, 100 + slot),
                 (e.EV_ABS, e.ABS_MT_POSITION_X, 2500 + 100 * slot), (e.EV_ABS, e.ABS_MT_POSITION_Y, 1000)]
    frame(land + [(e.EV_KEY, e.BTN_TOUCH, 1), (e.EV_KEY, e.BTN_TOOL_TRIPLETAP, 1)])
    # 30 mm to the left in fifteen steps of 2 mm.
    for step in range(1, 16):
        moves = []
        for slot in range(3):
            moves += [(e.EV_ABS, e.ABS_MT_SLOT, slot), (e.EV_ABS, e.ABS_MT_POSITION_X, 2500 + 100 * slot - 60 * step)]
        frame(moves)
    lift = []
    for slot in range(3):
        lift += [(e.EV_ABS, e.ABS_MT_SLOT, slot), (e.EV_ABS, e.ABS_MT_TRACKING_ID, -1)]
    frame(lift + [(e.EV_KEY, e.BTN_TOOL_TRIPLETAP, 0), (e.EV_KEY, e.BTN_TOUCH, 0)])
    time.sleep(0.3)
    got = []
    while True:
        event = keyboard.read_one()
        if event is None: break
        if event.type == e.EV_KEY: got.append((event.code, event.value))
    want = [(e.KEY_LEFTMETA, 1), (e.KEY_LEFTCTRL, 1), (e.KEY_RIGHT, 1), (e.KEY_RIGHT, 0), (e.KEY_LEFTCTRL, 0), (e.KEY_LEFTMETA, 0)]
    assert got == want, got
    """
    machine.succeed("cat > /tmp/touchpad.py <<'PY'\n" + touchpad_fixture + "\nPY")

    machine.wait_for_unit("escapepod.service")
    assert machine.succeed("systemctl show -p User --value escapepod").strip() == "escapepod"
    machine.succeed("test -c /dev/uinput && stat -c %G /dev/uinput | grep -qx escapepod")
    machine.fail("id -nG alice | grep -qw input")
    machine.fail("id -nG escapepod | grep -qw input")

    # A tap through the real kernel path.
    machine.succeed("python3 /tmp/fixture.py >&2")
    machine.succeed("python3 /tmp/mouse.py >&2")
    machine.succeed("python3 /tmp/touchpad.py >&2")

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
