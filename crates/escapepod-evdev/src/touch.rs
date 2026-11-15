// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Kernel multi-touch events to the engine's [`TouchInput`].
//!
//! [`TouchDecoder`] is pure: it is fed the raw `(type, code, value)` events of
//! a touchpad, frame by frame, and appends what happened at each
//! `SYN_REPORT`. Kept apart from the device so recorded traces replay in
//! tests.

use escapepod_core::TouchInput;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const SYN_REPORT: u16 = 0x00;
const SYN_DROPPED: u16 = 0x03;

const BTN_TOOL_FINGER: u16 = 0x145;
const BTN_TOOL_QUINTTAP: u16 = 0x148;
const BTN_TOOL_DOUBLETAP: u16 = 0x14d;
const BTN_TOOL_TRIPLETAP: u16 = 0x14e;
const BTN_TOOL_QUADTAP: u16 = 0x14f;

const ABS_MT_SLOT: u16 = 0x2f;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;

/// Slots tracked; touchpads report at most five contacts.
const SLOTS: usize = 10;

/// The finger count a `BTN_TOOL_*` key stands for.
fn tool_fingers(code: u16) -> Option<u8> {
    match code {
        BTN_TOOL_FINGER => Some(1),
        BTN_TOOL_DOUBLETAP => Some(2),
        BTN_TOOL_TRIPLETAP => Some(3),
        BTN_TOOL_QUADTAP => Some(4),
        BTN_TOOL_QUINTTAP => Some(5),
        _ => None,
    }
}

/// One slot's state. The kernel keeps a slot's position across contacts
/// and reports only coordinates that change, so a new contact in a slot can
/// arrive with a tracking id and no position: the position is the slot's,
/// not the contact's, and outlives a lift.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Slot {
    /// The tracking id of the contact in the slot, if one is down.
    id: Option<i32>,
    x: i32,
    y: i32,
}

/// The scale the mean motion is carried at: 2520 is the least common
/// multiple of 1 to 10, so dividing a frame's summed motion by any number of
/// contacts up to [`SLOTS`] is exact, and no fraction of a unit is ever lost
/// to truncation between frames.
const SCALE: i64 = 2520;

/// Decodes one touchpad's multi-touch events.
///
/// The finger count comes from the `BTN_TOOL_*` keys (ESC-SRS-115): the
/// kernel sets exactly one of them while fingers are down. Motion is the
/// mean movement of the contacts present in both this frame and the last,
/// matched by slot and tracking id, so a finger landing or lifting adds no
/// jump.
///
/// # Examples
///
/// ```
/// use escapepod_core::TouchInput;
/// use escapepod_evdev::TouchDecoder;
///
/// let mut dec = TouchDecoder::default();
/// let mut out = Vec::new();
/// // Slot 0 lands at x = 100 with three fingers down.
/// for (t, c, v) in [(3, 0x2f, 0), (3, 0x39, 7), (3, 0x35, 100), (3, 0x36, 50), (1, 0x14e, 1), (0, 0, 0)] {
///     dec.feed(t, c, v, &mut out);
/// }
/// // It moves to x = 160.
/// for (t, c, v) in [(3, 0x35, 160), (0, 0, 0)] {
///     dec.feed(t, c, v, &mut out);
/// }
/// assert_eq!(out, [TouchInput::Fingers(3), TouchInput::Motion { dx: 60, dy: 0 }]);
/// ```
#[derive(Clone, Debug, Default)]
pub struct TouchDecoder {
    slot: usize,
    /// Slots as the current frame builds them.
    current: [Slot; SLOTS],
    /// Slots at the last `SYN_REPORT`.
    previous: [Slot; SLOTS],
    /// Mean motion not yet reported, in units of 1/[`SCALE`].
    carry: (i64, i64),
    /// The `BTN_TOOL_*` key held, as a finger count.
    tool: u8,
    /// The finger count last reported.
    reported: u8,
    /// A `SYN_DROPPED` was seen: events up to the next report are lost.
    dropped: bool,
}

impl TouchDecoder {
    /// Feeds one kernel event; at a `SYN_REPORT`, appends the frame's
    /// motion (if any) and then a new finger count (if it changed).
    // Implements: ESC-SRS-115
    pub fn feed(&mut self, kind: u16, code: u16, value: i32, out: &mut Vec<TouchInput>) {
        match (kind, code) {
            (EV_SYN, SYN_DROPPED) => self.dropped = true,
            (EV_SYN, SYN_REPORT) => self.report(out),
            _ if self.dropped => {}
            (EV_KEY, c) => {
                if let Some(n) = tool_fingers(c) {
                    if value != 0 {
                        self.tool = n;
                    } else if self.tool == n {
                        self.tool = 0;
                    }
                }
            }
            (EV_ABS, ABS_MT_SLOT) => {
                self.slot = usize::try_from(value).unwrap_or(0).min(SLOTS - 1);
            }
            (EV_ABS, ABS_MT_TRACKING_ID) => {
                self.current[self.slot].id = (value >= 0).then_some(value);
            }
            (EV_ABS, ABS_MT_POSITION_X) => self.current[self.slot].x = value,
            (EV_ABS, ABS_MT_POSITION_Y) => self.current[self.slot].y = value,
            _ => {}
        }
    }

    fn report(&mut self, out: &mut Vec<TouchInput>) {
        if self.dropped {
            // The frame is incomplete: start the motion afresh from here and
            // let the next frame report the count.
            self.dropped = false;
            self.previous = self.current;
            return;
        }
        // Only contacts down in both frames, with the same tracking id,
        // move: a contact that just landed or lifted adds no jump.
        let (mut sx, mut sy, mut n) = (0i64, 0i64, 0i64);
        let kept = self
            .current
            .iter()
            .zip(&self.previous)
            .filter(|(a, b)| a.id.is_some() && a.id == b.id);
        for (a, b) in kept {
            sx += i64::from(a.x) - i64::from(b.x);
            sy += i64::from(a.y) - i64::from(b.y);
            n += 1;
        }
        if n > 0 {
            self.carry.0 = self.carry.0.saturating_add(sx.saturating_mul(SCALE) / n);
            self.carry.1 = self.carry.1.saturating_add(sy.saturating_mul(SCALE) / n);
        } else {
            // No contact carried over: a remainder belongs to no motion.
            self.carry = (0, 0);
        }
        let (dx, dy) = (self.carry.0 / SCALE, self.carry.1 / SCALE);
        if dx != 0 || dy != 0 {
            self.carry.0 -= dx * SCALE;
            self.carry.1 -= dy * SCALE;
            out.push(TouchInput::Motion {
                dx: clamp(dx),
                dy: clamp(dy),
            });
        }
        if self.tool != self.reported {
            self.reported = self.tool;
            out.push(TouchInput::Fingers(self.tool));
        }
        self.previous = self.current;
    }
}

fn clamp(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    type Ev = (u16, u16, i32);
    const SYN: Ev = (EV_SYN, SYN_REPORT, 0);

    fn land(slot: i32, id: i32, x: i32, y: i32) -> [Ev; 4] {
        [
            (EV_ABS, ABS_MT_SLOT, slot),
            (EV_ABS, ABS_MT_TRACKING_ID, id),
            (EV_ABS, ABS_MT_POSITION_X, x),
            (EV_ABS, ABS_MT_POSITION_Y, y),
        ]
    }

    fn feed(dec: &mut TouchDecoder, events: &[Ev]) -> Vec<TouchInput> {
        let mut out = Vec::new();
        for &(t, c, v) in events {
            dec.feed(t, c, v, &mut out);
        }
        out
    }

    // Verifies: ESC-SRS-115
    #[test]
    fn a_recorded_three_finger_swipe_decodes_to_count_and_mean_motion() {
        let mut dec = TouchDecoder::default();
        let mut frame = Vec::new();
        for (slot, id, x) in [(0, 1, 100), (1, 2, 200), (2, 3, 300)] {
            frame.extend(land(slot, id, x, 500));
        }
        frame.push((EV_KEY, BTN_TOOL_TRIPLETAP, 1));
        frame.push(SYN);
        assert_eq!(feed(&mut dec, &frame), [TouchInput::Fingers(3)]);
        // All three move left by 30, 30 and 60: the mean is 40.
        let moved = feed(
            &mut dec,
            &[
                (EV_ABS, ABS_MT_SLOT, 0),
                (EV_ABS, ABS_MT_POSITION_X, 70),
                (EV_ABS, ABS_MT_SLOT, 1),
                (EV_ABS, ABS_MT_POSITION_X, 170),
                (EV_ABS, ABS_MT_SLOT, 2),
                (EV_ABS, ABS_MT_POSITION_X, 240),
                SYN,
            ],
        );
        assert_eq!(moved, [TouchInput::Motion { dx: -40, dy: 0 }]);
        // The fingers lift: the tool goes from TRIPLETAP to none.
        let lifted = feed(
            &mut dec,
            &[
                (EV_ABS, ABS_MT_SLOT, 0),
                (EV_ABS, ABS_MT_TRACKING_ID, -1),
                (EV_ABS, ABS_MT_SLOT, 1),
                (EV_ABS, ABS_MT_TRACKING_ID, -1),
                (EV_ABS, ABS_MT_SLOT, 2),
                (EV_ABS, ABS_MT_TRACKING_ID, -1),
                (EV_KEY, BTN_TOOL_TRIPLETAP, 0),
                SYN,
            ],
        );
        assert_eq!(lifted, [TouchInput::Fingers(0)]);
    }

    #[test]
    fn a_finger_landing_adds_no_motion_and_changes_the_count() {
        let mut dec = TouchDecoder::default();
        let mut first = land(0, 1, 100, 100).to_vec();
        first.push((EV_KEY, BTN_TOOL_FINGER, 1));
        first.push(SYN);
        feed(&mut dec, &first);
        // A second finger lands far away; the first stays still. The kernel
        // swaps FINGER for DOUBLETAP in the same frame.
        let mut second = land(1, 2, 900, 900).to_vec();
        second.extend([
            (EV_KEY, BTN_TOOL_FINGER, 0),
            (EV_KEY, BTN_TOOL_DOUBLETAP, 1),
            SYN,
        ]);
        assert_eq!(feed(&mut dec, &second), [TouchInput::Fingers(2)]);
    }

    #[test]
    fn a_dropped_frame_is_skipped_and_motion_restarts_from_the_next() {
        let mut dec = TouchDecoder::default();
        let mut first = land(0, 1, 100, 100).to_vec();
        first.extend([(EV_KEY, BTN_TOOL_TRIPLETAP, 1), SYN]);
        feed(&mut dec, &first);
        let lost = feed(
            &mut dec,
            &[
                (EV_SYN, SYN_DROPPED, 0),
                (EV_ABS, ABS_MT_POSITION_X, 5000),
                SYN,
            ],
        );
        assert!(lost.is_empty());
        let next = feed(&mut dec, &[(EV_ABS, ABS_MT_POSITION_X, 110), SYN]);
        assert_eq!(next, [TouchInput::Motion { dx: 10, dy: 0 }]);
    }

    #[test]
    fn a_new_contact_keeps_the_slot_position_the_kernel_retained() {
        // A contact lifts at x = 3000; the next lands in the same slot at
        // the same x, so the kernel sends the tracking id but no position.
        let mut dec = TouchDecoder::default();
        let mut first = land(0, 1, 3000, 500).to_vec();
        first.push(SYN);
        feed(&mut dec, &first);
        feed(
            &mut dec,
            &[
                (EV_ABS, ABS_MT_SLOT, 0),
                (EV_ABS, ABS_MT_TRACKING_ID, -1),
                SYN,
            ],
        );
        assert!(feed(&mut dec, &[(EV_ABS, ABS_MT_TRACKING_ID, 2), SYN]).is_empty());
        // It moves by one unit: one unit of motion, not 3001.
        let moved = feed(&mut dec, &[(EV_ABS, ABS_MT_POSITION_X, 3001), SYN]);
        assert_eq!(moved, [TouchInput::Motion { dx: 1, dy: 0 }]);
    }

    #[test]
    fn sub_unit_mean_motion_is_carried_not_lost() {
        // Three contacts; one moves a unit per frame, in turn: a third of a
        // unit of mean motion each frame, a whole unit every three.
        let mut dec = TouchDecoder::default();
        let mut frame = Vec::new();
        for (slot, id) in [(0, 1), (1, 2), (2, 3)] {
            frame.extend(land(slot, id, 1000, 500));
        }
        frame.push(SYN);
        feed(&mut dec, &frame);
        let mut x = [1000; 3];
        let mut total = 0;
        for step in 0..300usize {
            let slot = step % 3;
            x[slot] += 1;
            let slot_value = i32::try_from(slot).unwrap_or(0);
            for input in feed(
                &mut dec,
                &[
                    (EV_ABS, ABS_MT_SLOT, slot_value),
                    (EV_ABS, ABS_MT_POSITION_X, x[slot]),
                    SYN,
                ],
            ) {
                if let TouchInput::Motion { dx, .. } = input {
                    total += dx;
                }
            }
        }
        // Every contact moved 100 units; so did their mean.
        assert_eq!(total, 100);
    }

    #[test]
    fn a_reused_slot_with_a_new_tracking_id_is_not_motion() {
        let mut dec = TouchDecoder::default();
        let mut first = land(0, 1, 100, 100).to_vec();
        first.push(SYN);
        feed(&mut dec, &first);
        let swapped = feed(
            &mut dec,
            &[
                (EV_ABS, ABS_MT_SLOT, 0),
                (EV_ABS, ABS_MT_TRACKING_ID, 9),
                (EV_ABS, ABS_MT_POSITION_X, 800),
                SYN,
            ],
        );
        assert!(swapped.is_empty());
    }
}

// Rust guideline compliant 2026-05-18
