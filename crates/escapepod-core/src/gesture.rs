// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Touchpad swipes: a recogniser fed the kernel's multi-touch events.
//!
//! The backend reads a touchpad without grabbing it and passes in two kinds
//! of event: the number of fingers on the pad (from `BTN_TOOL_*TAP`) and the
//! motion of the contacts in device units. [`SwipeRecognizer`] turns them
//! into finished [`Swipe`]s. It does no I/O and reads no clock, like the rest
//! of this crate, so every decision replays in tests.

/// The direction of a swipe, by the axis it moved along most.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    /// Towards the left edge of the touchpad.
    Left,
    /// Towards the right edge.
    Right,
    /// Towards the top edge.
    Up,
    /// Towards the bottom edge.
    Down,
}

impl Direction {
    /// Parses `left`, `right`, `up` or `down` (any case).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            _ => None,
        }
    }

    /// The lower-case name, as configuration and reports spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// One touchpad event, as the backend reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchInput {
    /// The number of fingers now on the touchpad.
    Fingers(u8),
    /// Motion of the contacts since the last report, in device units
    /// (positive `dx` to the right, positive `dy` downwards).
    Motion {
        /// Horizontal motion.
        dx: i32,
        /// Vertical motion.
        dy: i32,
    },
}

/// A touchpad's resolution in device units per millimetre, per axis.
///
/// A touchpad that reports no resolution cannot measure a swipe in
/// millimetres; the backend refuses it for gestures instead of building
/// one of these with a zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    x: u32,
    y: u32,
}

impl Resolution {
    /// The resolution, or `None` when either axis reports zero units per
    /// millimetre.
    #[must_use]
    pub const fn new(x: u32, y: u32) -> Option<Self> {
        if x == 0 || y == 0 {
            None
        } else {
            Some(Self { x, y })
        }
    }
}

/// A finished swipe: its finger count, its direction and how far it
/// travelled along that direction, in micrometres. No position is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swipe {
    /// Three or four.
    pub fingers: u8,
    /// The dominant axis and its sign.
    pub direction: Direction,
    /// Travel along `direction`, in micrometres.
    pub travel_um: u64,
}

/// What one [`TouchInput`] did to the swipe in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeEvent {
    /// Nothing finished.
    None,
    /// A swipe ended: its finger count fell.
    Ended(Swipe),
    /// A swipe was cancelled: its finger count rose. Carries the finger
    /// count it had.
    Cancelled(u8),
}

/// Travel, in micrometres on either axis, below which a swipe is still
/// settling: a fourth finger landing then joins it instead of cancelling.
pub const SETTLE_UM: i64 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Active {
    fingers: u8,
    dx: i64,
    dy: i64,
}

/// Recognises three- and four-finger swipes on one touchpad.
///
/// A swipe starts when three or four fingers are down, gathers motion, and
/// ends when the count falls (the fingers lift) or is cancelled when it
/// rises (another finger lands). After either, no new swipe starts until
/// fewer than three fingers remain, so the last fingers of a lift never
/// start one.
///
/// Fingers never land at the same instant: the kernel commonly reports
/// three a few milliseconds before the fourth. A rise from three to four
/// before the swipe has moved [`SETTLE_UM`] restarts it as a four-finger
/// swipe instead of cancelling it; the three-finger one emits nothing
/// either way.
///
/// # Examples
///
/// ```
/// use escapepod_core::{Direction, Resolution, SwipeEvent, SwipeRecognizer, TouchInput};
///
/// let res = Resolution::new(10, 10).expect("non-zero");
/// let mut swipe = SwipeRecognizer::new(res);
/// swipe.step(TouchInput::Fingers(3));
/// swipe.step(TouchInput::Motion { dx: -300, dy: 20 });
/// let SwipeEvent::Ended(s) = swipe.step(TouchInput::Fingers(2)) else {
///     unreachable!("three fingers lifted");
/// };
/// assert_eq!((s.fingers, s.direction, s.travel_um), (3, Direction::Left, 30_000));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwipeRecognizer {
    resolution: Resolution,
    active: Option<Active>,
    armed: bool,
}

impl SwipeRecognizer {
    /// A recogniser for a touchpad with `resolution`.
    #[must_use]
    pub const fn new(resolution: Resolution) -> Self {
        Self {
            resolution,
            active: None,
            armed: true,
        }
    }

    /// Whether a swipe is in progress.
    #[must_use]
    pub const fn in_swipe(&self) -> bool {
        self.active.is_some()
    }

    /// Consumes one touchpad event.
    // Implements: ESC-SRS-115, ESC-SRS-116, ESC-SRS-118, ESC-SRS-121
    pub fn step(&mut self, input: TouchInput) -> SwipeEvent {
        match input {
            TouchInput::Motion { dx, dy } => {
                if let Some(a) = self.active.as_mut() {
                    a.dx = a.dx.saturating_add(i64::from(dx));
                    a.dy = a.dy.saturating_add(i64::from(dy));
                }
                SwipeEvent::None
            }
            TouchInput::Fingers(n) => self.on_fingers(n),
        }
    }

    fn on_fingers(&mut self, n: u8) -> SwipeEvent {
        if let Some(a) = self.active {
            if n == 4 && a.fingers == 3 && self.settling(a) {
                self.active = Some(Active {
                    fingers: 4,
                    dx: 0,
                    dy: 0,
                });
                return SwipeEvent::None;
            }
            if n > a.fingers {
                self.active = None;
                self.armed = false;
                return SwipeEvent::Cancelled(a.fingers);
            }
            if n < a.fingers {
                self.active = None;
                self.armed = n < 3;
                return SwipeEvent::Ended(self.finish(a));
            }
            return SwipeEvent::None;
        }
        if n < 3 {
            self.armed = true;
        } else if self.armed && (n == 3 || n == 4) {
            self.active = Some(Active {
                fingers: n,
                dx: 0,
                dy: 0,
            });
        }
        SwipeEvent::None
    }

    fn settling(&self, a: Active) -> bool {
        micrometres(a.dx, self.resolution.x).abs() < SETTLE_UM
            && micrometres(a.dy, self.resolution.y).abs() < SETTLE_UM
    }

    fn finish(&self, a: Active) -> Swipe {
        let x_um = micrometres(a.dx, self.resolution.x);
        let y_um = micrometres(a.dy, self.resolution.y);
        // A tie goes to the horizontal axis, where workspace swipes live.
        let (direction, travel_um) = if x_um.unsigned_abs() >= y_um.unsigned_abs() {
            let d = if x_um < 0 {
                Direction::Left
            } else {
                Direction::Right
            };
            (d, x_um.unsigned_abs())
        } else {
            let d = if y_um < 0 {
                Direction::Up
            } else {
                Direction::Down
            };
            (d, y_um.unsigned_abs())
        };
        Swipe {
            fingers: a.fingers,
            direction,
            travel_um,
        }
    }
}

/// `units` of travel at `per_mm` units per millimetre, in micrometres.
fn micrometres(units: i64, per_mm: u32) -> i64 {
    units.saturating_mul(1000) / i64::from(per_mm.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec() -> SwipeRecognizer {
        SwipeRecognizer::new(Resolution::new(10, 20).unwrap_or(Resolution { x: 1, y: 1 }))
    }

    fn run(r: &mut SwipeRecognizer, inputs: &[TouchInput]) -> Vec<SwipeEvent> {
        inputs
            .iter()
            .map(|&i| r.step(i))
            .filter(|e| *e != SwipeEvent::None)
            .collect()
    }

    use TouchInput::{Fingers as F, Motion as M};

    #[test]
    // Verifies: ESC-SRS-115, ESC-SRS-116
    fn a_three_finger_swipe_ends_when_the_fingers_lift() {
        let mut r = rec();
        let got = run(
            &mut r,
            &[
                F(1),
                F(2),
                F(3),
                M { dx: 150, dy: 40 },
                M { dx: 100, dy: -40 },
                F(2),
                F(1),
                F(0),
            ],
        );
        assert_eq!(
            got,
            [SwipeEvent::Ended(Swipe {
                fingers: 3,
                direction: Direction::Right,
                travel_um: 25_000,
            })]
        );
    }

    #[test]
    // Verifies: ESC-SRS-116
    fn the_dominant_axis_wins_in_millimetres_not_units() {
        // 100 units across at 10/mm is 10 mm; 160 units down at 20/mm is
        // 8 mm. More units downwards, but the swipe went across.
        let mut r = rec();
        let got = run(&mut r, &[F(4), M { dx: -100, dy: 160 }, F(3)]);
        assert_eq!(
            got,
            [SwipeEvent::Ended(Swipe {
                fingers: 4,
                direction: Direction::Left,
                travel_um: 10_000,
            })]
        );
        let got = run(&mut r, &[F(0), F(3), M { dx: 10, dy: -400 }, F(0)]);
        assert_eq!(
            got,
            [SwipeEvent::Ended(Swipe {
                fingers: 3,
                direction: Direction::Up,
                travel_um: 20_000,
            })]
        );
    }

    #[test]
    // Verifies: ESC-SRS-118
    fn a_fourth_finger_cancels_and_nothing_restarts_until_the_lift() {
        let mut r = rec();
        let got = run(
            &mut r,
            &[
                F(3),
                M { dx: 500, dy: 0 },
                F(4),
                M { dx: 500, dy: 0 },
                F(3),
                F(2),
                F(0),
            ],
        );
        assert_eq!(got, [SwipeEvent::Cancelled(3)]);
        assert!(!r.in_swipe());
    }

    #[test]
    // Verifies: ESC-SRS-118
    fn a_fourth_finger_landing_before_any_travel_makes_a_four_finger_swipe() {
        // The kernel reports three, then four, as the fingers land.
        let mut r = rec();
        let got = run(
            &mut r,
            &[F(3), M { dx: 5, dy: 0 }, F(4), M { dx: -300, dy: 0 }, F(0)],
        );
        assert_eq!(
            got,
            [SwipeEvent::Ended(Swipe {
                fingers: 4,
                direction: Direction::Left,
                travel_um: 30_000,
            })]
        );
    }

    #[test]
    // Verifies: ESC-SRS-121
    fn a_lift_through_three_fingers_ends_one_swipe_only() {
        // Four fingers lifting one by one pass through three: that must
        // not start a three-finger swipe.
        let mut r = rec();
        let got = run(
            &mut r,
            &[
                F(4),
                M { dx: 300, dy: 0 },
                F(3),
                M { dx: 300, dy: 0 },
                F(2),
                F(0),
            ],
        );
        assert_eq!(got.len(), 1);
        assert!(matches!(
            got[0],
            SwipeEvent::Ended(Swipe { fingers: 4, .. })
        ));
    }

    #[test]
    fn motion_without_a_swipe_and_two_fingers_start_nothing() {
        let mut r = rec();
        assert!(
            run(
                &mut r,
                &[M { dx: 900, dy: 0 }, F(2), M { dx: 900, dy: 0 }, F(0)]
            )
            .is_empty()
        );
        assert!(run(&mut r, &[F(5), M { dx: 900, dy: 0 }, F(0)]).is_empty());
    }

    #[test]
    fn a_resolution_needs_both_axes() {
        assert!(Resolution::new(0, 10).is_none());
        assert!(Resolution::new(10, 0).is_none());
        assert!(Resolution::new(10, 10).is_some());
    }

    #[test]
    fn directions_parse_and_print() {
        for d in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ] {
            assert_eq!(Direction::from_name(d.name()), Some(d));
            assert_eq!(
                Direction::from_name(&d.name().to_ascii_uppercase()),
                Some(d)
            );
        }
        assert_eq!(Direction::from_name("sideways"), None);
    }
}

// Rust guideline compliant 2026-05-18
