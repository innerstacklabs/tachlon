//! The Moved row filter (gs#703): after an edit that moved other tasks, the table keeps the tasks
//! of the last move report and their summaries. (`filter.rs` is at its size limit.)

use crate::common;

use app::{Loaded, RowFilter, Session, edit::Edit, load_bytes};
use common::clock;
use engine::TaskId;
use import::ProjectSelection;
use std::collections::BTreeSet;
use timeline::Timeline;

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");
/// The task whose longer duration moves 8 others on this fixture (`tests/moved.rs`): A1-29, which
/// is row 16 now that activities list by start (it was row 12 in activity-ID text order).
const EDITED: TaskId = TaskId(16);
const LONGER: i64 = 4800;

fn ledger() -> Session {
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(LEDGER_XER, &ProjectSelection::Unspecified, &clock).expect("the file loads");
    Session::new(
        schedule,
        solution,
        Timeline::new(order, 1200.0, 600.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("a valid outline")
    .with_origin(origin)
}

fn set_duration(s: &mut Session, task: TaskId, minutes: i64) {
    s.edit(Edit::SetDuration { task, minutes })
        .expect("applies");
}

/// The tasks of the report: the moved ones and the edited one.
fn reported(s: &Session) -> BTreeSet<u32> {
    let r = s.move_report().expect("the edit moved others");
    r.was.iter().chain(&r.edited).map(|m| m.task.0).collect()
}

fn keeps(s: &Session, task: TaskId, members: &BTreeSet<u32>) -> bool {
    let outline = s.outline().expect("an outline");
    let children = outline.children(task);
    if children.is_empty() {
        members.contains(&task.0)
    } else {
        children.iter().any(|&c| keeps(s, c, members))
    }
}

/// The rows the filter keeps, by the oracle: preorder, no collapse.
fn oracle(s: &Session, members: &BTreeSet<u32>) -> Vec<u32> {
    let outline = s.outline().expect("an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&t| keeps(s, t, members))
        .map(|t| t.0)
        .collect()
}

fn all_rows(s: &Session) -> Vec<u32> {
    s.outline()
        .expect("an outline")
        .preorder()
        .iter()
        .map(|t| t.0)
        .collect()
}

#[test]
fn show_moved_keeps_the_tasks_of_the_last_report_and_their_summaries() {
    let mut s = ledger();
    assert!(!s.set_moved_only(true), "no report, no filter");
    set_duration(&mut s, EDITED, LONGER);
    let members = reported(&s);
    let full = all_rows(&s);
    assert_eq!(s.timeline().order(), full, "[all-rows-before]");
    assert!(s.set_moved_only(true));
    assert_eq!(s.row_filter(), RowFilter::Moved);
    assert_eq!(s.row_filter().name(), "moved");
    let rows = s.timeline().order().to_vec();
    assert_eq!(rows, oracle(&s, &members), "[oracle]");
    assert!(rows.len() < full.len(), "[some rows hidden]");
    assert!(members.iter().all(|t| rows.contains(t)), "[members kept]");
    assert!(s.set_moved_only(false));
    assert_eq!(s.timeline().order(), full, "[off restores]");
}

#[test]
fn a_new_report_moves_the_moved_filter_to_its_rows_and_a_change_with_none_ends_it() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    let first = reported(&s);
    assert!(s.set_moved_only(true));
    // A change that makes another report: the rows read that report, not the old one.
    // Another task's longer duration moves another set; find it on a session of its own, as a
    // change that makes no report would end the filter here.
    let other = (0..43)
        .filter(|&t| t != EDITED.0)
        .find(|&t| {
            let mut probe = ledger();
            set_duration(&mut probe, EDITED, LONGER);
            set_duration(&mut probe, TaskId(t), 14_400);
            probe.move_report().is_some() && reported(&probe) != first
        })
        .expect("some task moves a different set");
    set_duration(&mut s, TaskId(other), 14_400);
    let second = reported(&s);
    assert_eq!(s.row_filter(), RowFilter::Moved);
    assert_eq!(s.timeline().order(), oracle(&s, &second), "[second report]");
    assert_ne!(first, second, "[the reports differ]");
    // A change that makes none (a rename) ends the filter and restores the rows.
    s.edit(Edit::Rename {
        task: TaskId(3),
        name: "Renamed".to_string(),
    })
    .expect("renames");
    assert!(s.move_report().is_none());
    assert_eq!(s.row_filter(), RowFilter::Off, "[cleared ends it]");
    assert_eq!(s.timeline().order(), all_rows(&s), "[rows restored]");
}

#[test]
fn undo_with_the_moved_filter_on_keeps_the_undo_report_rows() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    assert!(s.set_moved_only(true));
    assert_eq!(s.undo(), Ok(true));
    let members = reported(&s);
    assert_eq!(s.row_filter(), RowFilter::Moved, "[undo keeps it on]");
    assert_eq!(s.timeline().order(), oracle(&s, &members), "[undo rows]");
    assert_eq!(s.redo(), Ok(true));
    let members = reported(&s);
    assert_eq!(s.timeline().order(), oracle(&s, &members), "[redo rows]");
}

/// Press the bar of `EDITED`, drag it right and apply the frame; the release is the caller's.
fn drag_edited(s: &mut Session) -> (f64, f64) {
    let row = s
        .timeline()
        .order()
        .iter()
        .position(|&t| t == EDITED.0)
        .expect("the edited task has a row");
    let start = s.solution().dates[EDITED.0 as usize].es;
    let view = &s.timeline().viewport;
    let (x, y) = (
        view.x_of(start as f64) + 2.0,
        view.row_top(row) + view.row_height / 2.0,
    );
    s.pointer_down(x, y, false);
    s.pointer_move(x + 120.0, y);
    s.apply_drag();
    (x, y)
}

#[test]
fn a_drag_defers_the_moved_filter_until_release() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    assert!(s.set_moved_only(true));
    let rows = s.timeline().order().to_vec();
    drag_edited(&mut s);
    assert!(!s.set_moved_only(false), "[press refuses a change]");
    assert_eq!(s.timeline().order(), rows, "[rows held during the drag]");
    s.pointer_up();
    let expected = match s.move_report() {
        Some(_) => oracle(&s, &reported(&s)),
        None => all_rows(&s),
    };
    assert_eq!(s.timeline().order(), expected, "[rows follow at release]");
}

#[test]
fn a_rename_ends_the_filter_and_a_later_drag_release_leaves_it_off() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    assert!(s.set_moved_only(true));
    s.edit(Edit::Rename {
        task: TaskId(3),
        name: "Renamed".to_string(),
    })
    .expect("renames");
    assert_eq!(s.row_filter(), RowFilter::Off, "[rename ends it]");
    drag_edited(&mut s);
    s.pointer_up();
    assert!(s.move_report().is_some(), "[the drag moved others]");
    assert_eq!(s.row_filter(), RowFilter::Off, "[release leaves it off]");
    assert_eq!(s.timeline().order(), all_rows(&s), "[all rows]");
}

/// Hold a proposal that lengthens `EDITED`, and the tasks it says it would move.
fn propose_longer(s: &mut Session) -> BTreeSet<u32> {
    let said: serde_json::Value =
        serde_json::from_str(&s.propose_duration(EDITED.0, 20)).expect("an answer");
    let moved = said["moved"].as_array().expect("a moved list");
    moved
        .iter()
        .map(|m| u32::try_from(m["task"].as_u64().expect("a task")).expect("a slot"))
        .collect()
}

#[test]
fn a_held_proposal_gives_show_moved_its_tasks_and_rejecting_it_ends_the_filter() {
    let mut s = ledger();
    let (full, solution) = (all_rows(&s), s.solution().clone());
    let members = propose_longer(&mut s);
    assert!(
        s.move_report().is_none() && members.len() > 1,
        "[held, no report]"
    );
    assert!(s.set_moved_only(true), "[a held proposal is enough]");
    let rows = s.timeline().order().to_vec();
    assert_eq!(rows, oracle(&s, &members), "[the proposal's rows]");
    assert!(rows.len() < full.len(), "[some rows hidden]");
    assert!(s.solution() == &solution, "[nothing applied]");
    assert!(s.reject_proposal());
    assert_eq!(s.row_filter(), RowFilter::Off, "[reject ends it]");
    assert_eq!(s.timeline().order(), full, "[rows restored]");
    assert!(!s.set_moved_only(true), "[nothing held still refuses]");
}

#[test]
fn accepting_a_proposal_leaves_the_filter_on_the_rows_of_the_report_it_made() {
    let mut s = ledger();
    propose_longer(&mut s);
    assert!(s.set_moved_only(true));
    assert!(s.accept_proposal().contains(r#""ok":true"#));
    let members = reported(&s);
    assert_eq!(s.row_filter(), RowFilter::Moved, "[accept keeps it on]");
    assert_eq!(
        s.timeline().order(),
        oracle(&s, &members),
        "[the report's rows]"
    );
}
