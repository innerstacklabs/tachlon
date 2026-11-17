//! A task carries a real start constraint only when its start-no-earlier-than is strictly above
//! the floor its import gave it, and a saved document keeps the floors (gs#703 K2).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::collapse::{schedule_with_tasks, task_meta};
use app::{
    Session,
    document::{Extras, read, write},
    edit::Edit,
    fitted_timeline, load_bytes,
};
use engine::{DAY, Instant, Limit, Pin, TaskId};
use import::ProjectSelection;
use timeline::{Timeline, scene::StartConstraints};

/// A project starting 2026-01-07 08:00. Task 0 has no constraint, task 1 one a day after the start,
/// task 2 one a week before it, which the import overrides.
const FILE: &str = concat!(
    "<Project xmlns=\"http://schemas.microsoft.com/project\"><StartDate>2026-01-07T08:00:00</StartDate><Tasks>",
    "<Task><UID>1</UID><Name>A</Name><Duration>PT8H0M0S</Duration></Task>",
    "<Task><UID>2</UID><Name>B</Name><Duration>PT8H0M0S</Duration><ConstraintType>4</ConstraintType><ConstraintDate>2026-01-08T08:00:00</ConstraintDate></Task>",
    "<Task><UID>3</UID><Name>C</Name><Duration>PT8H0M0S</Duration><ConstraintType>4</ConstraintType><ConstraintDate>2025-12-31T08:00:00</ConstraintDate></Task>",
    "</Tasks></Project>",
);

const EXTRAS: Extras = Extras {
    name: "k2",
    notes: &[],
    source: None,
};

fn session_of(loaded: app::Loaded) -> Session {
    let timeline = fitted_timeline(loaded.order, &loaded.solution, 1000.0, 400.0, 1.0);
    Session::new(
        loaded.schedule,
        loaded.solution,
        timeline,
        loaded.meta,
        || 0.0,
    )
    .with_outline(loaded.outline)
    .expect("the outline matches")
}

fn opened() -> Session {
    let loaded = load_bytes(FILE.as_bytes(), &ProjectSelection::Unspecified, &|| 0.0);
    session_of(loaded.unwrap())
}

fn constraints(session: &Session) -> Vec<Option<Instant>> {
    (0..3)
        .map(|i| session.start_constraint(TaskId(i)))
        .collect()
}

fn move_to(session: &mut Session, snet: Instant) {
    let edit = Edit::Move {
        task: TaskId(0),
        snet: Some(snet),
    };
    session.apply(edit).unwrap();
}

#[test]
fn an_imported_task_at_its_floor_has_no_constraint() {
    let session = opened();
    assert!(
        session.schedule().tasks().iter().all(|t| t.snet.is_some()),
        "every task is floored"
    );
    let found = constraints(&session);
    assert_eq!(
        (found[0], found[2]),
        (None, None),
        "the floor is not a constraint"
    );
    assert!(
        found[1].is_some(),
        "a start a day after the project start is one"
    );
}

#[test]
fn a_task_dragged_past_its_floor_has_one() {
    let mut session = opened();
    let floor = session.schedule().tasks()[0].snet.unwrap();
    move_to(&mut session, floor + 480);
    assert_eq!(session.start_constraint(TaskId(0)), Some(floor + 480));
    move_to(&mut session, floor);
    assert_eq!(
        session.start_constraint(TaskId(0)),
        None,
        "back at the floor"
    );
}

#[test]
fn the_card_text_names_a_real_constraint_and_not_the_floor() {
    let session = opened();
    assert_eq!(session.task_constraint_text(0), "", "at its floor");
    assert_eq!(
        session.task_constraint_text(2),
        "",
        "a date before the floor"
    );
    assert_eq!(session.task_constraint_text(1), "2026-01-08 08:00");
}

#[test]
fn a_saved_document_reopens_with_the_same_constraints() {
    let mut session = opened();
    let floor = session.schedule().tasks()[0].snet.unwrap();
    // Below its floor, so the earliest `snet` is no longer the floor of the others.
    move_to(&mut session, floor - 480);
    let text = write(&session, &EXTRAS);
    let reopened = session_of(read(&text, &|| 0.0).unwrap().loaded);
    assert_eq!(constraints(&reopened), constraints(&session));
    assert_eq!(constraints(&reopened).iter().flatten().count(), 1);
    assert_eq!(
        write(&reopened, &EXTRAS),
        text,
        "the floors save as they were"
    );
}

/// The caps the scene draws: one `text` rect on the left edge of each held bar.
fn caps(session: &Session) -> usize {
    let text = session.theme().text;
    session
        .scene()
        .rects
        .iter()
        .filter(|r| r.color == text)
        .count()
}

#[test]
fn the_scene_caps_a_real_constraint_until_the_task_starts() {
    let mut session = opened();
    assert_eq!(caps(&session), 1, "B holds one; A and C hold the floor");
    let floor = session.schedule().tasks()[0].snet.unwrap();
    move_to(&mut session, floor + 480);
    assert_eq!(caps(&session), 2, "a drag puts a cap on A");
    move_to(&mut session, floor);
    assert_eq!(caps(&session), 1, "a drag back takes it off");
    let started = Edit::SetProgress {
        task: TaskId(1),
        actual_start: session.schedule().tasks()[1].snet,
        actual_finish: None,
        remaining: Some(480),
    };
    session.apply(started).unwrap();
    assert_eq!(caps(&session), 0, "a started task is not held by it");
}

#[test]
fn a_started_task_reports_no_constraint() {
    let mut session = opened();
    assert!(session.start_constraint(TaskId(1)).is_some(), "B holds one");
    let started = Edit::SetProgress {
        task: TaskId(1),
        actual_start: session.schedule().tasks()[1].snet,
        actual_finish: None,
        remaining: Some(480),
    };
    session.apply(started).unwrap();
    assert_eq!(
        session.start_constraint(TaskId(1)),
        None,
        "it no longer binds"
    );
    assert_eq!(
        session.task_constraint_text(1),
        "",
        "so the card names none"
    );
}

#[test]
fn a_floor_is_written_only_when_it_differs_from_the_fallback() {
    let mut session = opened();
    let saved = |session: &Session| -> Vec<Option<serde_json::Value>> {
        let value: serde_json::Value = serde_json::from_str(&write(session, &EXTRAS)).unwrap();
        value["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|task| task.get("floor").cloned())
            .collect()
    };
    // Every task is at the earliest `snet` of the file, the fallback `read` gives a missing key.
    assert_eq!(
        saved(&session),
        [None, None, None],
        "no floor key at the fallback"
    );
    let read_back = |session: &Session| read(write(session, &EXTRAS), &|| 0.0).unwrap().loaded;
    assert_eq!(
        constraints(&session_of(read_back(&session))),
        constraints(&session)
    );

    // Below its floor, so the fallback drops under the floor of every task: each writes it back.
    let floor = session.schedule().tasks()[0].snet.unwrap();
    move_to(&mut session, floor - 480);
    let above = serde_json::json!(floor);
    assert_eq!(
        saved(&session),
        [Some(above.clone()), Some(above.clone()), Some(above)],
        "a floor above the fallback is written"
    );
    assert_eq!(
        constraints(&session_of(read_back(&session))),
        constraints(&session)
    );
}

#[test]
fn a_late_start_or_a_finish_limit_marks_its_edge_until_the_task_passes_it() {
    let on = |snlt, fnet, fnlt, pin| Limit {
        snlt,
        fnet,
        fnlt,
        pin,
        ..Limit::default()
    };
    let (d, none) = (Some(DAY), on(None, None, None, None));
    let limits = [
        none,
        on(d, None, None, None),
        on(None, None, None, Some(Pin::Start(DAY))),
        on(None, d, None, None),
        on(None, None, d, None),
        on(None, None, None, Some(Pin::Finish(DAY))),
        on(d, None, None, None),
        on(None, None, d, None),
        on(None, None, d, None),
    ];
    let mut schedule = schedule_with_tasks(limits.len());
    for (i, limit) in (0..).zip(limits) {
        schedule.set_limit(TaskId(i), limit).unwrap();
    }
    // Tasks 6 and 7 have started and task 8 has finished: the start limit is ignored, then all.
    for i in [6, 7] {
        schedule.set_actual_start(TaskId(i), Some(0)).unwrap();
    }
    schedule.set_actual_finish(TaskId(8), Some(480)).unwrap();
    let solution = schedule.compute().unwrap();
    let timeline = Timeline::new((0..9).collect(), 800.0, 400.0, 1.0);
    let meta = limits.iter().map(|_| task_meta(None, 1)).collect();
    let session = Session::new(schedule, solution, timeline, meta, || 0.0);
    let marks: Vec<_> = (0..9)
        .map(|task| (session.holds_start(task), session.holds_finish(task)))
        .collect();
    let (s, f, n) = ((true, false), (false, true), (false, false));
    assert_eq!(marks, [n, s, s, f, f, f, n, f, n]);
    assert_eq!(
        session.start_constraint(TaskId(1)),
        None,
        "not a start-no-earlier-than"
    );
}
