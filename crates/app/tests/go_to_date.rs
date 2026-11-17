//! `Session::go_to_date` (gs#391): pan a date to a fifth in from the left and scroll the first row
//! whose bar starts on or after it to the top. The oracle below is written from the rule: it walks
//! the table's rows in order and reads each bar's start from the summary span or the solved dates.
//!
//! The demo's origin is Monday 2026-01-05, so a date is a whole number of weeks from it.

use crate::common;

use app::Session;
use common::outline_session;
use engine::WEEK;

/// 2026-01-12, 2026-02-02 and 2026-03-02, each a Monday, with their instants.
const DAYS: [(&str, i64); 3] = [
    ("2026-01-12", WEEK),
    ("2026-02-02", 4 * WEEK),
    ("2026-03-02", 8 * WEEK),
];

/// Where the bar on `task`'s row starts: its summary span, else its early start.
fn bar_start(session: &Session, task: u32) -> i64 {
    session
        .timeline()
        .summaries()
        .get(task)
        .map_or(session.solution().dates[task as usize].es, |span| span.0)
}

/// The first row, in table order, whose bar starts on or after `at`.
fn first_row_from(session: &Session, at: i64) -> Option<usize> {
    session
        .timeline()
        .order()
        .iter()
        .position(|&task| bar_start(session, task) >= at)
}

#[test]
fn a_date_goes_a_fifth_in_with_the_first_row_starting_from_it_on_top() {
    for (text, at) in DAYS {
        let (mut session, _) = outline_session(2_000);
        common::zoom_to(&mut session, 100.0, 0.0);
        let scale = session.timeline().viewport.px_per_minute;
        let row = first_row_from(&session, at).expect("a bar starts on or after the day");
        assert!(
            row > 0,
            "{text}: rows above the target make the scroll mean something"
        );

        assert!(session.go_to_date(text), "{text}");

        let viewport = &session.timeline().viewport;
        assert!(
            (viewport.x_of(at as f64) - viewport.width * 0.2).abs() < 1e-6,
            "{text}"
        );
        assert_eq!(viewport.px_per_minute, scale, "{text}: the zoom stays");
        assert!(
            row as f64 * viewport.row_height <= viewport.max_scroll(),
            "{text}"
        );
        assert_eq!(
            viewport.scroll_y,
            row as f64 * viewport.row_height,
            "{text}"
        );
        let top = viewport.visible_rows().start;
        assert_eq!(top, row, "{text}");
        let task = session.timeline().order()[top];
        assert!(
            bar_start(&session, task) >= at,
            "{text}: the top row starts on or after"
        );
        for &above in &session.timeline().order()[..top] {
            assert!(
                bar_start(&session, above) < at,
                "{text}: no earlier row qualifies"
            );
        }
    }
}

#[test]
fn a_collapsed_summary_row_is_read_by_its_span() {
    let (mut session, _) = outline_session(2_000);
    let at = 8 * WEEK;
    let summary = session
        .outline()
        .expect("test session has an outline")
        .preorder()
        .iter()
        .copied()
        .find(|&task| {
            session
                .outline()
                .is_some_and(|outline| outline.is_summary(task))
                && session
                    .timeline()
                    .summaries()
                    .get(task.0)
                    .is_some_and(|span| span.0 >= at)
        })
        .expect("a summary starts on or after the day");
    assert!(session.toggle(summary));

    assert!(session.go_to_date("2026-03-02"));

    let row = first_row_from(&session, at).expect("a row starts on or after the day");
    assert_eq!(session.timeline().viewport.visible_rows().start, row);
    let first = session.timeline().order()[row];
    assert!(bar_start(&session, first) >= at);
}

#[test]
fn a_row_filter_leaves_hidden_rows_out_of_the_scan() {
    let (mut session, _) = outline_session(2_000);
    let all = session.timeline().order().len();
    assert!(session.set_critical_only(true));
    assert!(
        session.timeline().order().len() < all,
        "the filter hides rows"
    );
    let at = 4 * WEEK;
    let row = first_row_from(&session, at).expect("a shown row starts on or after the day");

    assert!(session.go_to_date("2026-02-02"));

    assert_eq!(session.timeline().viewport.visible_rows().start, row);
}

#[test]
fn a_day_after_every_bar_pans_and_leaves_the_rows_where_they_are() {
    let (mut session, _) = outline_session(2_000);
    common::zoom_to(&mut session, 100.0, 0.0);
    session.scroll_to_row(10);
    let scroll = session.timeline().viewport.scroll_y;
    // 2045-06-04 is 7,090 days after the origin.
    let at = 7_090 * engine::DAY;
    assert!(first_row_from(&session, at).is_none());

    assert!(session.go_to_date("2045-06-04"));

    let viewport = &session.timeline().viewport;
    assert_eq!(viewport.scroll_y, scroll);
    assert!((viewport.x_of(at as f64) - viewport.width * 0.2).abs() < 1e-6);
}

#[test]
fn text_that_is_not_a_date_changes_nothing() {
    let (mut session, _) = outline_session(2_000);
    common::zoom_to(&mut session, 100.0, 0.0);
    session.scroll_to_row(10);
    let before = (session.timeline().viewport.clone(), session.stamp());

    for text in [
        "",
        "tomorrow",
        "2026-1-12",
        "2026-02-30",
        "2026-13-01",
        "2026-01-12 ",
        "26-01-12",
        "2026/01/12",
        "２０２６-01-12",
    ] {
        assert!(!session.go_to_date(text), "{text:?}");
        assert_eq!(
            (session.timeline().viewport.clone(), session.stamp()),
            before,
            "{text:?}"
        );
    }
}

#[test]
fn a_date_during_a_bar_press_changes_nothing() {
    let (mut session, _) = outline_session(2_000);
    let (row, task) = {
        let outline = session.outline().expect("test session has an outline");
        session
            .timeline()
            .order()
            .iter()
            .copied()
            .enumerate()
            .find(|&(_, task)| {
                !outline.is_summary(engine::TaskId(task))
                    && session.solution().dates[task as usize].ef
                        - session.solution().dates[task as usize].es
                        >= 800
            })
            .expect("demo has a wide leaf bar")
    };
    let start = session.solution().dates[task as usize].es;
    common::zoom_to(&mut session, 100.0, start as f64 - 50_000.0);
    session.scroll_to_row(row);
    let point = (
        session.timeline().viewport.x_of(start as f64) + 2.0,
        session.timeline().viewport.row_top(row) + session.timeline().viewport.row_height / 2.0,
    );
    session.pointer_down(point.0, point.1, false);
    assert!(session.timeline().bar_drag().is_some());
    let before = (session.timeline().viewport.clone(), session.stamp());

    assert!(!session.go_to_date("2026-02-02"));
    assert_eq!(
        (session.timeline().viewport.clone(), session.stamp()),
        before
    );
    session.pointer_up();
}

#[test]
fn a_date_change_leaves_the_schedule_and_the_selection_alone() {
    let (mut session, _) = outline_session(2_000);
    let leaf = session.timeline().order()[3];
    assert!(session.go_to(leaf));
    let dates = session.solution().dates.clone();
    let selected = session.selection().map(|selection| selection.task);
    assert_eq!(selected, Some(leaf), "a task is selected before the jump");

    assert!(session.go_to_date("2026-02-02"));

    assert_eq!(session.solution().dates, dates);
    assert_eq!(
        session.selection().map(|selection| selection.task),
        selected
    );
}

/// 24/7 tasks with exact starts, in this order: 30 that start at the origin, then one a minute
/// before 2026-01-12, one exactly at its 00:00, one an hour after it, then 30 that start a week on.
fn exact_starts() -> Session {
    let day = vec![(0, 1_440)];
    let calendar =
        engine::Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7 calendar");
    let mut schedule = engine::Schedule::new();
    let calendar = schedule.add_calendar(calendar);
    let starts: Vec<i64> = [
        vec![0; 30],
        vec![WEEK - 1, WEEK, WEEK + 60],
        vec![2 * WEEK; 30],
    ]
    .concat();
    for &snet in &starts {
        schedule
            .add_task(engine::Task {
                duration: 60,
                calendar,
                snet: Some(snet),
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    let solution = schedule.clone().compute().expect("the fixture schedules");
    let meta = (0..starts.len())
        .map(|index| app::meta::TaskMeta {
            name: format!("Task {index}"),
            parent: None,
            outline_level: 1,
            code: None,
            ..app::meta::TaskMeta::default()
        })
        .collect();
    let order = (0..starts.len() as u32).collect();
    Session::new(
        schedule,
        solution,
        timeline::Timeline::new(order, 1_000.0, 400.0, 1.0),
        meta,
        common::clock,
    )
}

#[test]
fn a_bar_starting_exactly_at_midnight_counts_and_one_a_minute_before_does_not() {
    let mut session = exact_starts();
    assert_eq!(
        bar_start(&session, 31),
        WEEK,
        "the fixture has a bar at 00:00 on the day"
    );

    assert!(session.go_to_date("2026-01-12"));

    let viewport = &session.timeline().viewport;
    assert_eq!(
        viewport.scroll_y,
        31.0 * viewport.row_height,
        "the 00:00 bar is on top"
    );
}
