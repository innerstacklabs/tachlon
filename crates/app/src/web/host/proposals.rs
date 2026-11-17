//! The host API's proposals: changes held apart from the schedule until accepted or rejected.

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Propose that `task` starts no earlier than `start` (`YYYY-MM-DD` or `YYYY-MM-DD HH:MM`), held apart from the
    /// schedule and drawn as ghosts. Answers like `what_moves`, with every held change, or
    /// `{"ok":false,"reason":r,"message":m}`. The schedule and its history do not change.
    pub fn propose_move(&mut self, task: u32, start: &str) -> String {
        self.session.propose_move(task, start)
    }

    /// Propose that `task` takes `days` working days, held and answered as `propose_move` does.
    pub fn propose_duration(&mut self, task: u32, days: u32) -> String {
        self.session.propose_duration(task, days)
    }

    /// Propose `text` for `field` (`actual_start`, `actual_finish`, `remaining` or `percent`) of `task`'s progress, held and answered as `propose_move` does.
    pub fn propose_progress(&mut self, task: u32, field: &str, text: &str) -> String {
        self.session.propose_progress(task, field, text)
    }

    /// Accept every held change as one undo step: `{"ok":true,"changes":n}`, or `{"ok":false,"reason":r,"message":m}`.
    pub fn accept_proposal(&mut self) -> String {
        self.session.accept_proposal()
    }

    /// Drop every held change; true when there were any.
    pub fn reject_proposal(&mut self) -> bool {
        self.session.reject_proposal()
    }

    /// Accept the held change of `kind` (`"move"`, `"duration"` or `"progress"`) to `task` as one undo step, answered as `accept_proposal` does
    /// (`noProposal` when it is not held). The others stay held, solved again.
    pub fn accept_proposed(&mut self, kind: &str, task: u32) -> String {
        self.session.accept_proposed(kind, task)
    }

    /// Drop the held change of `kind` to `task`, leaving the others held and solved again; true when it was held. Records no undo step.
    pub fn reject_proposed(&mut self, kind: &str, task: u32) -> bool {
        self.session.reject_proposed(kind, task)
    }

    /// Propose the link from `pred` to `succ` as `text` writes it (`FS`, `SS+2d`), new or changing the live one; held and answered as `propose_move` does (`loop`, `alreadyLinked`, `badLinkText` and `other` are refusals too).
    pub fn propose_link(&mut self, pred: u32, succ: u32, text: &str) -> String {
        self.session.propose_link(pred, succ, text)
    }

    /// Propose that the link from `pred` to `succ` is removed, as `propose_link` does; `noLink` when there is none.
    pub fn propose_unlink(&mut self, pred: u32, succ: u32) -> String {
        self.session.propose_unlink(pred, succ)
    }

    /// Accept the held change to the link from `pred` to `succ` as one undo step, answered as `accept_proposed` does.
    pub fn accept_proposed_link(&mut self, pred: u32, succ: u32) -> String {
        self.session.accept_proposed_link(pred, succ)
    }

    /// Drop the held change to the link from `pred` to `succ`; true when it was held. As `reject_proposed`.
    pub fn reject_proposed_link(&mut self, pred: u32, succ: u32) -> bool {
        self.session.reject_proposed_link(pred, succ)
    }
}
