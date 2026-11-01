<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Escape Pod — idle analysis

Verifies: ESC-SRS-057, by analysis (Standard §21.1), with a supporting test.

**Requirement.** While no input event, signal, hotplug event, or socket connection arrives, the
daemon shall not wake.

**Analysed** 2026-11-01 at the M3 branch.

1. The daemon has exactly one blocking call: `nix::poll::poll` in `service.rs` `wait`. There is
   no other thread, no async runtime and no `sleep` in the loop.
2. The descriptors polled are the `signalfd` (signals), the inotify watch on `/dev/input`
   (hotplug), the status listener and its clients (socket connections), and the keyboards and
   pointers (input events): exactly the four sources the requirement names.
3. The timeout is `Daemon::timeout()`: `None`, which `wait` passes as `PollTimeout::NONE`
   (block forever), except while a keyboard is waiting to go idle before its grab, when it is
   20 ms. That wait ends after at most `IDLE_WAIT` (1 s), when the keyboard is grabbed or
   dropped (`Daemon::tick`); after that the timeout is `None` again. A keyboard waits only when
   it reported keys down at the moment it appeared, which is itself input activity.
4. Nothing else re-arms a timer: the engine decides taps on release (pass-through), so no key
   needs a timeout to resolve.

The supporting test `idle_daemon_has_no_timer` (`daemon.rs`) checks that `timeout()` is `None`
once every keyboard is grabbed, and `waits_for_idle_before_grabbing` checks that the timer exists
only while a keyboard waits and is gone after its grab.

**Result:** pass. The loop wakes only for the four named sources, apart from a bounded idle-wait
window that follows input.
