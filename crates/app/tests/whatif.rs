//! A what-if mark (gantt#703): begin, read, keep or discard.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Session, WhatIfError, edit::Edit};
use engine::{DAY, TaskId};

/// Lengthen the task that finishes last by a working day.
fn lengthen(session: &mut Session) {
    let finish = session.solution().project_finish;
    let last = session.solution().dates.iter().position(|d| d.ef == finish);
    let task = TaskId(last.unwrap() as u32);
    let minutes = session.schedule().tasks()[task.0 as usize].duration + 480;
    session.edit(Edit::SetDuration { task, minutes }).unwrap();
}

#[test]
fn discard_leaves_a_schedule_equal_to_a_full_compute_of_the_start() {
    let (mut session, _) = common::demo_session(60);
    let (start, stamp) = (session.solution().clone(), session.stamp());
    session.begin_whatif().unwrap();
    assert_ne!(session.stamp(), stamp, "begin moves the stamp");
    lengthen(&mut session);
    let summary = session.whatif_summary().unwrap();
    let days = |t: i64| t.div_euclid(DAY);
    let moved = days(session.solution().project_finish) - days(start.project_finish);
    assert_eq!((summary.edits, summary.finish_days), (1, moved));
    assert!(moved > 0 && summary.finish_now != summary.finish_was);
    let stamp = session.stamp();
    session.discard_whatif().unwrap();
    assert_ne!(session.stamp(), stamp, "discard moves the stamp");
    let full = session.schedule().clone().compute().unwrap();
    assert!(session.solution() == &full && session.solution() == &start);
    assert!(session.whatif_summary().is_none() && session.can_redo());
}

#[test]
fn an_undo_past_the_mark_ends_it() {
    let (mut session, _) = common::demo_session(60);
    lengthen(&mut session);
    session.begin_whatif().unwrap();
    session.undo().unwrap();
    assert!(session.whatif_summary().is_none());
}

#[test]
fn a_history_that_dropped_the_mark_cannot_be_discarded() {
    let mut session = common::demo_session(60).0.with_history_cap(2);
    session.begin_whatif().unwrap();
    assert_eq!(session.begin_whatif(), Err(WhatIfError::AlreadyOn));
    (0..3).for_each(|_| lengthen(&mut session));
    let solution = session.solution().clone();
    assert_eq!(session.discard_whatif(), Err(WhatIfError::Lost));
    assert!(session.solution() == &solution && session.history_len() == (2, 0));
    assert_eq!(session.keep_whatif(), Ok(()));
}

#[test]
fn begin_during_a_bar_press_is_refused() {
    let (mut session, _, _, point) = common::setup();
    session.pointer_down(point.0, point.1, false);
    assert_eq!(session.begin_whatif(), Err(WhatIfError::Busy));
}
