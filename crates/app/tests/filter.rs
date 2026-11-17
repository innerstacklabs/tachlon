use crate::common;

use app::{RowFilter, Session, edit::Edit, outline::demo_layout};
use common::baseline::{
    kept_as_slipped, leaf_count, set as set_baseline, slipped_leaves, slipping_edit,
};
use common::{clock, outline_session};
use engine::TaskId;
use std::collections::{BTreeSet, HashSet};
use timeline::{HIDDEN_ROW, Timeline, Viewport};

const N: usize = 2_000;

fn session(n: usize) -> Session {
    outline_session(n).0
}

fn kept_by_filter(session: &Session, task: TaskId) -> bool {
    let outline = session.outline().expect("test session has an outline");
    let children = outline.children(task);
    children.is_empty() && session.solution().dates[task.0 as usize].float == 0
        || children
            .iter()
            .copied()
            .any(|child| kept_by_filter(session, child))
}

fn hidden_by_collapse(session: &Session, task: TaskId) -> bool {
    let outline = session.outline().expect("test session has an outline");
    let mut parent = outline.parent(task);
    while let Some(task) = parent {
        if session.is_collapsed(task) {
            return true;
        }
        parent = outline.parent(task);
    }
    false
}

fn visible_oracle(session: &Session) -> Vec<u32> {
    session
        .outline()
        .expect("test session has an outline")
        .preorder()
        .iter()
        .copied()
        .filter(|&task| !hidden_by_collapse(session, task) && kept_by_filter(session, task))
        .map(|task| task.0)
        .collect()
}

fn critical_leaves(session: &Session) -> BTreeSet<u32> {
    let outline = session.outline().expect("test session has an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| {
            !outline.is_summary(task) && session.solution().dates[task.0 as usize].float == 0
        })
        .map(|task| task.0)
        .collect()
}

fn summaries(session: &Session) -> Vec<TaskId> {
    let outline = session.outline().expect("test session has an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| outline.is_summary(task))
        .collect()
}

fn link_pairs(session: &Session) -> BTreeSet<(u32, u32)> {
    let rows = session.timeline().order().len();
    let mut view = Viewport::new(
        10_000.0,
        40.0 + rows as f64 * Viewport::new(1.0, 1.0, 1).row_height + 1.0,
        rows,
    );
    let span = (session.solution().project_finish as f64).max(120.0) * 1.04;
    view.px_per_minute = view.width / span;
    view.t0 = -0.02 * span;
    session
        .links()
        .visible(&view, &session.solution().dates)
        .iter()
        .map(|placed| (placed.link.pred, placed.link.succ))
        .collect()
}

#[test]
fn filter_shows_exactly_the_zero_float_leaves_with_their_summaries() {
    let mut session = session(N);
    let stamp = session.stamp();
    assert!(session.set_critical_only(true), "filter must turn on");
    assert_ne!(session.stamp(), stamp, "[filter-stamp]");
    assert_eq!(
        session.timeline().order(),
        visible_oracle(&session),
        "[filter-rows]"
    );

    let outline = session.outline().expect("test session has an outline");
    let critical = critical_leaves(&session);
    let visible: HashSet<_> = session.timeline().order().iter().copied().collect();
    let leaf_rows: Vec<_> = visible
        .iter()
        .copied()
        .filter(|&task| !outline.is_summary(TaskId(task)))
        .collect();
    assert_eq!(leaf_rows.len(), critical.len(), "[filter-leaf-count]");
    assert!(
        leaf_rows
            .iter()
            .all(|task| session.solution().dates[*task as usize].float == 0),
        "[filter-leaves]"
    );
    for summary in summaries(&session) {
        assert_eq!(
            visible.contains(&summary.0),
            kept_by_filter(&session, summary),
            "[filter-summary]"
        );
    }
    assert!(session.timeline().order().len() < N, "[filter-fewer-rows]");
    assert!(
        session.timeline().order().len() > critical.len(),
        "[filter-context]"
    );
}

#[test]
fn turning_the_filter_off_restores_rows_and_collapse_state() {
    let mut session = session(N);
    let sums = summaries(&session);
    let kept = sums
        .iter()
        .copied()
        .find(|&task| kept_by_filter(&session, task))
        .expect("demo has a summary with a critical leaf");
    let other = sums
        .iter()
        .copied()
        .find(|&task| task != kept)
        .expect("demo has two summaries");
    assert!(
        session.toggle(kept) && session.toggle(other),
        "two summaries must collapse"
    );
    let rows = session.timeline().order().to_vec();
    let flags: Vec<_> = sums
        .iter()
        .map(|&task| (task, session.is_collapsed(task)))
        .collect();
    assert!(
        session.set_critical_only(true) && session.set_critical_only(false),
        "filter must round trip"
    );
    assert_eq!(session.timeline().order(), rows, "[filter-restore]");
    assert!(
        flags
            .iter()
            .all(|&(task, flag)| session.is_collapsed(task) == flag),
        "[filter-collapse-flags]"
    );

    assert!(session.set_critical_only(true), "filter must turn on again");
    assert!(session.toggle(kept), "summary must toggle while filtered");
    let toggled = session.is_collapsed(kept);
    assert!(
        session.set_critical_only(false),
        "filter must turn off again"
    );
    assert_eq!(
        session.is_collapsed(kept),
        toggled,
        "[filter-toggle-persisted]"
    );
}

#[test]
fn collapse_and_filter_combine() {
    let mut session = session(N);
    assert!(session.set_critical_only(true), "filter must turn on");
    let summary = summaries(&session)
        .into_iter()
        .find(|&task| kept_by_filter(&session, task))
        .expect("demo has a kept summary");
    let descendants = session
        .outline()
        .expect("test session has an outline")
        .subtree(summary)[1..]
        .to_vec();
    assert!(session.toggle(summary), "kept summary must collapse");
    assert_eq!(
        session.timeline().order(),
        visible_oracle(&session),
        "[collapse-filter]"
    );
    let visible: HashSet<_> = session.timeline().order().iter().copied().collect();
    assert!(visible.contains(&summary.0), "[collapse-summary]");
    assert!(
        descendants.iter().all(|task| !visible.contains(&task.0)),
        "[collapse-descendants]"
    );
    assert!(session.toggle(summary), "kept summary must expand");
    assert_eq!(
        session.timeline().order(),
        visible_oracle(&session),
        "[expand-filter]"
    );
    assert!(
        descendants
            .iter()
            .any(|task| session.timeline().row_of_task(task.0).is_some()),
        "[expand-critical]"
    );
}

#[test]
fn an_edit_that_changes_criticality_moves_the_filtered_rows() {
    let (mut session, _) = outline_session(N);
    assert!(session.set_critical_only(true), "filter must turn on");
    let old_rows = session.timeline().order().to_vec();
    let old_critical = critical_leaves(&session);
    let mut chosen = None;
    'search: for task in old_critical.iter().copied() {
        let duration = session.schedule().tasks()[task as usize].duration;
        // Parallel work packages make one critical path: lengthening it only
        // moves the finish, so also try emptying the task, which lets a
        // parallel path take over.
        for minutes in [
            if duration == 0 {
                480
            } else {
                duration.saturating_mul(2)
            },
            0,
        ] {
            let edit = Edit::SetDuration {
                task: TaskId(task),
                minutes,
            };
            let inverse = session
                .apply(edit.clone())
                .expect("candidate duration applies");
            let changed = critical_leaves(&session) != old_critical;
            session.apply(inverse).expect("candidate duration restores");
            if changed {
                chosen = Some(edit);
                break 'search;
            }
        }
    }
    let edit = chosen.expect("one critical leaf changes criticality when lengthened or emptied");
    assert_eq!(
        session.timeline().order(),
        old_rows,
        "[edit-search-restore]"
    );
    let stamp = session.stamp();
    session.edit(edit).expect("recorded duration applies");
    assert_ne!(session.stamp(), stamp, "[edit-stamp]");
    let new_rows = session.timeline().order().to_vec();
    assert_eq!(new_rows, visible_oracle(&session), "[edit-oracle]");
    assert_ne!(new_rows, old_rows, "[edit-rows-change]");
    let stamp = session.stamp();
    assert_eq!(session.undo(), Ok(true), "undo must succeed");
    assert_ne!(session.stamp(), stamp, "[undo-stamp]");
    assert_eq!(session.timeline().order(), old_rows, "[undo-rows]");
    let stamp = session.stamp();
    assert_eq!(session.redo(), Ok(true), "redo must succeed");
    assert_ne!(session.stamp(), stamp, "[redo-stamp]");
    assert_eq!(session.timeline().order(), new_rows, "[redo-rows]");
}

/// Press the `nth` wide critical leaf bar and drag it `dx` pixels. False
/// when the drag leaves the filtered rows as they are; otherwise the
/// deferral checks run and it is true.
fn drag_defers_filter(nth: usize, dx: f64) -> bool {
    let mut session = session(N);
    assert!(
        session.set_critical_only(true),
        "filter must turn on before the press"
    );
    let (row, task) = {
        let outline = session.outline().expect("test session has an outline");
        session
            .timeline()
            .order()
            .iter()
            .copied()
            .enumerate()
            .filter(|&(_, task)| {
                let dates = session.solution().dates[task as usize];
                !outline.is_summary(TaskId(task)) && dates.float == 0 && dates.ef - dates.es >= 800
            })
            .nth(nth)
            .expect("filtered demo has a wide critical leaf bar")
    };
    let start = session.solution().dates[task as usize].es;
    common::zoom_to(&mut session, 100.0, start as f64 - 50_000.0);
    session.scroll_to_row(row);
    let view = &session.timeline().viewport;
    let point = (
        view.x_of(start as f64) + 2.0,
        view.row_top(row) + view.row_height / 2.0,
    );
    assert_eq!(
        session
            .timeline()
            .hit_test(&session.solution().dates, point.0, point.1),
        Some(task),
        "press must hit the critical bar"
    );
    session.pointer_down(point.0, point.1, false);
    assert!(
        !session.set_critical_only(false) && !session.set_critical_only(true),
        "[drag-refused]"
    );
    let rows = session.timeline().order().to_vec();
    session.pointer_move(point.0 + dx, point.1);
    session.apply_drag();
    if visible_oracle(&session) == rows {
        return false;
    }
    assert_eq!(session.timeline().order(), rows, "[drag-rows-deferred]");
    assert!(
        session.timeline().order().contains(&task),
        "[drag-task-kept]"
    );
    session.pointer_up();
    assert_eq!(
        session.timeline().order(),
        visible_oracle(&session),
        "[drag-release-oracle]"
    );
    true
}

#[test]
fn a_drag_defers_the_filter_until_release() {
    // With work packages in parallel there is one critical path: only a drag
    // that lets a parallel path take over changes the filtered rows, so look
    // for one.
    let found = (0..8).any(|nth| [80.0, -80.0].iter().any(|&dx| drag_defers_filter(nth, dx)));
    assert!(
        found,
        "[drag-changes-criticality] a drag must change the filtered rows, or deferral is untested"
    );
}

#[test]
fn c_walks_only_the_filtered_critical_leaves() {
    let mut session = session(N);
    assert!(session.set_critical_only(true), "filter must turn on");
    let expected: BTreeSet<_> = critical_leaves(&session)
        .into_iter()
        .filter(|task| session.timeline().row_of_task(*task).is_some())
        .collect();
    assert!(!expected.is_empty(), "filtered demo has critical leaves");
    let mut visited = BTreeSet::new();
    for _ in 0..expected.len() + 2 {
        let row = session
            .next_critical()
            .expect("a filtered critical leaf exists") as usize;
        let task = session.timeline().order()[row];
        assert!(expected.contains(&task), "[filter-c-task]");
        visited.insert(task);
    }
    assert_eq!(visited, expected, "[filter-c-wrap]");
}

#[test]
fn the_filter_refuses_without_an_outline() {
    let layout = demo_layout(N);
    let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
    let mut session = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    );
    let rows = session.timeline().order().to_vec();
    let stamp = session.stamp();
    assert!(
        !session.set_critical_only(true),
        "flat session must refuse the filter"
    );
    assert!(!session.critical_only(), "[flat-filter-flag]");
    assert_eq!(
        (session.timeline().order(), session.stamp()),
        (rows.as_slice(), stamp),
        "[flat-filter-state]"
    );
}

#[test]
fn links_with_a_hidden_end_are_absent_under_the_filter() {
    let mut session = session(N);
    assert!(session.set_critical_only(true), "filter must turn on");
    let visible: HashSet<_> = session.timeline().order().iter().copied().collect();
    let pairs = link_pairs(&session);
    assert!(
        pairs
            .iter()
            .all(|(a, b)| visible.contains(a) && visible.contains(b)),
        "[filter-link-ends]"
    );
    let expected = session
        .schedule()
        .dependencies()
        .iter()
        .filter(|d| visible.contains(&d.pred.0) && visible.contains(&d.succ.0))
        .map(|d| (d.pred.0, d.succ.0))
        .collect();
    assert_eq!(pairs, expected, "[filter-link-pairs]");
}

#[test]
fn selection_survives_the_filter_by_task_id() {
    let mut session = session(N);
    let outline = session.outline().expect("test session has an outline");
    let task = outline
        .preorder()
        .iter()
        .copied()
        .find(|&task| {
            !outline.is_summary(task) && session.solution().dates[task.0 as usize].float != 0
        })
        .expect("demo has a noncritical leaf");
    session.select(task.0);
    assert!(session.set_critical_only(true), "filter must turn on");
    let selected = session.selection().expect("selection survives filtering");
    assert_eq!(
        (selected.task, selected.row),
        (task.0, HIDDEN_ROW),
        "[filter-selection-hidden]"
    );
    assert!(session.set_critical_only(false), "filter must turn off");
    let row = session
        .timeline()
        .row_of_task(task.0)
        .expect("selected task has a restored row");
    assert_eq!(
        session
            .selection()
            .expect("selection survives restoring")
            .row as usize,
        row,
        "[filter-selection-row]"
    );
}

fn slipped_oracle(session: &Session) -> Vec<u32> {
    let slipped = slipped_leaves(session);
    let outline = session.outline().expect("test session has an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| {
            !hidden_by_collapse(session, task) && kept_as_slipped(session, task, &slipped)
        })
        .map(|task| task.0)
        .collect()
}

#[test]
fn slipped_keeps_the_slipped_leaves_and_their_summaries() {
    let (mut session, _) = outline_session(N);
    set_baseline(&mut session);
    let inserted = {
        let place = session.place_below(None);
        let name = "Added after the baseline".to_string();
        session.edit(Edit::Insert { place, name }).expect("inserts");
        TaskId(session.schedule().tasks().len() as u32 - 1)
    };
    let edit = slipping_edit(&mut session, &[]);
    session.edit(edit).expect("the slip applies");

    let stamp = session.stamp();
    assert!(session.set_slipped_only(true), "filter must turn on");
    assert_ne!(session.stamp(), stamp, "[slipped-stamp]");
    assert_eq!(session.timeline().order(), slipped_oracle(&session));

    let slipped = slipped_leaves(&session);
    assert!(!slipped.is_empty() && slipped.len() < leaf_count(&session));
    let visible: HashSet<_> = session.timeline().order().iter().copied().collect();
    assert!(
        !visible.contains(&inserted.0),
        "[slipped-added-task-has-no-entry]"
    );

    // A summary that slips on its own, with no slipped leaf under it, is not kept: the filter
    // keeps leaves and the summaries above them. The status line counts it, so its count is the
    // leaves and this summary.
    let lonely = summaries(&session)
        .into_iter()
        .find(|&task| !kept_as_slipped(&session, task, &slipped) && task != TaskId(0))
        .expect("a summary has no slipped leaf");
    session
        .edit(Edit::Move {
            task: lonely,
            snet: Some(session.solution().project_finish + 100_000),
        })
        .expect("a summary can be moved");
    let baseline = session.baseline().expect("a baseline is set");
    let then = baseline.task(lonely).expect("the summary was baselined");
    assert!(
        session.solution().dates[lonely.0 as usize].ef > then.ef,
        "[slipped-summary-case]"
    );
    assert!(
        !session.timeline().order().contains(&lonely.0),
        "[slipped-summary-alone-not-kept]"
    );
    assert_eq!(session.timeline().order(), slipped_oracle(&session));
    let counted = session.baseline_diff().expect("a baseline is set").slipped as usize;
    assert!(counted > slipped.len(), "[status-counts-summaries-too]");
}

#[test]
fn clearing_the_baseline_turns_the_slipped_filter_off() {
    let (mut session, _) = outline_session(N);
    let all_rows = session.timeline().order().to_vec();
    set_baseline(&mut session);
    let edit = slipping_edit(&mut session, &[]);
    session.edit(edit).expect("the slip applies");
    assert!(session.set_slipped_only(true));

    session
        .edit(Edit::ClearBaseline)
        .expect("the baseline clears");
    assert_eq!(session.row_filter(), RowFilter::Off, "[clear-filter]");
    assert_eq!(session.timeline().order(), all_rows, "[clear-rows]");

    // Undo brings the baseline back, not the filter.
    assert_eq!(session.undo(), Ok(true));
    assert!(session.baseline().is_some(), "[undo-baseline]");
    assert_eq!(session.row_filter(), RowFilter::Off, "[undo-filter]");
    assert_eq!(session.timeline().order(), all_rows, "[undo-rows]");

    // Undoing the baseline itself, with the filter on, turns the filter off too.
    assert!(session.set_slipped_only(true));
    assert_eq!(session.undo(), Ok(true), "the slip is taken back");
    assert_eq!(session.undo(), Ok(true), "the baseline is taken back");
    assert!(session.baseline().is_none(), "[undo-set-baseline]");
    assert_eq!(session.row_filter(), RowFilter::Off, "[undo-set-filter]");
    assert_eq!(session.timeline().order(), all_rows, "[undo-set-rows]");
}

#[test]
fn slipped_refuses_without_a_baseline() {
    let (mut session, _) = outline_session(N);
    let rows = session.timeline().order().to_vec();
    let stamp = session.stamp();
    assert!(!session.set_slipped_only(true), "no baseline, no filter");
    assert_eq!(
        (session.timeline().order(), session.stamp()),
        (rows.as_slice(), stamp),
        "[none-state]"
    );
}

#[test]
fn an_edit_moves_rows_into_and_out_of_the_slipped_filter() {
    let (mut session, _) = outline_session(N);
    set_baseline(&mut session);
    let first = slipping_edit(&mut session, &[]);
    let Edit::SetDuration {
        task: first_task,
        minutes: _,
    } = first
    else {
        unreachable!("slipping_edit gives a duration");
    };
    session.edit(first).expect("the first slip applies");
    assert!(session.set_slipped_only(true));
    let one = session.timeline().order().to_vec();
    assert_eq!(one, slipped_oracle(&session), "[one-oracle]");
    assert_ne!(one, [] as [u32; 0]);

    // A second leaf slips: rows come in.
    let second = slipping_edit(&mut session, &[first_task.0]);
    session.edit(second).expect("the second slip applies");
    let two = session.timeline().order().to_vec();
    assert_eq!(two, slipped_oracle(&session), "[two-oracle]");
    assert_ne!(two, one, "[rows-in]");

    // Undo takes them out again, redo brings them back.
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(session.timeline().order(), one, "[undo-rows]");
    assert_eq!(session.redo(), Ok(true));
    assert_eq!(session.timeline().order(), two, "[redo-rows]");
}
