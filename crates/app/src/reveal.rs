//! The driving path lights up from the selected task back (gs#309). A new selection reveals its
//! `back` chain over [`REVEAL_MS`], nearest first, on the ease-out cubic of the slide; the forward
//! chain lights with the last frame. Reduced motion shows the whole path at once, and an edit that
//! rebuilds the same task's path does not replay it.
//!
//! What a frame costs: the partial path is made once when a selection starts (one copy of the
//! highlight) and again after an edit rebuilds the path. Every other frame lights only the tasks
//! whose rank the clock has reached since the last frame, by index into `back`; a frame that
//! reaches no new rank does nothing, and so does an idle one. It adds no rectangles.

use crate::Session;
use timeline::scene::Highlight;

/// How long the reveal of a new selection's path takes, in milliseconds.
pub const REVEAL_MS: f64 = 300.0;

/// What the session keeps for the reveal.
#[derive(Default)]
pub(crate) struct Reveal {
    /// The task whose reveal this is; a different selected task restarts it.
    task: Option<u32>,
    /// The session clock when it started.
    started: f64,
    /// The whole path is lit: the reveal ended, or reduced motion is on.
    done: bool,
    /// The `chain_builds` the partial path was made from; an edit changes it.
    builds: u32,
    /// How many of the selection's `back` tasks are lit, nearest first.
    lit: usize,
    /// The partial path, made when the reveal starts or an edit rebuilds the path.
    shown: Option<Highlight>,
}

impl Session {
    /// The highlight to draw: the path revealed so far while a reveal runs, else the whole path.
    /// Before [`Session::advance_reveal`] has seen the selection it is the whole path.
    pub fn revealed_highlight(&self) -> Option<&Highlight> {
        let selection = self.selection.as_ref()?;
        match &self.reveal.shown {
            Some(partial) if self.reveal.task == Some(selection.task) => Some(partial),
            _ => Some(selection.highlight()),
        }
    }

    /// One frame of the reveal: restart it when the selected task changed, then light the ranks
    /// the session clock has reached.
    pub fn advance_reveal(&mut self) {
        let Some(selection) = &self.selection else {
            self.reveal = Reveal::default();
            return;
        };
        let now = (self.clock)();
        let reveal = &mut self.reveal;
        if reveal.task != Some(selection.task) {
            *reveal = Reveal {
                task: Some(selection.task),
                started: now,
                ..Reveal::default()
            };
        }
        let p = ((now - reveal.started) / REVEAL_MS).clamp(0.0, 1.0);
        if self.reports.motion.reduced_motion || p >= 1.0 {
            reveal.done = true;
        }
        if reveal.done {
            reveal.shown = None;
            return;
        }
        let back = &selection.back;
        if reveal.shown.is_none() || reveal.builds != self.chain_builds {
            // A new selection, or an edit rebuilt the path: unlight both chains, then light again
            // the ranks already reached, so the reveal carries on where it was.
            let ranked = back.iter().chain(&selection.forward).copied();
            let mut partial = selection.highlight().unlit(ranked);
            reveal.lit = reveal.lit.min(back.len());
            for &task in &back[..reveal.lit] {
                partial.light(task);
            }
            reveal.shown = Some(partial);
            reveal.builds = self.chain_builds;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            reason = "back.len() is a chain length, far under 2^53, so f64 holds it exactly; ease(p) is 0 to 1, so the product is 0 to len, and the float-to-usize `as` saturates rather than wraps"
        )]
        let target = (timeline::slide::ease(p) * back.len() as f64) as usize;
        if let Some(partial) = &mut reveal.shown {
            for &task in &back[reveal.lit..target.max(reveal.lit)] {
                partial.light(task);
            }
        }
        reveal.lit = reveal.lit.max(target);
    }
}
