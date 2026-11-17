//! Range selection (gs#423, story S2 of #300): `Session::extend_selection`, `select_row_range`,
//! `extend_to` and the range they keep, through the native `Session` over the demo outline.

use crate::common;

use app::{RenderFacts, Session, edit::EditError};
use common::collapse::{N, session, summaries};
use engine::TaskId;

/// A summary that is not the first row and has more than one task in its subtree, with the task
/// on the row above it and the task on the row after its whole subtree.
struct Around {
    summary: TaskId,
    before: u32,
    first_child: u32,
    last_child: u32,
    after: u32,
}

fn around(session: &Session) -> Around {
    let outline = session.outline().expect("test session has an outline");
    let preorder = outline.preorder();
    summaries(session)
        .into_iter()
        .find_map(|summary| {
            let at = preorder.iter().position(|&t| t == summary)?;
            let end = at + outline.subtree(summary).len();
            let subtree = outline.subtree(summary).len();
            let after = *preorder.get(end)?;
            (at > 0 && subtree > 2).then(|| Around {
                summary,
                before: preorder[at - 1].0,
                first_child: preorder[at + 1].0,
                last_child: preorder[end - 1].0,
                after: after.0,
            })
        })
        .expect("the demo outline has a summary with a row before and after it")
}

fn selected(session: &Session) -> Option<u32> {
    session.selection().map(|selection| selection.task)
}

fn status(session: &Session) -> serde_json::Value {
    serde_json::from_str(&session.status_json(&RenderFacts::default())).expect("status is JSON")
}

#[test]
fn extending_down_across_a_collapsed_summary_selects_only_visible_rows() {
    let mut session = session(N);
    let a = around(&session);
    assert!(session.toggle(a.summary), "the summary collapses");
    session.select(a.before);

    assert_eq!(session.extend_selection(1), Some(a.summary.0));
    assert_eq!(session.extend_selection(1), Some(a.after));

    assert_eq!(
        session.range_tasks(),
        vec![a.before, a.summary.0, a.after],
        "the hidden children are not in the range"
    );
    let rows = session.range_rows().expect("a range");
    assert_eq!(rows.last - rows.first, 2, "three visible rows");
    assert_eq!(
        selected(&session),
        Some(a.before),
        "the anchor stays selected"
    );
    assert_eq!(session.range(), Some((a.before, a.after)));
}

#[test]
fn extending_up_across_a_collapsed_summary_selects_only_visible_rows() {
    let mut session = session(N);
    let a = around(&session);
    assert!(session.toggle(a.summary), "the summary collapses");
    session.select(a.after);

    assert_eq!(session.extend_selection(-1), Some(a.summary.0));
    assert_eq!(session.extend_selection(-1), Some(a.before));

    assert_eq!(session.range_tasks(), vec![a.before, a.summary.0, a.after]);
    let rows = session.range_rows().expect("a range");
    assert_eq!(rows.extent, rows.first, "the extent is the upper end");
}

#[test]
fn an_expanded_summary_is_walked_into() {
    let mut session = session(N);
    let a = around(&session);
    session.select(a.before);

    session.extend_selection(1);
    session.extend_selection(1);

    assert_eq!(
        session.range_tasks(),
        vec![a.before, a.summary.0, a.first_child]
    );
}

#[test]
fn the_extent_stops_at_the_first_and_last_row() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    let (first, last) = (order[0], *order.last().expect("rows"));

    session.select(last);
    assert_eq!(
        session.extend_selection(5),
        Some(last),
        "down at the end stays"
    );
    assert_eq!(session.range_tasks(), vec![last]);

    session.select(first);
    assert_eq!(
        session.extend_selection(-1),
        Some(first),
        "up at the top stays"
    );
    assert_eq!(session.extend_selection(3), Some(order[3]));
    assert_eq!(
        session.extend_selection(-100_000),
        Some(first),
        "a step past the top clamps"
    );
    assert_eq!(session.range_tasks(), vec![first]);
}

#[test]
fn stepping_the_extent_back_past_the_anchor_puts_the_range_above_it() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    session.select(order[3]);
    for _ in 0..2 {
        session.extend_selection(1);
    }
    assert_eq!(status(&session)["range"], serde_json::json!([3, 5, 5]));

    for _ in 0..4 {
        session.extend_selection(-1);
    }

    assert_eq!(
        status(&session)["range"],
        serde_json::json!([1, 3, 1]),
        "four steps up from row 5 pass the anchor: rows 1 to 3, the extent on 1"
    );
    assert_eq!(
        selected(&session),
        Some(order[3]),
        "the anchor stays selected"
    );
}

#[test]
fn extending_from_nothing_selects_the_first_row_as_a_range() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    assert_eq!(selected(&session), None);

    assert_eq!(session.extend_selection(1), Some(order[0]));

    assert_eq!(selected(&session), Some(order[0]));
    assert_eq!(session.range_tasks(), vec![order[0]]);
}

#[test]
fn shift_space_makes_the_selected_row_a_range_and_keeps_a_larger_one() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    assert!(!session.select_row_range(), "nothing is selected");
    assert_eq!(session.range(), None);

    session.select(order[4]);
    assert!(session.select_row_range());
    assert_eq!(session.range(), Some((order[4], order[4])));
    assert_eq!(session.range_tasks(), vec![order[4]]);

    session.extend_selection(2);
    assert!(session.select_row_range());
    assert_eq!(
        session.range_tasks(),
        order[4..=6].to_vec(),
        "a range that is there stays"
    );
}

#[test]
fn extend_to_sets_the_extent_and_refuses_a_row_that_is_not_there() {
    let mut session = session(N);
    let a = around(&session);
    let order = session.timeline().order().to_vec();
    session.select(order[2]);

    assert!(session.extend_to(order[5]));
    assert_eq!(session.range_tasks(), order[2..=5].to_vec());
    assert!(session.extend_to(order[0]), "above the anchor");
    assert_eq!(session.range_tasks(), order[0..=2].to_vec());
    assert_eq!(selected(&session), Some(order[2]));

    assert!(session.toggle(a.summary), "the summary collapses");
    let kept = session.range_tasks();
    assert!(
        !session.extend_to(a.first_child),
        "a hidden child has no row"
    );
    assert!(!session.extend_to(u32::MAX), "an unknown task");
    assert_eq!(session.range_tasks(), kept, "a refusal changes nothing");
}

#[test]
fn extend_to_with_nothing_selected_selects_that_row() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();

    assert!(session.extend_to(order[3]));

    assert_eq!(selected(&session), Some(order[3]));
    assert_eq!(session.range_tasks(), vec![order[3]]);
}

#[test]
fn collapsing_under_a_range_keeps_its_anchor_and_extent_valid() {
    let mut session = session(N);
    let a = around(&session);
    session.select(a.before);
    assert!(session.extend_to(a.last_child));
    let open = session.range_tasks();
    assert!(open.contains(&a.first_child) && open.contains(&a.last_child));

    assert!(
        session.toggle(a.summary),
        "the summary collapses under the range"
    );

    assert_eq!(
        session.range(),
        Some((a.before, a.last_child)),
        "the anchor and the extent are the same tasks"
    );
    assert_eq!(
        session.range_tasks(),
        vec![a.before, a.summary.0],
        "the hidden extent stands on its summary's row"
    );
    let summary_row = session.timeline().row_of_task(a.summary.0).expect("row") as u32;
    assert_eq!(session.range_rows().expect("a range").extent, summary_row);

    assert_eq!(
        session.extend_selection(1),
        Some(a.after),
        "the next step is the row after the summary"
    );
    assert!(session.toggle(a.summary), "the summary expands");
    assert_eq!(session.range(), Some((a.before, a.after)));
    let from = session.timeline().row_of_task(a.before).expect("row");
    let to = session.timeline().row_of_task(a.after).expect("row");
    assert_eq!(
        session.range_tasks(),
        session.timeline().order()[from..=to].to_vec(),
        "expanded again, the children are back in the range"
    );
    assert!(session.range_tasks().contains(&a.first_child));
}

#[test]
fn collapsing_the_anchor_under_its_summary_keeps_the_range() {
    let mut session = session(N);
    let a = around(&session);
    session.select(a.first_child);
    assert!(session.extend_to(a.after));

    assert!(
        session.toggle(a.summary),
        "the summary collapses over the anchor"
    );

    assert_eq!(selected(&session), Some(a.first_child));
    assert_eq!(session.range(), Some((a.first_child, a.after)));
    assert_eq!(session.range_tasks(), vec![a.summary.0, a.after]);
}

#[test]
fn a_click_after_a_range_collapses_it_to_one_row() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    session.select(order[2]);
    assert!(session.extend_to(order[6]));
    assert_eq!(session.range_tasks().len(), 5);

    assert!(session.select_task(order[4]));

    assert_eq!(session.range(), None, "the range is gone");
    assert_eq!(session.range_tasks(), [] as [u32; 0]);
    assert_eq!(
        selected(&session),
        Some(order[4]),
        "the clicked row is the selection"
    );
}

#[test]
fn clicking_the_anchor_itself_also_ends_the_range() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    session.select(order[2]);
    assert!(session.extend_to(order[6]));

    assert!(session.select_task(order[2]));

    assert_eq!(session.range(), None);
}

#[test]
fn an_arrow_or_escape_after_a_range_ends_it() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    session.select(order[2]);
    assert!(session.extend_to(order[6]));

    assert_eq!(session.move_selection(1), Some(order[3]));
    assert_eq!(session.range(), None, "an arrow moves one row");

    assert!(session.extend_to(order[6]));
    session.clear_selection();
    assert_eq!(session.range(), None, "Escape clears it with the selection");
    session.select(order[2]);
    assert_eq!(session.range(), None, "and it does not come back");
}

#[test]
fn a_filter_never_leaves_a_range_on_a_row_that_is_not_there() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    assert!(session.set_critical_only(true), "the filter applies");
    let shown = session.timeline().order().to_vec();
    assert!(session.set_critical_only(false));
    assert!(shown.len() < order.len(), "the filter hides rows");

    // An extent the filter leaves no row for, not even its ancestor's: the first one found.
    let mut dropped = false;
    for &extent in order.iter().filter(|task| !shown.contains(task)).take(300) {
        session.select(shown[0]);
        assert!(session.extend_to(extent));
        assert!(session.set_critical_only(true));
        session.refresh_selection();
        for task in session.range_tasks() {
            assert!(
                shown.contains(&task),
                "only rows the filter shows are in a range"
            );
        }
        dropped = session.range().is_none();
        assert!(session.set_critical_only(false));
        session.refresh_selection();
        if dropped {
            assert_eq!(
                session.range(),
                None,
                "the range does not come back with its row"
            );
            break;
        }
    }
    assert!(
        dropped,
        "the demo outline has a task the filter leaves no row for"
    );
}

#[test]
fn undoing_the_extent_away_leaves_no_range_and_a_redo_does_not_bring_it_back() {
    // A redo selects the task it puts back, which ends a range on its own; the undo is the case.
    let mut session = session(N);
    let anchor = around(&session).first_child;
    session.select(anchor);
    let inserted = session.insert_below_selection().expect("insert").0;
    session.select(anchor);
    assert!(session.extend_to(inserted));
    assert_eq!(session.range(), Some((anchor, inserted)));

    assert!(session.undo().expect("undo"), "the insert is undone");
    assert_eq!(
        selected(&session),
        Some(anchor),
        "the anchor is still selected"
    );
    assert_eq!(session.range(), None, "the extent is deleted");
    session.refresh_selection();
    assert!(session.redo().expect("redo"), "the insert is done again");
    session.refresh_selection();

    assert_eq!(
        session.range(),
        None,
        "a task that comes back is not a range"
    );
}

#[test]
fn the_status_carries_the_range_rows_or_null() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    assert_eq!(status(&session)["range"], serde_json::Value::Null);

    session.select(order[5]);
    assert_eq!(
        status(&session)["range"],
        serde_json::Value::Null,
        "a selection is not a range"
    );
    assert!(session.extend_to(order[3]));
    assert_eq!(status(&session)["range"], serde_json::json!([3, 5, 3]));

    assert!(session.extend_to(order[8]));
    assert_eq!(status(&session)["range"], serde_json::json!([5, 8, 8]));
}

#[test]
fn the_timeline_scene_tints_the_range_rows_with_the_selection_band() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    let theme = *session.theme();
    let tops = |session: &Session| -> Vec<f32> {
        session
            .scene()
            .rects
            .iter()
            .filter(|r| r.color == theme.sel && r.pos[0] == 0.0)
            .map(|r| r.pos[1])
            .collect()
    };
    let row_top = |session: &Session, row: usize| session.timeline().viewport.row_top(row) as f32;

    session.select(order[5]);
    assert_eq!(tops(&session), vec![row_top(&session, 5)], "one row");

    assert!(session.extend_to(order[8]));
    assert_eq!(
        tops(&session),
        (5..=8).map(|r| row_top(&session, r)).collect::<Vec<_>>(),
        "the range's four rows"
    );

    // Every row, on a 2,000-row schedule: the scene holds the rows in view, not the range.
    assert!(session.extend_to(*order.last().expect("rows")));
    let in_view = session.timeline().viewport.visible_rows();
    assert!(in_view.len() < N / 10);
    assert_eq!(
        tops(&session).len(),
        (in_view.start.max(5)..in_view.end).count(),
        "the rows in view from the anchor down"
    );

    // A selection with no range goes back to one row.
    session.select(order[5]);
    assert_eq!(tops(&session), vec![row_top(&session, 5)]);
}

#[test]
fn select_all_rows_makes_the_first_to_last_visible_row_a_range() {
    let mut session = session(N);
    let all = session.timeline().order().len() as u32;
    assert!(session.set_critical_only(true), "the filter applies");
    let shown = session.timeline().order().to_vec();
    assert!(
        (shown.len() as u32) < all,
        "the filter hides rows, so the range is not the whole outline"
    );

    assert!(session.select_all_rows());

    let rows = session.range_rows().expect("a range");
    assert_eq!((rows.first, rows.last), (0, shown.len() as u32 - 1));
    assert_eq!(session.range_tasks(), shown, "exactly the visible rows");
    assert_eq!(session.range(), Some((shown[0], *shown.last().unwrap())));
    assert_eq!(
        selected(&session),
        Some(shown[0]),
        "the first row is the anchor"
    );
    let text = session.range_text().expect("text");
    assert_eq!(
        text.lines().count(),
        shown.len() + 1,
        "a header and a line per row"
    );
}

#[test]
fn select_all_rows_replaces_the_range_there_was() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    session.select(order[3]);
    assert!(session.extend_to(order[5]));

    assert!(session.select_all_rows());

    assert_eq!(session.range(), Some((order[0], *order.last().unwrap())));
}

#[test]
fn delete_refuses_a_range_of_several_rows_and_deletes_nothing() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    session.select(order[3]);
    assert!(session.extend_to(order[5]));

    assert_eq!(session.delete_selected(), Err(EditError::SeveralRows));

    assert_eq!(session.timeline().order(), order, "no row went");
    assert_eq!(
        session.range(),
        Some((order[3], order[5])),
        "the range stays"
    );
}
