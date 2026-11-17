//! Undo and redo of a link edit put back exactly what history recorded (gs#345): an import can
//! hold two links from A to B, and removing one must undo even though its twin is live.

use crate::common;

use app::{Session, edit::Edit, edit::EditError, meta::TaskMeta, outline::Outline};
use common::clock;
use engine::{Calendar, DepId, Dependency, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

/// Two hour-long leaves A and B with two identical finish-to-start links A -> B, added directly
/// as an import does.
fn duplicated() -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
    let calendar = schedule.add_calendar(calendar);
    let mut meta = Vec::new();
    for name in ["A", "B"] {
        let task = Task {
            duration: 60,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("a task");
        meta.push(TaskMeta {
            name: name.to_string(),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        });
    }
    let link = Dependency {
        pred: TaskId(0),
        succ: TaskId(1),
        kind: LinkType::FS,
        lag: 0,
    };
    schedule.add_dependency(link).expect("a link");
    schedule.add_dependency(link).expect("its twin");
    let solution = schedule.compute().expect("acyclic");
    let wbs = vec![TaskId(0), TaskId(1)];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let timeline = Timeline::new(vec![0, 1], 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline fits")
}

fn live(session: &Session) -> usize {
    session.schedule().live_dependencies().count()
}

#[test]
fn undo_and_redo_bring_back_one_of_two_imported_links() {
    let mut session = duplicated();
    assert_eq!(live(&session), 2);
    let dates = session.solution().dates.clone();
    session
        .edit(Edit::RemoveLink { dep: DepId(1) })
        .expect("removes one of the twins");
    assert_eq!(live(&session), 1);
    assert!(session.undo().expect("undo is not refused as a duplicate"));
    assert_eq!(live(&session), 2, "both links are back");
    assert_eq!(session.solution().dates, dates);
    assert!(session.redo().expect("redoes"));
    assert_eq!(live(&session), 1);
    assert!(session.undo().expect("undoes again"));
    assert_eq!(live(&session), 2);
}

#[test]
fn a_person_restoring_a_twin_is_still_refused() {
    let mut session = duplicated();
    session
        .edit(Edit::RemoveLink { dep: DepId(1) })
        .expect("removes");
    let error = session
        .edit(Edit::RestoreLink { dep: DepId(1) })
        .expect_err("a user restore is checked");
    assert!(
        matches!(error, EditError::AlreadyLinked { .. }),
        "{error:?}"
    );
    assert_eq!(live(&session), 1);
}

fn link_of(session: &Session, dep: u32) -> (LinkType, i64) {
    let d = session.schedule().dependencies()[dep as usize];
    (d.kind, d.lag)
}

#[test]
fn a_lag_edit_works_beside_a_same_type_twin() {
    let mut session = duplicated();
    session
        .edit(Edit::SetLink {
            dep: DepId(0),
            kind: LinkType::FS,
            lag: 60,
        })
        .expect("a lag edit creates no new twin");
    assert_eq!(link_of(&session, 0), (LinkType::FS, 60));
    assert_eq!(
        *session.solution(),
        session.schedule().clone().compute().unwrap()
    );
    assert!(session.undo().expect("undoes"));
    assert_eq!(link_of(&session, 0), (LinkType::FS, 0));
}

#[test]
fn undo_and_redo_of_a_type_edit_are_exact_with_a_twin_live() {
    let mut session = duplicated();
    let dates = session.solution().clone();
    session
        .edit(Edit::SetLink {
            dep: DepId(0),
            kind: LinkType::SS,
            lag: 30,
        })
        .expect("sets one of the twins to SS");
    let set = session.solution().clone();
    assert!(session.undo().expect("undo is not refused as a duplicate"));
    assert_eq!(link_of(&session, 0), (LinkType::FS, 0));
    assert_eq!(*session.solution(), dates);
    assert_eq!(session.history_len(), (0, 1));
    assert!(session.redo().expect("redoes"));
    assert_eq!(link_of(&session, 0), (LinkType::SS, 30));
    assert_eq!(*session.solution(), set);
    assert!(session.undo().expect("undoes again"));
    assert_eq!(link_of(&session, 0), (LinkType::FS, 0));
    assert_eq!(live(&session), 2);
}

#[test]
fn a_refused_type_edit_beside_a_twin_changes_nothing() {
    let mut session = duplicated();
    session
        .edit(Edit::SetLink {
            dep: DepId(0),
            kind: LinkType::SS,
            lag: 0,
        })
        .expect("sets");
    let before = (
        session.solution().clone(),
        session.history_len(),
        session.revision(),
        link_of(&session, 1),
    );
    // Link 1 becoming SS would repeat link 0: a person's edit is checked.
    let error = session
        .edit(Edit::SetLink {
            dep: DepId(1),
            kind: LinkType::SS,
            lag: 0,
        })
        .expect_err("a new twin is refused");
    assert!(
        matches!(error, EditError::AlreadyLinked { .. }),
        "{error:?}"
    );
    assert_eq!(
        (
            session.solution().clone(),
            session.history_len(),
            session.revision(),
            link_of(&session, 1)
        ),
        before
    );
}
