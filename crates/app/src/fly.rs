//! The fly (gs#309): Fit and find ease the viewport to their target over [`FLY_MS`] instead of
//! jumping, so a viewer sees where the view went. A wheel, pinch, drag or key cancels it at once;
//! reduced motion jumps. The target is worked out the way a jump works it out: the move is made
//! on the viewport, read, and the viewport put back to where it was.
//!
//! [`Session::advance_fly`] steps it once per frame from the session clock, before the frame
//! draws. A flying frame does the work of a zoom or pan frame and draws the same rectangles.

use crate::Session;

/// How long a fly takes, in milliseconds.
pub const FLY_MS: f64 = 300.0;

/// The three numbers of a viewport that a fly moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pose {
    t0: f64,
    px_per_minute: f64,
    scroll_y: f64,
}

/// A running fly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fly {
    started: f64,
    from: Pose,
    to: Pose,
    /// What the last step set. A viewport that is not this was moved by something else.
    shown: Pose,
}

/// Smoothstep: 0 at 0, 1 at 1, slow at both ends.
fn ease(p: f64) -> f64 {
    p * p * (3.0 - 2.0 * p)
}

impl Pose {
    /// The point `p` (0 to 1) of the way: the scale moves by ratio, so a zoom feels even.
    fn at(self, to: Pose, p: f64) -> Pose {
        let lerp = |a: f64, b: f64| a + (b - a) * p;
        Pose {
            t0: lerp(self.t0, to.t0),
            px_per_minute: if self.px_per_minute > 0.0 && to.px_per_minute > 0.0 {
                self.px_per_minute * (to.px_per_minute / self.px_per_minute).powf(p)
            } else {
                lerp(self.px_per_minute, to.px_per_minute)
            },
            scroll_y: lerp(self.scroll_y, to.scroll_y),
        }
    }
}

impl Session {
    /// The viewport as it is now, for [`Session::fly_from`] to fly from.
    pub(crate) fn pose(&self) -> Pose {
        let v = &self.timeline.viewport;
        Pose {
            t0: v.t0,
            px_per_minute: v.px_per_minute,
            scroll_y: v.scroll_y,
        }
    }

    fn set_pose(&mut self, pose: Pose) {
        let v = &mut self.timeline.viewport;
        (v.t0, v.px_per_minute, v.scroll_y) = (pose.t0, pose.px_per_minute, pose.scroll_y);
        self.link_refresh();
    }

    /// The viewport has just been jumped to its target from `from`: fly there instead. With
    /// reduced motion, or no move, the jump stands. It replaces a fly that runs.
    pub(crate) fn fly_from(&mut self, from: Pose) {
        let to = self.pose();
        if self.reports.motion.reduced_motion || to == from {
            self.reports.motion.fly = None;
            return;
        }
        self.reports.motion.fly = Some(Fly {
            started: (self.clock)(),
            from,
            to,
            shown: from,
        });
        self.set_pose(from);
    }

    /// Step the fly to the session clock. A viewport that something else moved since the last
    /// step (a wheel, a drag, a scroll) ends the fly where it is. Call once per frame.
    pub fn advance_fly(&mut self) {
        let Some(fly) = self.reports.motion.fly else {
            return;
        };
        // A press is on the chart (a pan, a bar or a link drag): the person has the view. Reduced
        // motion turned on mid-flight lands it.
        if self.pointer_held() || fly.shown != self.pose() {
            self.reports.motion.fly = None;
            return;
        }
        if self.reports.motion.reduced_motion {
            self.land_fly();
            return;
        }
        let p = ((self.clock)() - fly.started) / FLY_MS;
        if p >= 1.0 || p.is_nan() {
            self.set_pose(fly.to);
            self.reports.motion.fly = None;
            return;
        }
        let shown = fly.from.at(fly.to, ease(p.max(0.0)));
        self.set_pose(shown);
        self.reports.motion.fly = Some(Fly { shown, ..fly });
    }

    /// Stop a fly where it is: a key, a wheel or a drag has taken the view.
    pub fn cancel_fly(&mut self) {
        self.reports.motion.fly = None;
    }

    /// Finish a fly at once, at its target.
    pub fn land_fly(&mut self) {
        if let Some(fly) = self.reports.motion.fly.take() {
            self.set_pose(fly.to);
        }
    }

    /// Whether a fly runs: not while a press holds the chart, which has ended it.
    pub fn flying(&self) -> bool {
        self.reports.motion.fly.is_some() && !self.pointer_held()
    }

    fn pointer_held(&self) -> bool {
        self.click.is_some() || self.linking()
    }
}
