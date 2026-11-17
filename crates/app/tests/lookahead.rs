//! The lookahead filter (gs#96): a start and a length in weeks that keeps the activities whose
//! early dates intersect the window, with the summaries that hold them as context.
//!
//! The rule is half-open on both edges: the window is `[start, start + weeks * WEEK)` and an
//! activity occupies `[es, ef)`. An activity that ends exactly at the window's start is out; one
//! that starts on the window's last day is in; one that starts exactly at its end is out.

use crate::common;

use app::{
    Session,
    dates::Origin,
    edit::Edit,
    lookahead::MAX_WEEKS,
    meta::TaskMeta,
    outline::{Outline, demo_layout},
};
use common::clock;
use engine::{Calendar, DAY, Dependency, LinkType, Schedule, Task, TaskId, WEEK};
use timeline::Timeline;

/// The window the tests use: the second week of the project, Monday 2026-01-12 to Sunday 18.
const START: i64 = WEEK;
const END: i64 = 2 * WEEK;

const S1: u32 = 0;
const A: u32 = 1;
const D: u32 = 2;
const S2: u32 = 3;
const B: u32 = 4;
const C: u32 = 5;
const E: u32 = 6;
const F: u32 = 7;
const M1: u32 = 8;
const H: u32 = 9;
const G: u32 = 10;
const Q: u32 = 11;

/// One fixture task: `(id, name, parent, snet, duration)`.
type Row = (u32, &'static str, Option<u32>, Option<i64>, i64);

/// What each fixture task is, to read the failures: `(id, name, parent, snet, duration)`.
/// A 24/7 calendar makes every date exact: an activity runs `[snet, snet + duration)`.
#[rustfmt::skip]
const TASKS: [Row; 12] = [
    (S1, "Outside", None, None, 0),
    (A, "A ends on the window start", Some(S1), Some(START - DAY), DAY),
    (D, "D starts on the window end", Some(S1), Some(END), 60),
    (S2, "Around the window", None, None, 0),
    (B, "B ends a minute into the window", Some(S2), Some(START - DAY), DAY + 1),
    (C, "C starts on the last day", Some(S2), Some(END - DAY), 60),
    (E, "E milestone on the window start", Some(S2), Some(START), 0),
    (F, "F spans the window", Some(S2), Some(0), 3 * WEEK),
    (M1, "M1 drives H from the last day", Some(S2), Some(END - DAY), DAY),
    (H, "H after the window", Some(S2), None, 2 * WEEK),
    (G, "G long before", Some(S2), Some(0), DAY),
    (Q, "Q milestone on the window end", Some(S2), Some(END), 0),
];

fn fixture() -> (Session, Vec<TaskMeta>) {
    let day = vec![(0, 1_440)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7 calendar");
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar);
    let mut meta = Vec::new();
    let mut wbs = Vec::new();
    for (id, name, parent, snet, duration) in TASKS {
        let task = schedule
            .add_task(Task {
                duration,
                calendar,
                snet,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
        assert_eq!(task, TaskId(id), "ids follow the table");
        meta.push(TaskMeta {
            name: name.to_string(),
            parent: parent.map(TaskId),
            outline_level: if parent.is_some() { 2 } else { 1 },
            code: None,
            ..TaskMeta::default()
        });
        wbs.push(task);
    }
    schedule
        .add_dependency(Dependency {
            pred: TaskId(M1),
            succ: TaskId(H),
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("M1 drives H");
    let solution = schedule.clone().compute().expect("the fixture schedules");
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    // Tasks Q and the summaries sit anywhere in the order; the outline decides the rows.
    let timeline = Timeline::new(wbs.iter().map(|t| t.0).collect(), 1_000.0, 400.0, 1.0);
    let session = Session::new(schedule, solution, timeline, meta.clone(), clock)
        .with_outline(outline)
        .expect("the outline matches the schedule");
    (session, meta)
}

fn rows(session: &Session) -> Vec<u32> {
    session.timeline().order().to_vec()
}

#[test]
fn the_fixture_has_the_dates_the_edges_need() {
    let (session, _) = fixture();
    let dates = |task: u32| {
        let d = &session.solution().dates[task as usize];
        (d.es, d.ef)
    };
    assert_eq!(dates(A), (START - DAY, START), "A ends on the window start");
    assert_eq!(dates(B), (START - DAY, START + 1));
    assert_eq!(
        dates(C),
        (END - DAY, END - DAY + 60),
        "C starts on the last day"
    );
    assert_eq!(dates(D), (END, END + 60), "D starts on the window end");
    assert_eq!(dates(E), (START, START));
    assert_eq!(dates(Q), (END, END));
    assert_eq!(dates(M1), (END - DAY, END));
    assert_eq!(dates(H), (END, END + 2 * WEEK));
    assert_eq!(dates(F), (0, 3 * WEEK));
    assert!(session.solution().dates[M1 as usize].is_critical());
    assert!(session.solution().dates[H as usize].is_critical());
    assert!(!session.solution().dates[F as usize].is_critical());
}

#[test]
fn the_window_keeps_exactly_the_activities_that_intersect_it() {
    let (mut session, _) = fixture();
    assert_eq!(session.lookahead(), None);
    let all = rows(&session);
    assert_eq!(all.len(), TASKS.len());

    assert!(session.set_lookahead(START, 1));
    // In: B (ends a minute into the window), C and M1 (start on the last day), E (a milestone on
    // the start), F (spans it). Out: A (ends on the start), D and Q (start on the end), G, H.
    assert_eq!(rows(&session), [S2, B, C, E, F, M1]);
    assert_eq!(
        session.lookahead().map(|w| (w.start, w.weeks)),
        Some((START, 1))
    );

    // The window is its length: one more week brings D, Q and H in, and keeps A out.
    assert!(session.set_lookahead(START, 2));
    assert_eq!(rows(&session), [S1, D, S2, B, C, E, F, M1, H, Q]);

    // Moving the window a minute later drops E (a milestone on the old start) and B (it now ends
    // on the start), and picks up D, Q and H, which start on the old end.
    assert!(session.set_lookahead(START + 1, 1));
    assert_eq!(rows(&session), [S1, D, S2, C, F, M1, H, Q]);
    // A minute earlier picks up A (it ends a minute into the window) and drops D, Q and H.
    assert!(session.set_lookahead(START - 1, 1));
    assert_eq!(rows(&session), [S1, A, S2, B, C, E, F, M1]);

    assert!(session.clear_lookahead());
    assert_eq!(rows(&session), all);
    assert_eq!(session.lookahead(), None);
}

#[test]
fn a_summary_shows_as_context_only_while_a_leaf_inside_it_does() {
    let (mut session, _) = fixture();
    assert!(session.set_lookahead(START, 1));
    assert!(
        !rows(&session).contains(&S1),
        "every child of S1 is outside the window, so S1 is hidden"
    );
    assert!(rows(&session).contains(&S2));
    // A collapsed summary stays and hides its children, as without the window.
    assert!(session.toggle(TaskId(S2)));
    assert_eq!(rows(&session), [S2]);
    assert!(session.toggle(TaskId(S2)));
    assert_eq!(rows(&session), [S2, B, C, E, F, M1]);
    // Widening the window brings S1 back with the leaves that now intersect it.
    assert!(session.set_lookahead(START, 2));
    assert!(rows(&session).starts_with(&[S1, D, S2]));
}

#[test]
fn the_window_combines_with_critical_only_and_longest_path_by_and() {
    let (mut session, _) = fixture();
    // Critical alone: M1 and H. The window alone: B, C, E, F, M1. Both: M1.
    assert!(session.set_critical_only(true));
    assert_eq!(rows(&session), [S2, M1, H]);
    assert!(session.set_lookahead(START, 1));
    assert_eq!(rows(&session), [S2, M1], "critical in the window");
    assert!(
        session.critical_only(),
        "setting the window leaves the filter on"
    );
    assert!(session.set_critical_only(false));
    assert_eq!(rows(&session), [S2, B, C, E, F, M1], "the window stays");
    assert!(session.set_longest_path(true));
    assert_eq!(rows(&session), [S2, M1], "longest path in the window");
    assert!(session.set_longest_path(false));
    assert!(session.clear_lookahead());
    assert_eq!(rows(&session).len(), TASKS.len());

    // Collapse still applies to the combination.
    assert!(session.set_critical_only(true));
    assert!(session.set_lookahead(START, 1));
    assert!(session.toggle(TaskId(S2)));
    assert_eq!(rows(&session), [S2]);
}

#[test]
fn collapse_to_level_one_keeps_the_top_summaries_that_hold_a_kept_leaf() {
    let (mut session, _) = fixture();
    assert!(session.set_lookahead(START, 2));
    assert!(session.collapse_to_level(1));
    assert_eq!(rows(&session), [S1, S2], "D keeps S1, B to Q keep S2");
    // Critical only drops D, so S1 holds no kept leaf and goes.
    assert!(session.set_critical_only(true));
    assert_eq!(rows(&session), [S2]);
    assert!(session.expand_all());
    assert_eq!(rows(&session), [S2, M1, H]);
}

#[test]
fn the_status_reports_the_window_or_the_default_a_reader_gets() {
    let facts = app::RenderFacts::default();
    let window = |session: &Session| {
        let status: serde_json::Value =
            serde_json::from_str(&session.status_json(&facts)).expect("status is JSON");
        status["lookahead"].clone()
    };
    let (mut session, _) = fixture();
    assert_eq!(
        window(&session),
        serde_json::json!({"on": false, "start": "2026-01-05", "weeks": 3}),
        "no window and no data date: the origin Monday for 3 weeks"
    );
    assert!(session.set_lookahead(START, 2));
    assert_eq!(
        window(&session),
        serde_json::json!({"on": true, "start": "2026-01-12", "weeks": 2})
    );
    assert!(!session.set_lookahead(START, 0));
    assert_eq!(session.parse_lookahead_day("soon"), None);
    assert_eq!(
        window(&session),
        serde_json::json!({"on": true, "start": "2026-01-12", "weeks": 2}),
        "a refused window leaves the last one"
    );
    assert!(session.clear_lookahead());
    let session = session.with_data_date(Some(2 * WEEK + 8 * 60));
    assert_eq!(
        window(&session),
        serde_json::json!({"on": false, "start": "2026-01-19", "weeks": 3}),
        "off again, the default is the data date's day"
    );
}

#[test]
fn an_edit_that_moves_a_task_into_the_window_moves_the_rows() {
    let (mut session, _) = fixture();
    assert!(session.set_lookahead(START, 1));
    assert_eq!(rows(&session), [S2, B, C, E, F, M1]);
    let stamp = session.stamp();
    session
        .edit(Edit::Move {
            task: TaskId(G),
            snet: Some(START + DAY),
        })
        .expect("G moves");
    assert_ne!(session.stamp(), stamp);
    assert_eq!(rows(&session), [S2, B, C, E, F, M1, G]);
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(
        rows(&session),
        [S2, B, C, E, F, M1],
        "undo takes G out again"
    );
}

#[test]
fn find_skips_a_task_the_window_hides() {
    let (mut session, _) = fixture();
    let mut both = session.find("starts on the");
    both.sort_unstable();
    assert_eq!(both, vec![D.min(C), D.max(C)]);
    assert!(session.set_lookahead(START, 1));
    assert_eq!(session.find("starts on the"), vec![C]);
    assert!(!session.go_to(D), "D is hidden by the window");
    assert!(session.go_to(C));
}

#[test]
fn the_setters_refuse_what_they_cannot_do() {
    let (mut session, _) = fixture();
    assert!(!session.set_lookahead(START, 0));
    assert!(!session.set_lookahead(START, MAX_WEEKS + 1));
    assert_eq!(session.lookahead(), None, "a refused call changes nothing");
    assert!(session.set_lookahead(START, MAX_WEEKS));

    // No outline, no rows to filter.
    let layout = demo_layout(50);
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let mut flat = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    );
    assert!(!flat.set_lookahead(START, 3));
    assert!(!flat.clear_lookahead());
    assert_eq!(flat.lookahead(), None);
}

#[test]
fn the_window_defaults_to_the_data_date_and_the_origin_without_one() {
    let (session, _) = fixture();
    assert_eq!(
        session.default_lookahead_start(),
        0,
        "no data date: the origin"
    );
    let at = 2 * WEEK + 8 * 60;
    let session = session.with_data_date(Some(at));
    assert_eq!(session.default_lookahead_start(), at);
    assert_eq!(session.lookahead_start_text(), "2026-01-19");
    assert_eq!(session.parse_lookahead_day("2026-01-19"), Some(2 * WEEK));
}

#[test]
fn opening_another_file_drops_the_window() {
    let (mut session, _) = fixture();
    assert!(session.set_lookahead(START, 1));
    let layout = demo_layout(50);
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("outline");
    let reopened = session
        .reopen(
            layout.schedule,
            layout.solution,
            layout.order,
            layout.meta,
            Some(outline),
            None,
            Origin::DEMO,
        )
        .expect("reopens");
    assert_eq!(reopened.lookahead(), None);
    assert_eq!(reopened.timeline().order().len(), 50);
}
