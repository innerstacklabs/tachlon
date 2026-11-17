//! Keep the place (tl#703): collapse, expand and Clear filters leave the row the reader was on at
//! the top of the table, or the summary that now holds it, not the same pixel offset into
//! different rows.

use crate::common;

use app::Session;
use common::outline_session;

/// The task on the row at the top of the table.
fn top_task(session: &Session) -> u32 {
    let row = session.timeline().viewport.visible_rows().start;
    session.timeline().order()[row]
}

/// `task` if it has a row, else its nearest ancestor that does.
fn visible_self_or_ancestor(session: &Session, task: u32) -> u32 {
    let outline = session.outline().expect("the session has an outline");
    let mut at = Some(engine::TaskId(task));
    while let Some(id) = at {
        if session.timeline().row_of_task(id.0).is_some() {
            return id.0;
        }
        at = outline.parent(id);
    }
    panic!("task {task} and its ancestors are all hidden");
}

#[test]
fn collapse_keeps_the_row_you_were_reading() {
    let (mut session, _) = outline_session(5_000);
    assert!(session.timeline().order().len() > 3_020);
    for row in [3_000, 3_007, 3_014] {
        session.expand_all();
        session.scroll_to_row(row);
        let reading = top_task(&session);

        assert!(session.collapse_to_level(1));

        assert!(
            session.timeline().row_of_task(reading).is_none(),
            "row {row}: the collapse hides the row that was read, or the test shows nothing"
        );
        assert_eq!(
            top_task(&session),
            visible_self_or_ancestor(&session, reading),
            "row {row}: the summary holding the row is at the top"
        );
    }
}

#[test]
fn expand_keeps_the_row_you_were_reading() {
    let (mut session, _) = outline_session(5_000);
    assert!(session.collapse_to_level(2));
    let rows = session.timeline().order().len();
    session.scroll_to_row(rows / 2);
    session.scroll_by(7.0);
    let reading = top_task(&session);
    let offset = session.timeline().viewport.scroll_y % session.timeline().viewport.row_height;

    assert!(session.expand_all());

    assert!(session.timeline().order().len() > rows + 100);
    assert_eq!(top_task(&session), reading, "the same row is on top");
    let viewport = &session.timeline().viewport;
    assert_eq!(viewport.scroll_y % viewport.row_height, offset);
}

#[test]
fn clear_filters_keeps_the_row_you_were_reading() {
    let (mut session, _) = outline_session(5_000);
    let start = session.default_lookahead_start() + 20 * engine::WEEK;
    assert!(session.set_lookahead(start, 8));
    let rows = session.timeline().order().len();
    assert!(rows > 100 && rows < 4_000, "rows {rows}");
    session.scroll_to_row(rows / 2);
    let reading = top_task(&session);

    assert!(session.clear_filters());

    assert!(session.timeline().order().len() > rows + 100);
    assert_eq!(top_task(&session), reading, "the same row is on top");
}
