//! Insert below the selected subtree (gs#303, gs#50 PR 3).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{
    Place, Session,
    edit::{Edit, EditError},
    meta::TaskMeta,
    outline::Outline,
};
use common::{outline_session, setup};
use engine::{Calendar, Dependency, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

const N: usize = 300;

/// Everything an insert changes, in comparable form.
fn view(session: &Session) -> impl PartialEq + std::fmt::Debug + use<> {
    (
        session.outline().unwrap().preorder().to_vec(),
        session.timeline().order().to_vec(),
        session.solution().dates.clone(),
        session.meta().to_vec(),
        session.selection().map(|s| (s.task, s.row)),
    )
}

fn leaf(session: &Session) -> TaskId {
    let outline = session.outline().unwrap();
    *outline
        .preorder()
        .iter()
        .find(|&&t| !outline.is_summary(t) && outline.parent(t).is_some())
        .expect("a nested leaf")
}

fn summary(session: &Session) -> TaskId {
    let outline = session.outline().unwrap();
    *outline
        .preorder()
        .iter()
        .find(|&&t| outline.is_summary(t) && outline.parent(t).is_some())
        .expect("a nested summary")
}

/// The new task is where `after`'s whole subtree ends, one level below `parent`, and is selected.
fn assert_inserted(session: &Session, new: TaskId, before: &[TaskId], after: Option<TaskId>) {
    let outline = session.outline().unwrap();
    let preorder = outline.preorder();
    let at = preorder.iter().position(|&t| t == new).expect("in outline");
    let expected = after.map_or(before.len(), |a| {
        let old = before.iter().position(|&t| t == a).unwrap();
        // The subtree of `a` ends where the preorder first leaves its depth.
        let level = session.meta()[a.0 as usize].outline_level;
        before[old + 1..]
            .iter()
            .position(|t| session.meta()[t.0 as usize].outline_level <= level)
            .map_or(before.len(), |n| old + 1 + n)
    });
    assert_eq!(at, expected);
    let mut without = preorder.to_vec();
    without.remove(at);
    assert_eq!(without, before);
    let meta = &session.meta()[new.0 as usize];
    assert_eq!(meta.name, "New task");
    assert!(
        meta.code
            .as_deref()
            .is_some_and(|code| code.starts_with('A'))
    );
    assert_eq!(
        meta.outline_level,
        after.map_or(1, |a| session.meta()[a.0 as usize].outline_level)
    );
    assert_eq!(
        meta.parent,
        after.and_then(|a| session.meta()[a.0 as usize].parent)
    );
    assert_eq!(session.task_duration(new.0), "1d");
    assert_eq!(session.selection().map(|s| s.task), Some(new.0));
}

fn insert_after(session: &mut Session, selected: Option<TaskId>) -> (TaskId, Vec<TaskId>) {
    match selected {
        Some(t) => session.select(t.0),
        None => session.clear_selection(),
    }
    let before = session.outline().unwrap().preorder().to_vec();
    let new = session.insert_below_selection().expect("inserts");
    assert_inserted(session, new, &before, selected);
    (new, before)
}

#[test]
fn inserts_below_a_leaf() {
    let (mut session, _) = outline_session(N);
    let task = leaf(&session);
    insert_after(&mut session, Some(task));
}

#[test]
fn inserts_after_the_last_descendant_of_an_expanded_summary() {
    let (mut session, _) = outline_session(N);
    let task = summary(&session);
    assert!(!session.is_collapsed(task));
    let (new, _) = insert_after(&mut session, Some(task));
    let outline = session.outline().unwrap();
    assert!(!outline.is_summary(new));
    assert!(outline.children(task).iter().all(|&c| c != new));
}

#[test]
fn insert_keeps_the_selected_row_and_undo_and_redo_restore_the_rows() {
    let (mut session, _) = outline_session(N);
    let task = leaf(&session);
    session.select(task.0);
    let rows_before = session.timeline().order().to_vec();
    let row = session.timeline().row_of_task(task.0).unwrap();
    let steps = session.history_len().0;
    let new = session.insert_below_selection().unwrap();
    assert!(!rows_before.contains(&new.0), "the new task is a new id");
    assert_eq!(session.timeline().row_of_task(task.0), Some(row));
    assert_eq!(session.timeline().row_of_task(new.0), Some(row + 1));
    assert_eq!(session.timeline().order().len(), rows_before.len() + 1);
    assert_eq!(session.history_len().0, steps + 1, "one undo step");
    assert!(session.undo().unwrap());
    assert_eq!(session.timeline().order(), &rows_before[..]);
    assert_eq!(session.task_count(), N);
    assert!(session.redo().unwrap());
    assert_eq!(session.timeline().row_of_task(new.0), Some(row + 1));
    let selected = session.selection().map(|s| (s.task, s.row as usize));
    assert_eq!(selected, Some((new.0, row + 1)));
}

#[test]
fn inserts_after_a_collapsed_summary() {
    let (mut session, _) = outline_session(N);
    let task = summary(&session);
    assert!(session.toggle(task));
    assert!(session.is_collapsed(task));
    let (new, _) = insert_after(&mut session, Some(task));
    assert!(session.is_collapsed(task));
    assert!(session.timeline().row_of_task(new.0).is_some());
}

#[test]
fn inserts_as_the_last_root_with_nothing_selected() {
    let (mut session, _) = outline_session(N);
    let (new, _) = insert_after(&mut session, None);
    let outline = session.outline().unwrap();
    assert_eq!(outline.roots().last(), Some(&new));
    assert_eq!(session.meta()[new.0 as usize].outline_level, 1);
    assert_eq!(session.meta()[new.0 as usize].parent, None);
}

#[test]
fn place_below_names_the_sibling_position() {
    let (session, _) = outline_session(N);
    let task = leaf(&session);
    let parent = session.outline().unwrap().parent(task);
    assert_eq!(
        session.place_below(Some(task)),
        Place {
            parent,
            after: Some(task)
        }
    );
    let last = session.outline().unwrap().roots().last().copied();
    assert_eq!(
        session.place_below(None),
        Place {
            parent: None,
            after: last
        }
    );
}

/// A reference built through the engine API: the same tasks added to the starting schedule, solved
/// from scratch, and an outline built by the plain rule on the previous outline.
#[test]
fn many_inserts_equal_a_reference_built_through_the_engine() {
    let (mut session, _) = outline_session(N);
    let mut reference: Schedule = session.schedule().clone();
    let mut meta = session.meta().to_vec();
    let mut wbs = session.outline().unwrap().preorder().to_vec();
    let picks: Vec<Option<TaskId>> = {
        let preorder = wbs.clone();
        (0..40)
            .map(|i| (i % 5 != 0).then(|| preorder[(i * 37) % preorder.len()]))
            .collect()
    };
    let mut ids = Vec::new();
    for pick in picks {
        // Selecting a task under a collapsed ancestor is not possible; none are collapsed.
        let calendar = pick.map_or_else(
            || {
                reference.tasks()[session.outline().unwrap().roots().last().unwrap().0 as usize]
                    .calendar
            },
            |p| reference.tasks()[p.0 as usize].calendar,
        );
        let (new, before) = insert_after(&mut session, pick);
        let day = app::edit::day_minutes(&reference.calendars()[calendar.0 as usize]);
        let id = reference
            .add_task(Task {
                duration: day,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .unwrap();
        assert_eq!(id, new);
        let _ = before;
        // The expected outline and meta come from the reference alone: the new task goes where
        // the picked task's subtree ends (the preorder first leaving its level), at its level.
        let (at, parent, level) = match pick {
            Some(p) => {
                let from = wbs.iter().position(|&t| t == p).unwrap();
                let level = meta[p.0 as usize].outline_level;
                let end = wbs[from + 1..]
                    .iter()
                    .position(|t| meta[t.0 as usize].outline_level <= level)
                    .map_or(wbs.len(), |n| from + 1 + n);
                (end, meta[p.0 as usize].parent, level)
            }
            None => (wbs.len(), None, 1),
        };
        wbs.insert(at, new);
        meta.push(TaskMeta {
            name: "New task".into(),
            parent,
            outline_level: level,
            // The ID rule has its own tests; this one is about where the row goes.
            code: session.meta()[new.0 as usize].code.clone(),
            ..TaskMeta::default()
        });
        ids.push(new);
    }
    let mut fresh = reference.clone();
    fresh.clear_cached_order();
    assert_eq!(&fresh.compute().unwrap(), session.solution());
    assert_eq!(meta, session.meta());
    let rebuilt = Outline::new(&meta, &wbs, &reference).expect("valid");
    assert_eq!(rebuilt.preorder(), session.outline().unwrap().preorder());
    assert_eq!(&wbs[..], session.outline().unwrap().preorder());
    assert_eq!(session.task_count(), reference.live_task_count());
    // Every insert undoes, in reverse, to the starting live view.
    for &id in ids.iter().rev() {
        assert!(session.undo().unwrap());
        assert!(session.schedule().is_deleted(id));
    }
    assert_eq!(session.task_count(), N);
}

#[test]
fn undo_redo_keeps_the_id_and_a_new_insert_takes_a_new_one() {
    let (mut session, _) = outline_session(N);
    let task = leaf(&session);
    session.select(task.0);
    let first = session.insert_below_selection().unwrap();
    let inserted = view(&session);
    // Undoing an insert follows the Delete rule: the task on the row after the inserted one,
    // else the one before it.
    let order = session.timeline().order();
    let row = session.timeline().row_of_task(first.0).unwrap();
    let expected = order
        .get(row + 1)
        .or_else(|| row.checked_sub(1).map(|r| &order[r]))
        .copied();
    assert!(session.undo().unwrap());
    assert!(session.schedule().is_deleted(first));
    assert!(session.timeline().row_of_task(first.0).is_none());
    assert_eq!(session.selection().map(|s| s.task), expected);
    assert!(session.redo().unwrap());
    assert_eq!(view(&session), inserted);
    assert!(session.undo().unwrap());
    session.select(task.0);
    let second = session.insert_below_selection().unwrap();
    assert_ne!(first, second);
    assert_eq!(second.0, first.0 + 1);
}

#[test]
fn refused_under_a_filter_or_window_and_during_a_press() {
    let (mut session, _) = outline_session(N);
    let task = leaf(&session);
    session.select(task.0);
    assert!(session.set_critical_only(true));
    let before = session.history_len();
    let shown = view(&session);
    let error = session.insert_below_selection().unwrap_err();
    assert_eq!(view(&session), shown, "rows, dates and selection stay");
    assert_eq!(error, EditError::FilterActive);
    assert_eq!(
        error.to_string(),
        "Clear the row filter or window to insert a row."
    );
    assert_eq!(session.history_len(), before);
    assert_eq!(session.task_count(), N);
    assert!(session.set_critical_only(false));
    assert!(session.set_lookahead(0, 4));
    assert_eq!(
        session.insert_below_selection(),
        Err(EditError::FilterActive)
    );
    assert_eq!(session.task_count(), N);

    let (mut pressed, _task, _es, point) = setup();
    pressed.pointer_down(point.0, point.1, false);
    let place = Place {
        parent: None,
        after: None,
    };
    assert_eq!(
        pressed.edit(Edit::Insert {
            place,
            name: "x".into()
        }),
        Err(EditError::Busy)
    );
}

#[test]
fn rename_and_set_duration_work_on_the_inserted_task() {
    let (mut session, _) = outline_session(N);
    let task = leaf(&session);
    session.select(task.0);
    let new = session.insert_below_selection().unwrap();
    session
        .edit(Edit::Rename {
            task: new,
            name: "Pour".into(),
        })
        .unwrap();
    session
        .edit(Edit::SetDuration {
            task: new,
            minutes: 3 * app::edit::day_minutes(session.schedule().calendar_of(new)),
        })
        .unwrap();
    assert_eq!(session.meta()[new.0 as usize].name, "Pour");
    assert_eq!(session.task_duration(new.0), "3d");
    assert!(session.undo().unwrap());
    assert_eq!(session.task_duration(new.0), "1d");
    assert!(session.undo().unwrap());
    assert_eq!(session.meta()[new.0 as usize].name, "New task");
}

#[test]
fn a_typed_insert_name_is_checked() {
    let (mut session, _) = outline_session(N);
    let place = session.place_below(None);
    assert_eq!(
        session.edit(Edit::Insert {
            place,
            name: " ".into()
        }),
        Err(EditError::EmptyName)
    );
    assert_eq!(
        session.edit(Edit::Insert {
            place,
            name: "a / b".into()
        }),
        Err(EditError::InvalidName)
    );
}

/// `Concrete` (level 1) holds `Pour` at level 3, a skipped level; `Cure` then `Strip` are roots,
/// `Cure` before `Strip` as a finish-to-start link.
fn skipped_levels() -> Session {
    levels_with_last_root(1)
}

/// The same fixture with `Strip` stored at `level`: an imported root keeps the level it had.
fn levels_with_last_root(level: u8) -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
    let calendar = schedule.add_calendar(calendar);
    for minutes in [0, 120, 60, 30] {
        let task = Task {
            duration: minutes,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("a task");
    }
    schedule
        .add_dependency(Dependency {
            pred: TaskId(2),
            succ: TaskId(3),
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("a link");
    let solution = schedule.compute().expect("acyclic");
    let row = |name: &str, parent: Option<u32>, level: u8| TaskMeta {
        name: name.to_string(),
        parent: parent.map(TaskId),
        outline_level: level,
        code: None,
        ..TaskMeta::default()
    };
    let meta = vec![
        row("Concrete", None, 1),
        row("Pour", Some(0), 3),
        row("Cure", None, 1),
        row("Strip", None, level),
    ];
    let wbs: Vec<TaskId> = (0..4).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let timeline = Timeline::new(vec![0, 1, 2, 3], 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, common::clock)
        .with_outline(outline)
        .expect("the outline fits")
}

#[test]
fn inserting_below_a_task_under_a_skipped_level_keeps_its_level() {
    let mut session = skipped_levels();
    session.select(1);
    let new = session.insert_below_selection().unwrap();
    let held = (
        session.meta()[new.0 as usize].parent,
        session.meta()[new.0 as usize].outline_level,
    );
    assert_eq!(held, (Some(TaskId(0)), 3));
    assert!(session.undo().unwrap());
    assert!(session.redo().unwrap());
    let meta = &session.meta()[new.0 as usize];
    assert_eq!((meta.parent, meta.outline_level), held);
    // The public edit agrees: the sibling's level, else the parent's plus one, else 1.
    let first_child = Place {
        parent: Some(TaskId(0)),
        after: None,
    };
    let child = TaskId(session.schedule().tasks().len() as u32);
    session
        .edit(Edit::Insert {
            place: first_child,
            name: "Child".into(),
        })
        .unwrap();
    assert_eq!(session.meta()[child.0 as usize].outline_level, 2);
}

#[test]
fn inserting_with_nothing_selected_is_level_one_whatever_the_last_root_holds() {
    let mut session = levels_with_last_root(3);
    session.clear_selection();
    let new = session.insert_below_selection().unwrap();
    let level = |session: &Session| session.meta()[new.0 as usize].outline_level;
    assert_eq!(session.meta()[new.0 as usize].parent, None);
    assert_eq!(level(&session), 1);
    assert!(session.undo().unwrap());
    assert!(session.redo().unwrap());
    assert_eq!(level(&session), 1);
    // Selecting that root makes the insert its sibling at its stored level.
    session.select(3);
    let sibling = session.insert_below_selection().unwrap();
    assert_eq!(session.meta()[sibling.0 as usize].outline_level, 3);
}

#[test]
fn a_refused_insert_leaves_the_session_as_it_was() {
    let mut session = skipped_levels();
    let (slots, history) = (session.schedule().tasks().len(), session.history_len());
    let before = view(&session);
    let place = Place {
        parent: Some(TaskId(2)),
        after: None,
    };
    let error = session
        .edit(Edit::Insert {
            place,
            name: "x".into(),
        })
        .unwrap_err();
    assert!(matches!(error, EditError::Outline(_)), "{error:?}");
    assert_eq!(session.schedule().tasks().len(), slots);
    assert_eq!(session.solution().dates.len(), slots);
    assert_eq!(session.meta().len(), slots);
    assert_eq!(session.task_count(), slots);
    assert_eq!(session.history_len(), history);
    assert_eq!(view(&session), before);
    // A later edit recomputes over the same slots and does not panic.
    session
        .edit(Edit::SetDuration {
            task: TaskId(1),
            minutes: 240,
        })
        .unwrap();
    assert_eq!(session.solution().dates.len(), slots);
    assert_eq!(session.task_count(), slots);
}

/// The 50k timing is recorded, never asserted: no timing in CI.
#[test]
#[ignore = "bench: records the insert time; run with --release -- --ignored --nocapture"]
fn insert_time_on_the_50k_stress_schedule() {
    let (mut session, _) = common::rooted_stress_session(50_000);
    let t0 = std::time::Instant::now();
    let new = session.insert_below_selection().unwrap();
    println!(
        "insert on 50,000 tasks: {:.1} ms (budget 100 ms), new task {new:?}",
        t0.elapsed().as_secs_f64() * 1e3
    );
}
