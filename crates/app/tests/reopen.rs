//! `Session::reopen` and `parse_selection`: opening a file replaces the schedule in place
//! (gs#87 PR 2). The wasm `App::open` is a thin wrapper over `reopen`.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::dates::Origin;
use app::edit::Edit;
use app::outline::OutlineError;
use app::{Loaded, Session, load_bytes, parse_selection};
use common::clock;
use engine::TaskId;
use import::ProjectSelection;
use timeline::{Theme, Timeline};
use xer::synth::{Spec, write};

fn loaded(spec: &Spec) -> Loaded {
    load_bytes(&write(spec), &ProjectSelection::All, &|| 0.0).expect("a generated XER should load")
}

fn session_of(loaded: Loaded, width: f64, height: f64, dpr: f64) -> Session {
    session_with(loaded, width, height, dpr, |_| {})
}

/// [`session_of`], with `tune` changing the fitted timeline before the session is built.
fn session_with(
    loaded: Loaded,
    width: f64,
    height: f64,
    dpr: f64,
    tune: impl FnOnce(&mut Timeline),
) -> Session {
    let mut timeline = app::fitted_timeline(loaded.order, &loaded.solution, width, height, dpr);
    tune(&mut timeline);
    Session::new(
        loaded.schedule,
        loaded.solution,
        timeline,
        loaded.meta,
        clock,
    )
    .with_outline(loaded.outline)
    .expect("a loaded outline matches its schedule")
}

fn rows(session: &Session) -> usize {
    session.timeline().order().len()
}

/// Leave the session with an undo step and a redo step on its history: two duration edits, then
/// one undo. Two different leaves, so the redo is a real change.
fn give_it_history(session: &mut Session) {
    let leaves: Vec<u32> = session
        .timeline()
        .order()
        .iter()
        .copied()
        .filter(|&task| !session.outline().unwrap().is_summary(TaskId(task)))
        .take(2)
        .collect();
    assert_eq!(leaves.len(), 2, "the schedule has two leaves to edit");
    for task in leaves {
        let minutes = session.schedule().tasks()[task as usize].duration + 480;
        let edit = Edit::SetDuration {
            task: TaskId(task),
            minutes,
        };
        session.edit(edit).expect("a longer duration applies");
    }
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(
        session.history_len(),
        (1, 1),
        "one undo step, one redo step"
    );
}

/// Press on a wide leaf bar and drag it, leaving the press open: a drag is in flight. A summary
/// that hides a candidate is expanded first, until one passes the row filter and shows.
fn start_a_drag(session: &mut Session) {
    let preorder: Vec<TaskId> = session.outline().expect("an outline").preorder().to_vec();
    let mut target = None;
    for candidate in preorder {
        let dates = session.solution().dates[candidate.0 as usize];
        // A task with actuals is applied at open and refuses a drag.
        let task = &session.schedule().tasks()[candidate.0 as usize];
        let has_actuals = task.actual_start.is_some() || task.actual_finish.is_some();
        if session.outline().unwrap().is_summary(candidate)
            || dates.ef - dates.es < 800
            || has_actuals
        {
            continue;
        }
        let mut ancestor = session.outline().unwrap().parent(candidate);
        while let Some(summary) = ancestor {
            if session.is_collapsed(summary) {
                session.toggle(summary);
            }
            ancestor = session.outline().unwrap().parent(summary);
        }
        if let Some(row) = session.timeline().row_of_task(candidate.0) {
            target = Some((row, candidate.0));
            break;
        }
    }
    let (row, task) = target.expect("a wide leaf shows through the row filter");
    let start = session.solution().dates[task as usize].es;
    common::zoom_to(session, 100.0, start as f64 - 50_000.0);
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
        "the press lands on the bar"
    );
    session.pointer_down(point.0, point.1, false);
    session.pointer_move(point.0 + 80.0, point.1);
    session.apply_drag();
}

#[test]
fn the_selection_text_is_none_all_or_a_project_id() {
    assert_eq!(parse_selection(None), Ok(ProjectSelection::Unspecified));
    assert_eq!(parse_selection(Some("all")), Ok(ProjectSelection::All));
    assert_eq!(
        parse_selection(Some("1001")),
        Ok(ProjectSelection::Only(vec![1001]))
    );
    assert_eq!(
        parse_selection(Some(" 7 ")),
        Ok(ProjectSelection::Only(vec![7]))
    );
    for bad in ["", "ALL", "-1", "1.5", "4294967296", "proj"] {
        let error = parse_selection(Some(bad)).expect_err("a bad selection is a message");
        assert!(error.contains(bad), "{error:?} names {bad:?}");
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test checks each thing a reopen keeps or resets"
)]
fn reopen_starts_a_clean_session_that_keeps_the_canvas_and_the_colours() {
    let first = loaded(&Spec::with_activities(1, 40));
    let mut old = session_with(first, 900.0, 150.0, 2.0, |timeline| {
        timeline.viewport.row_height = 24.0;
        timeline.viewport.axis_height = 44.0;
    });
    let dark = Theme::dark();
    old.set_theme(dark);
    old = old
        .with_data_date(Some(999))
        .with_origin(Origin::from_day(20_458 + 28));
    give_it_history(&mut old);
    old.collapse_to_level(1);
    old.set_critical_only(true);
    let task = old.timeline().order()[0];
    old.select(task);
    assert!(old.set_lookahead(0, 4));
    // A drag is the last thing set up: it refuses a collapse or a filter change while it lasts.
    start_a_drag(&mut old);
    old.scroll_by(30.0);
    let old_stamp = old.stamp();
    let old_rows = rows(&old);
    let old_tasks = old.schedule().tasks().len();
    assert!(old.selection().is_some());
    assert_ne!(old.row_filter(), app::RowFilter::Off);
    // Every kind of state the new session must not inherit is there to be inherited.
    assert!(
        old.can_undo() && old.can_redo(),
        "the old session has both histories"
    );
    assert!(old.drag_info().is_some(), "a drag is active");
    assert!(old.drag_started() && old.timeline().bar_drag().is_some());
    assert!(
        old.timeline().viewport.scroll_y > 0.0,
        "the old view is scrolled"
    );
    assert!(old.revision() > 0);
    assert!(old.lookahead().is_some() && old.chain_builds() > 0);

    let next = loaded(&Spec::with_activities(2, 90));
    let next_tasks = next.schedule.tasks().len();
    let next_preorder = next.outline.preorder().len();
    let next_date = Some(1_234_567);
    let fresh = old
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(next.outline),
            next_date,
            next.origin,
        )
        .expect("the outline matches its schedule");

    // The previous session was only read.
    assert_eq!(old.schedule().tasks().len(), old_tasks);
    assert_eq!(rows(&old), old_rows);
    assert_eq!(old.stamp(), old_stamp);
    assert_eq!(
        old.history_len(),
        (1, 1),
        "reopen did not touch the old history"
    );
    assert!(old.drag_info().is_some() && old.drag_started());

    // A clean session over the new schedule.
    assert_eq!(fresh.schedule().tasks().len(), next_tasks);
    assert_eq!(fresh.solution().dates.len(), next_tasks);
    assert_eq!(fresh.revision(), 0);
    assert_eq!(
        fresh.data_date(),
        next_date,
        "the new file's data date, not the old one's"
    );
    assert_eq!(fresh.origin(), next.origin, "the new file's origin");
    assert_ne!(fresh.origin(), old.origin());
    assert!(fresh.stamp() != old_stamp, "the table redraws on the stamp");
    assert!(fresh.lookahead().is_none());
    assert_eq!(fresh.chain_builds(), 0);
    assert!(fresh.selection().is_none());
    assert_eq!(fresh.row_filter(), app::RowFilter::Off);
    assert!(!fresh.can_undo() && !fresh.can_redo());
    assert_eq!(fresh.history_len(), (0, 0));
    assert!(fresh.drag_info().is_none());
    assert!(!fresh.drag_started() && fresh.timeline().bar_drag().is_none());
    assert_eq!(rows(&fresh), next_preorder, "every summary is expanded");
    assert_eq!(fresh.timeline().viewport.scroll_y, 0.0);

    // What belongs to the canvas stays.
    let (before, after) = (&old.timeline(), &fresh.timeline());
    assert_eq!(after.viewport.width, before.viewport.width);
    assert_eq!(after.viewport.height, before.viewport.height);
    assert_eq!(after.dpr, before.dpr);
    assert_eq!(after.viewport.row_height, 24.0);
    assert_eq!(after.viewport.axis_height, 44.0);
    assert_eq!(fresh.theme(), &dark);

    // The view is fitted to the new schedule's first screen: those rows' dates are on screen.
    let (dates, (from, to)) = (
        &fresh.solution().dates,
        fresh.timeline().viewport.visible_time(),
    );
    let shown = &fresh.timeline().order()[fresh.timeline().viewport.visible_rows()];
    let start = shown.iter().map(|&t| dates[t as usize].es).min().unwrap() as f64;
    let finish = shown.iter().map(|&t| dates[t as usize].ef).max().unwrap() as f64;
    assert!(
        from <= start,
        "{from} shows the first screen's start {start}"
    );
    assert!(
        to >= finish,
        "{to} shows the first screen's finish {finish}"
    );
}

#[test]
fn reopen_without_an_outline_keeps_a_flat_session() {
    let old = session_of(loaded(&Spec::with_activities(3, 20)), 800.0, 400.0, 1.0);
    let next = loaded(&Spec::with_activities(4, 30));
    let tasks = next.schedule.tasks().len();
    let fresh = old
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            None,
            None,
            next.origin,
        )
        .expect("a flat session needs no outline");
    assert!(fresh.outline().is_none());
    assert_eq!(rows(&fresh), tasks);
}

#[test]
fn reopen_takes_the_new_names_and_drops_the_old_search() {
    let mut old = session_of(loaded(&Spec::with_activities(3, 20)), 800.0, 400.0, 1.0);
    let query = old.task_label(0).to_string();
    assert!(!old.find(&query).is_empty(), "the old session finds a task");
    let next = loaded(&Spec::with_activities(4, 30));
    let names = next.meta.clone();
    let mut fresh = old
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(next.outline),
            None,
            next.origin,
        )
        .expect("the outline matches its schedule");
    assert_eq!(fresh.meta(), names);
    assert_eq!(fresh.find_go(0), None, "the old results are gone");
    assert!(old.find_go(0).is_some(), "the old session keeps its own");
}

#[test]
fn a_failed_reopen_leaves_the_previous_session_as_it_was() {
    let mut old = session_of(loaded(&Spec::with_activities(5, 40)), 900.0, 500.0, 1.0);
    old.collapse_to_level(1);
    let (tasks, rows_before, stamp) = (old.schedule().tasks().len(), rows(&old), old.stamp());

    // An outline for a different schedule: the wrong length.
    let next = loaded(&Spec::with_activities(6, 25));
    let other = loaded(&Spec::with_activities(7, 60));
    let error = old
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(other.outline),
            None,
            next.origin,
        )
        .expect_err("an outline of the wrong length is refused");
    assert_eq!(error, OutlineError::WrongLength);

    assert_eq!(old.schedule().tasks().len(), tasks);
    assert_eq!(rows(&old), rows_before);
    assert_eq!(old.stamp(), stamp);
    assert!(old.is_collapsed(old.outline().unwrap().roots()[0]));
}

#[test]
fn the_timeline_a_session_reopens_onto_is_the_one_create_builds() {
    let next = loaded(&Spec::with_activities(8, 30));
    let timeline = app::fitted_timeline(next.order.clone(), &next.solution, 700.0, 300.0, 1.5);
    let plain = Timeline::new(next.order, 700.0, 300.0, 1.5);
    assert_eq!(timeline.order(), plain.order());
    assert_eq!(timeline.viewport.width, 700.0);
    assert!(timeline.viewport.px_per_minute > plain.viewport.px_per_minute);
}

/// The XER files the page tests open (`packages/gantt/tests/open.spec.ts`), written by `xer::synth`. Their task
/// counts are constants in that spec; this test fails when a fixture and the spec disagree.
#[test]
fn the_page_fixtures_hold_the_task_counts_the_web_tests_expect() {
    let one = include_bytes!("../../../packages/gantt/tests/fixtures/one-project.xer");
    let two = include_bytes!("../../../packages/gantt/tests/fixtures/two-projects.xer");

    let opened =
        loaded_bytes(one, &ProjectSelection::Unspecified).expect("one project needs no choice");
    assert_eq!(opened.schedule.tasks().len(), 14);
    assert_eq!(
        opened.unsupported.len(),
        5,
        "three notes, the percent-complete column note and one for the four activities with an actual after the data date"
    );
    assert!(
        opened.data_date.is_some(),
        "the XER project states a data date"
    );
    assert!(opened.unsupported[4].starts_with("in-progress activities: 2 of 2"));

    let opened = loaded_bytes(two, &ProjectSelection::All).expect("two projects open together");
    assert_eq!(opened.schedule.tasks().len(), 20);
    assert!(
        opened.meta[0].name.contains("Café"),
        "{:?}",
        opened.meta[0].name
    );

    assert!(matches!(
        loaded_bytes(two, &ProjectSelection::Unspecified),
        Err(app::OpenError::ChooseProject(projects)) if projects.len() == 2
    ));
}

/// The page opens an MSPDI file and then an XER (`packages/gantt/tests/open.spec.ts`): the session shows the data
/// date each file states, and none for a file that states none, whatever the session before it showed.
#[test]
fn reopen_takes_the_new_files_data_date_or_its_lack_of_one() {
    let mspdi = include_bytes!("../../import/tests/fixtures/summary.xml");
    let xer = include_bytes!("../../../packages/gantt/tests/fixtures/one-project.xer");
    let reopen = |old: &Session, next: Loaded| -> Session {
        old.reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(next.outline),
            next.data_date,
            next.origin,
        )
        .expect("the outline matches its schedule")
    };
    let first = loaded_bytes(mspdi, &ProjectSelection::Unspecified).expect("the MSPDI file loads");
    assert_eq!(first.data_date, None, "an MSPDI file states no data date");
    let session = session_of(first, 900.0, 500.0, 1.0);

    let next = loaded_bytes(xer, &ProjectSelection::Unspecified).expect("the XER loads");
    let stated = next.data_date;
    assert!(stated.is_some(), "the XER project states a data date");
    let session = reopen(&session, next);
    assert_eq!(
        session.data_date(),
        stated,
        "the XER's data date replaces the MSPDI file's none"
    );

    let back = loaded_bytes(mspdi, &ProjectSelection::Unspecified).expect("the MSPDI file loads");
    let session = reopen(&session, back);
    assert_eq!(
        session.data_date(),
        None,
        "the MSPDI file's none replaces the XER's data date"
    );
}

fn loaded_bytes(bytes: &[u8], selection: &ProjectSelection) -> Result<Loaded, app::OpenError> {
    load_bytes(bytes, selection, &|| 0.0)
}
