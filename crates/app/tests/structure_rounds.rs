//! Delete and restore, round 1 of the gs#302 review (ruling devteam-1-96): readers of a deleted
//! task, the edit path's order of checks, the level an undo restores, and the selection rules.

use crate::common;

use app::{
    Place, Session,
    edit::{Edit, EditError},
    meta::TaskMeta,
    outline::Outline,
};
use common::clock;
use engine::{Calendar, Dependency, LinkType, Schedule, ScheduleError, Task, TaskId};
use timeline::Timeline;

/// One task of a hand-built outline, listed in outline (preorder) order.
struct Spec {
    name: &'static str,
    minutes: i64,
    parent: Option<usize>,
    level: u8,
    code: Option<&'static str>,
}

fn spec(name: &'static str, minutes: i64) -> Spec {
    Spec {
        name,
        minutes,
        parent: None,
        level: 1,
        code: None,
    }
}

/// A 24/7 session over `specs` and `links` (task indices), with an outline in listed order.
fn fixture(specs: &[Spec], links: &[(usize, usize)]) -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
    let calendar = schedule.add_calendar(calendar);
    for spec in specs {
        schedule
            .add_task(Task {
                duration: spec.minutes,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("a task");
    }
    for &(pred, succ) in links {
        schedule
            .add_dependency(Dependency {
                pred: TaskId(pred as u32),
                succ: TaskId(succ as u32),
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("a link");
    }
    let solution = schedule.compute().expect("acyclic");
    let meta: Vec<TaskMeta> = specs
        .iter()
        .map(|spec| TaskMeta {
            name: spec.name.to_string(),
            parent: spec.parent.map(|p| TaskId(p as u32)),
            outline_level: spec.level,
            code: spec.code.map(str::to_string),
            ..TaskMeta::default()
        })
        .collect();
    let wbs: Vec<TaskId> = (0..specs.len() as u32).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let order: Vec<u32> = (0..specs.len() as u32).collect();
    Session::new(
        schedule,
        solution,
        Timeline::new(order, 1_000.0, 400.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("the outline fits")
}

fn unknown(task: TaskId) -> EditError {
    EditError::Schedule(ScheduleError::UnknownTask(task))
}

fn selected(session: &Session) -> Option<u32> {
    session.selection().map(|s| s.task)
}

/// `Pour` (code `A1010`) under `Concrete`, then two roots: task 1 is the one to delete.
fn nested() -> Session {
    let mut concrete = spec("Concrete", 0);
    concrete.level = 1;
    let mut pour = spec("Pour", 120);
    pour.parent = Some(0);
    pour.level = 3;
    pour.code = Some("A1010");
    fixture(&[concrete, pour, spec("Cure", 60), spec("Strip", 30)], &[])
}

#[test]
fn text_readers_treat_a_deleted_task_as_absent() {
    let mut session = nested();
    let task = TaskId(1);
    assert_eq!(session.task_label(1), "Pour");
    assert_eq!(session.outline_path(1), "Concrete / Pour");
    assert_eq!(session.task_code(1), "A1010");
    session.edit(Edit::Delete { task }).expect("a leaf deletes");
    assert_eq!(session.task_label(1), "");
    assert_eq!(session.outline_path(1), "");
    assert_eq!(session.task_code(1), "");
    assert!(session.task_details(1).is_none());
    // A live task still reads.
    assert_eq!(session.task_label(2), "Cure");
    assert_eq!(session.outline_path(2), "Cure");
    session.undo().expect("undo");
    assert_eq!(session.task_label(1), "Pour");
    assert_eq!(session.outline_path(1), "Concrete / Pour");
    assert_eq!(session.task_code(1), "A1010");
}

#[test]
fn the_level_reader_treats_a_deleted_task_as_absent() {
    let mut session = nested();
    assert_eq!(session.task_outline_level(1), 3);
    session
        .edit(Edit::Delete { task: TaskId(1) })
        .expect("a leaf deletes");
    assert_eq!(session.task_outline_level(1), 0);
    assert_eq!(session.task_outline_level(2), 1);
}

#[test]
fn the_duration_and_float_readers_treat_a_deleted_task_as_absent() {
    let mut session = nested();
    assert_ne!(session.task_duration(1), "");
    assert_ne!(session.task_float(1), "");
    session
        .edit(Edit::Delete { task: TaskId(1) })
        .expect("a leaf deletes");
    assert_eq!(session.task_duration(1), "");
    assert_eq!(session.task_float(1), "");
    assert_ne!(session.task_duration(2), "");
    assert_ne!(session.task_float(2), "");
}

#[test]
fn an_edit_of_a_deleted_task_is_unknown_before_its_name_is_checked() {
    let mut session = nested();
    let task = TaskId(1);
    session.edit(Edit::Delete { task }).expect("a leaf deletes");
    for name in ["", "  ", "a / b", "line\nbreak", "Fine"] {
        let rename = Edit::Rename {
            task,
            name: name.to_string(),
        };
        assert_eq!(session.edit(rename), Err(unknown(task)), "name {name:?}");
    }
    let duration = Edit::SetDuration { task, minutes: 60 };
    assert_eq!(session.edit(duration), Err(unknown(task)));
    let mv = Edit::Move { task, snet: None };
    assert_eq!(session.edit(mv), Err(unknown(task)));
    assert_eq!(session.edit(Edit::Delete { task }), Err(unknown(task)));
    // A live task still gets the name check.
    let live = Edit::Rename {
        task: TaskId(2),
        name: String::new(),
    };
    assert_eq!(session.edit(live), Err(EditError::EmptyName));
}

#[test]
fn undo_restores_the_level_a_reparenting_restore_replaced() {
    let mut session = nested();
    let task = TaskId(1);
    let concrete = TaskId(0);
    let initial = session.meta().to_vec();
    let before: Vec<_> = session.outline().unwrap().preorder().to_vec();
    session.edit(Edit::Delete { task }).expect("delete");
    let after_delete = session.meta().to_vec();
    let root = Place {
        parent: None,
        after: Some(concrete),
    };
    session
        .edit(Edit::Restore { task, place: root })
        .expect("restore as a root");
    assert_eq!(session.meta()[1].outline_level, 1);
    assert_ne!(session.meta(), after_delete, "the restore changed the meta");
    assert!(session.undo().expect("undo the restore"));
    assert_eq!(
        session.meta(),
        after_delete,
        "undoing the restore puts back the meta before it: the parent and the level"
    );
    assert_eq!(session.meta()[1].parent, Some(concrete));
    assert_eq!(session.meta()[1].outline_level, 3);
    assert!(session.undo().expect("undo the delete"));
    assert_eq!(
        session.meta(),
        initial,
        "every meta field, after the last undo"
    );
    assert_eq!(session.task_outline_level(1), 3);
    assert_eq!(session.outline().unwrap().parent(task), Some(concrete));
    assert_eq!(session.outline().unwrap().preorder(), before);
    // Redo and undo again land on the same states, step by step.
    assert!(session.redo().expect("redo the delete"));
    assert_eq!(session.meta(), after_delete);
    assert!(session.redo().expect("redo the restore"));
    assert_eq!(session.meta()[1].parent, None);
    assert_eq!(session.meta()[1].outline_level, 1);
    assert!(session.undo().expect("undo"));
    assert_eq!(session.meta(), after_delete);
    assert!(session.undo().expect("undo"));
    assert_eq!(session.meta(), initial);
}

#[test]
fn a_persons_restore_derives_its_level_from_the_parent_it_names() {
    // `Edit::Restore` has no level field: only `task` and `place` can be named from outside the
    // crate, so a level cannot be passed. Under a level-1 parent the task gets level 2.
    let mut session = nested();
    let task = TaskId(1);
    session.edit(Edit::Delete { task }).expect("delete");
    let under_concrete = Place {
        parent: Some(TaskId(0)),
        after: None,
    };
    session
        .edit(Edit::Restore {
            task,
            place: under_concrete,
        })
        .expect("restore under Concrete (level 1)");
    assert_eq!(session.meta()[1].parent, Some(TaskId(0)));
    assert_eq!(session.meta()[1].outline_level, 2, "the parent's level + 1");
    assert_eq!(session.task_outline_level(1), 2);
}

#[test]
fn has_codes_reads_live_tasks_only() {
    // Task 1 holds the only code: deleting it hides the ID column, undo shows it again.
    let mut session = nested();
    assert!(session.has_codes());
    session
        .edit(Edit::Delete { task: TaskId(1) })
        .expect("delete the only coded task");
    assert!(!session.has_codes(), "no live task has a code");
    assert!(session.undo().expect("undo the delete"));
    assert!(session.has_codes(), "the coded task is live again");
}

/// Three roots with no links: A, B and C.
fn roots() -> Session {
    fixture(&[spec("A", 100), spec("B", 100), spec("C", 100)], &[])
}

#[test]
fn deleting_an_unselected_task_keeps_the_selection() {
    let mut session = roots();
    session.select(0);
    session
        .edit(Edit::Delete { task: TaskId(1) })
        .expect("a leaf deletes");
    assert_eq!(selected(&session), Some(0));
    session
        .edit(Edit::Delete { task: TaskId(2) })
        .expect("a leaf deletes");
    assert_eq!(selected(&session), Some(0));
    session.clear_selection();
    session
        .edit(Edit::Restore {
            task: TaskId(1),
            place: Place {
                parent: None,
                after: Some(TaskId(0)),
            },
        })
        .expect("restore selects the restored task");
    assert_eq!(selected(&session), Some(1));
}

#[test]
fn deleting_the_selection_moves_to_the_next_row_else_the_previous_else_none() {
    let mut session = roots();
    session.select(1);
    session.delete_selected().expect("delete B");
    assert_eq!(selected(&session), Some(2), "the next row");
    session.delete_selected().expect("delete C");
    assert_eq!(selected(&session), Some(0), "the previous row");
    session.delete_selected().expect("delete A");
    assert_eq!(selected(&session), None, "no rows left");
}

/// A (100) then B (100) in a chain, and C (150) alone: A and B are critical (200), C is not.
/// Deleting A or B shortens the chain, so C becomes critical and the other chain task is not.
fn chain_and_loner() -> Session {
    let mut session = fixture(&[spec("A", 100), spec("B", 100), spec("C", 150)], &[(0, 1)]);
    assert!(session.set_critical_only(true));
    assert_eq!(session.timeline().order(), [0, 1]);
    session
}

#[test]
fn critical_only_delete_selects_a_row_that_is_still_visible() {
    let mut session = chain_and_loner();
    session.select(0);
    session.delete_selected().expect("delete A");
    // B is no longer critical, C is now: C is the next visible row, B has none.
    assert_eq!(session.timeline().order(), [2]);
    assert_eq!(selected(&session), Some(2));
    assert_eq!(session.selection().map(|s| s.row), Some(0));
}

#[test]
fn critical_only_delete_of_the_last_visible_row_selects_the_previous_visible_row() {
    let mut session = fixture(&[spec("A", 200), spec("B", 100), spec("C", 200)], &[]);
    assert!(session.set_critical_only(true));
    assert_eq!(session.timeline().order(), [0, 2]);
    session.select(2);
    session.delete_selected().expect("delete C");
    assert_eq!(session.timeline().order(), [0]);
    assert_eq!(selected(&session), Some(0));
}

#[test]
fn restore_under_a_filter_selects_a_visible_task_and_keeps_the_filter() {
    // C is not critical, so the filter hides it once it is back.
    let mut session = fixture(&[spec("A", 100), spec("B", 100), spec("C", 50)], &[(0, 1)]);
    session.select(0);
    session
        .edit(Edit::Delete { task: TaskId(2) })
        .expect("delete C");
    assert_eq!(selected(&session), Some(0));
    assert!(session.set_critical_only(true));
    assert!(session.undo().expect("undo the delete"));
    assert!(!session.schedule().is_deleted(TaskId(2)), "C is restored");
    assert!(session.critical_only(), "the filter is not cleared");
    assert_eq!(session.timeline().row_of_task(2), None, "C has no row");
    assert_eq!(selected(&session), Some(0), "the selection stays");

    // A visible restored task is selected.
    let mut session = fixture(&[spec("A", 100), spec("B", 100), spec("C", 50)], &[(0, 1)]);
    session
        .edit(Edit::Delete { task: TaskId(1) })
        .expect("delete B");
    assert!(session.set_critical_only(true));
    session.select(0);
    assert!(session.undo().expect("undo the delete"));
    assert!(session.critical_only());
    assert_eq!(selected(&session), Some(1), "B is critical again and shown");
}

#[test]
fn undo_and_redo_put_back_the_repeated_links_an_import_kept() {
    // Two identical A -> B links, as an import leaves them (gs#356).
    let mut session = fixture(&[spec("A", 100), spec("B", 100)], &[(0, 1), (0, 1)]);
    let live = |s: &Session| {
        (0..2)
            .filter(|&i| s.schedule().is_dependency_live(engine::DepId(i)))
            .count()
    };
    assert_eq!(live(&session), 2);
    session
        .edit(Edit::Delete { task: TaskId(1) })
        .expect("delete B");
    assert_eq!(live(&session), 0);
    assert!(session.undo().expect("undo the delete"));
    assert!(!session.schedule().is_deleted(TaskId(1)), "B is back");
    assert_eq!(live(&session), 2, "both links are live again");
    assert!(session.redo().expect("redo the delete"));
    assert!(session.schedule().is_deleted(TaskId(1)));
    assert!(session.undo().expect("undo again"));
    assert!(!session.schedule().is_deleted(TaskId(1)), "B is back");
    assert_eq!(live(&session), 2);
}
