//! The open draws in (gs#309): after a file opens, each bar of the rows in the window grows from
//! its start to its finish, staggered by start date, over `total_ms(rows)`. The slide's was-extent
//! has zero width, so at t = 0 a visible task has `ef == es` in `display_dates()` and at the end it
//! equals `solution().dates`. Only the window's rows are entered, whatever the schedule's size.
//! Reduced motion draws the final picture at once; a scroll, zoom or edit snaps it. Time is a fake
//! clock the test sets.

use app::{
    Session,
    dates::Origin,
    edit::Edit,
    outline::{Outline, demo_layout},
};
use std::cell::Cell;
use timeline::{Timeline, slide};

const TASKS: usize = 50_000;

thread_local! {
    /// The fake clock. Each test runs on its own thread, so each has its own time.
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    NOW.with(Cell::get)
}

fn at(ms: f64) {
    NOW.with(|now| now.set(ms));
}

fn session(n: usize) -> Session {
    let layout = demo_layout(n);
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        clock,
    )
    .with_outline(outline)
    .expect("the outline matches")
}

/// What `App::open` does with a loaded file: `reopen` the page's session, then mark the open.
fn opened(old: &Session, n: usize) -> Session {
    let layout = demo_layout(n);
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let mut next = old
        .reopen(
            layout.schedule,
            layout.solution,
            layout.order,
            layout.meta,
            Some(outline),
            None,
            Origin::from_day(20_458),
        )
        .expect("the outline matches");
    next.mark_open();
    next
}

/// The tasks on the rows in the window.
fn visible(s: &Session) -> Vec<u32> {
    let rows = s.timeline().viewport.visible_rows();
    s.timeline().order()[rows].to_vec()
}

/// The first visible task that takes time, so its bar has a width to grow.
fn long_task(s: &Session) -> u32 {
    *visible(s)
        .iter()
        .find(|&&t| {
            let d = &s.solution().dates[t as usize];
            d.ef > d.es
        })
        .expect("a visible task takes time")
}

#[test]
fn the_bars_start_empty_and_grow_to_the_solution() {
    at(0.0);
    let mut s = opened(&session(TASKS), TASKS);
    assert!(s.slide_active());
    let tasks = visible(&s);
    assert!(tasks.len() > 5);
    for &t in &tasks {
        let d = &s.display_dates()[t as usize];
        assert_eq!(d.ef, d.es, "task {t} starts as a bar of no width");
        assert_eq!(
            d.es,
            s.solution().dates[t as usize].es,
            "and starts where it starts"
        );
    }
    let total = slide::total_ms(tasks.len());
    assert_eq!(total, slide::SLIDE_MS + slide::STAGGER_MS, "360 ms in all");

    let long = long_task(&s);
    at(total / 2.0);
    s.advance_motion();
    let mid = &s.display_dates()[long as usize];
    let end = &s.solution().dates[long as usize];
    assert!(
        mid.ef >= mid.es && mid.ef <= end.ef,
        "a bar between nothing and whole"
    );

    at(total);
    s.advance_motion();
    assert!(!s.slide_active());
    assert_eq!(s.display_dates(), &s.solution().dates[..]);
}

#[test]
fn the_entries_are_the_rows_in_the_window_only() {
    at(0.0);
    let s = opened(&session(TASKS), TASKS);
    let rows = visible(&s);
    assert!(rows.len() < 100, "the window is a few rows of {TASKS}");
    let grown = s
        .display_dates()
        .iter()
        .zip(&s.solution().dates)
        .filter(|(shown, solved)| shown != solved)
        .count();
    assert!(
        grown <= rows.len(),
        "{grown} dates differ, the window holds {}",
        rows.len()
    );
    assert!(grown > 0);
    // Other rows hold the solution's dates.
    let off = s.timeline().order()[s.timeline().viewport.visible_rows().end + 10] as usize;
    assert_eq!(s.display_dates()[off], s.solution().dates[off]);
}

#[test]
fn reduced_motion_shows_the_final_dates_at_once() {
    at(0.0);
    let mut old = session(TASKS);
    old.set_reduced_motion(true);
    let s = opened(&old, TASKS);
    assert!(!s.slide_active());
    assert_eq!(s.display_dates(), &s.solution().dates[..]);
}

#[test]
fn a_scroll_a_zoom_or_an_edit_snaps_it() {
    at(0.0);
    let old = session(TASKS);
    let mut s = opened(&old, TASKS);
    assert!(s.slide_active());
    s.scroll_by(40.0);
    assert!(!s.slide_active(), "a scroll");
    assert_eq!(s.display_dates(), &s.solution().dates[..]);

    let mut s = opened(&old, TASKS);
    s.wheel(300.0, 100.0, 0.0, -120.0, true);
    assert!(!s.slide_active(), "a zoom");

    let mut s = opened(&old, TASKS);
    let task = long_task(&s);
    let before = s.solution().dates.clone();
    s.edit(Edit::SetDuration {
        task: engine::TaskId(task),
        minutes: 9_600,
    })
    .expect("applies");
    s.advance_motion();
    // The edit's own slide may run, but only for the tasks it moved: the open's is gone.
    for t in visible(&s) {
        let i = t as usize;
        if before[i] == s.solution().dates[i] {
            assert_eq!(
                s.display_dates()[i],
                s.solution().dates[i],
                "an edit, task {t}"
            );
        }
    }
}

#[test]
fn a_table_sync_that_moves_nothing_keeps_it() {
    at(0.0);
    let mut s = opened(&session(TASKS), TASKS);
    s.scroll_by(0.0);
    assert!(s.slide_active());
}

/// A summary row is drawn from its rolled-up span, not its own dates, so it is not entered: it takes
/// no place in the stagger. The leaves' delays spread over the leaves alone, so a leaf in the middle
/// has not started before its own delay (with the summary rows counted the spread is finer and it has).
#[test]
fn a_summary_row_is_not_entered() {
    at(0.0);
    let mut s = opened(&session(TASKS), TASKS);
    let outline = s.outline().expect("an outline");
    let (mut leaves, summaries): (Vec<u32>, Vec<u32>) = visible(&s)
        .into_iter()
        .partition(|&t| !outline.is_summary(engine::TaskId(t)));
    assert!(!summaries.is_empty(), "the first screen has a summary row");
    leaves.sort_by_key(|&t| (s.solution().dates[t as usize].es, t));
    let (count, rank) = (leaves.len(), leaves.len() / 2);
    let leaf = leaves[rank];
    let d = s.solution().dates[leaf as usize];
    assert!(d.ef - d.es >= 600, "the leaf has a width to grow");
    let with_summaries = slide::delay(rank, count + summaries.len());
    let own = slide::delay(rank, count);
    assert!(with_summaries + 2.0 < own, "the two spreads are told apart");
    at(f64::midpoint(own, with_summaries));
    s.advance_motion();
    let shown = s.display_dates()[leaf as usize];
    assert_eq!(shown.ef, shown.es, "the leaf waits for its own delay");
}
