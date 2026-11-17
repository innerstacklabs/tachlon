//! Indent and outdent (gs#417): `Edit::Reparent`, `Session::indent_selected` and
//! `outdent_selected`. Outdent adopts the siblings below it (ruling R1), and undo hands them back.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{
    SEED, Session, Siblings,
    edit::{Edit, EditError},
    meta::TaskMeta,
    outline::{Outline, demo_layout},
};
use common::{clock, setup};
use engine::{Calendar, Dependency, LinkType, Rng, Schedule, Task, TaskId};
use timeline::{Summaries, Timeline};

/// One task of a hand-built outline: name, parent (an index) and the level stored for it. The
/// levels are given, not derived, so a fixture can hold the gaps an imported file has.
type Row = (&'static str, Option<u32>, u8);

/// A session over `rows` (listed in outline order, ids in that order), 60-minute tasks on a
/// 24/7 calendar, with a finish-to-start link for each `(pred, succ)` pair.
pub(crate) fn build(rows: &[Row], links: &[(u32, u32)]) -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
    let calendar = schedule.add_calendar(calendar);
    let mut meta = Vec::new();
    for &(name, parent, level) in rows {
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
            parent: parent.map(TaskId),
            outline_level: level,
            code: None,
            ..TaskMeta::default()
        });
    }
    for &(pred, succ) in links {
        let link = Dependency {
            pred: TaskId(pred),
            succ: TaskId(succ),
            kind: LinkType::FS,
            lag: 0,
        };
        schedule.add_dependency(link).expect("a link");
    }
    let solution = schedule.compute().expect("acyclic");
    let wbs: Vec<TaskId> = (0..rows.len() as u32).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let order: Vec<u32> = (0..rows.len() as u32).collect();
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline fits")
}

/// R(1) / A(2), B(2) / b1(3), C(2), D(2) / d1(4: a gap an import can hold) / S(1).
pub(crate) const TREE: [Row; 8] = [
    ("R", None, 1),
    ("A", Some(0), 2),
    ("B", Some(0), 2),
    ("b1", Some(2), 3),
    ("C", Some(0), 2),
    ("D", Some(0), 2),
    ("d1", Some(5), 4),
    ("S", None, 1),
];

const R: TaskId = TaskId(0);
const A: TaskId = TaskId(1);
const B: TaskId = TaskId(2);
const B1: TaskId = TaskId(3);
const C: TaskId = TaskId(4);
const D: TaskId = TaskId(5);
const D1: TaskId = TaskId(6);
const S: TaskId = TaskId(7);

/// What a reparent must put back: the preorder, and every task's parent and level.
pub(crate) fn shape(session: &Session) -> (Vec<TaskId>, Vec<TaskMeta>) {
    (
        session.outline().unwrap().preorder().to_vec(),
        session.meta().to_vec(),
    )
}

/// Everything a refused reparent must leave alone.
pub(crate) fn state(session: &Session) -> impl PartialEq + std::fmt::Debug + use<> {
    (
        shape(session),
        session.timeline().order().to_vec(),
        session.solution().dates.clone(),
        session.selection().map(|s| s.task),
        session.revision(),
        session.stamp(),
        session.history_len(),
    )
}

fn levels(session: &Session, tasks: &[TaskId]) -> Vec<(Option<TaskId>, u8)> {
    tasks
        .iter()
        .map(|t| {
            let meta = &session.meta()[t.0 as usize];
            (meta.parent, meta.outline_level)
        })
        .collect()
}

#[test]
fn outdent_adopts_the_following_siblings_and_undo_restores_them_exactly() {
    let mut session = build(&TREE, &[]);
    let before = shape(&session);
    session.select(B.0);

    assert_eq!(session.outdent_selected(), Ok(B));

    let outline = session.outline().unwrap();
    // B sits after R's remaining subtree at the top level, and C and D follow it as children.
    assert_eq!(outline.preorder(), [R, A, B, B1, C, D, D1, S]);
    assert_eq!(outline.parent(B), None);
    assert_eq!(outline.children(B), [B1, C, D]);
    assert_eq!(outline.children(R), [A]);
    assert_eq!(
        levels(&session, &[B, B1, C, D, D1]),
        [
            (None, 1),
            (Some(B), 2),
            (Some(B), 2),
            (Some(B), 2),
            (Some(D), 4)
        ]
    );
    assert_eq!(session.selection().map(|s| s.task), Some(B.0));
    assert_eq!(session.history_len(), (1, 0));

    assert_eq!(session.undo(), Ok(true));
    assert_eq!(shape(&session), before, "undo hands the siblings back");
    assert_eq!(session.selection().map(|s| s.task), Some(B.0));
    assert_eq!(session.redo(), Ok(true));
    assert_eq!(session.outline().unwrap().children(B), [B1, C, D]);
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(shape(&session), before);
}

#[test]
fn undo_puts_back_a_level_the_parent_does_not_imply() {
    let mut session = build(&TREE, &[]);
    let before = shape(&session);
    // d1 is stored at level 4 under D at level 2: the outline an import brought. Outdenting it
    // gives it level 2 (under R); only history knows it was 4.
    session.select(D1.0);
    assert_eq!(session.outdent_selected(), Ok(D1));
    assert_eq!(levels(&session, &[D1]), [(Some(R), 2)]);
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(shape(&session), before);
    assert_eq!(levels(&session, &[D1]), [(Some(D), 4)]);
}

#[test]
fn indent_makes_the_row_the_last_child_of_its_previous_sibling() {
    let mut session = build(&TREE, &[]);
    let before = shape(&session);
    session.select(C.0);

    assert_eq!(session.indent_selected(), Ok(C));

    let outline = session.outline().unwrap();
    assert_eq!(outline.preorder(), [R, A, B, B1, C, D, D1, S]);
    assert_eq!(outline.children(B), [B1, C]);
    assert_eq!(levels(&session, &[C]), [(Some(B), 3)]);
    assert_eq!(session.selection().map(|s| s.task), Some(C.0));
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(shape(&session), before);
}

#[test]
fn indent_under_a_collapsed_parent_opens_it_and_the_selection_follows() {
    let mut session = build(&TREE, &[]);
    session.select(B.0);
    assert!(session.collapse_selected());
    assert!(session.is_collapsed(B));
    session.select(C.0);

    assert_eq!(session.indent_selected(), Ok(C));

    assert!(!session.is_collapsed(B), "the new parent opens");
    assert_eq!(session.selection().map(|s| s.task), Some(C.0));
    assert!(session.timeline().row_of_task(C.0).is_some());
}

#[test]
fn a_subtree_moves_with_its_row_and_every_level_shifts_by_one_delta() {
    let mut session = build(&TREE, &[]);
    session.select(D.0);
    assert_eq!(session.indent_selected(), Ok(D));
    // D goes under C (level 3); d1 keeps its gap: 4 -> 5.
    assert_eq!(levels(&session, &[D, D1]), [(Some(C), 3), (Some(D), 5)]);
    assert_eq!(session.outline().unwrap().subtree(D), [D, D1]);
}

#[test]
fn a_refused_reparent_changes_nothing() {
    // A -> C: A is a leaf with a link, so nothing can be indented under it, and A cannot adopt
    // the siblings below it.
    let mut session = build(&TREE, &[(1, 4)]);
    let refused = |session: &mut Session, task: TaskId, outdent: bool| {
        session.select(task.0);
        let before = state(session);
        let error = if outdent {
            session.outdent_selected()
        } else {
            session.indent_selected()
        }
        .expect_err("refused");
        assert_eq!(state(session), before, "{error}");
        error
    };

    assert_eq!(
        refused(&mut session, A, false),
        EditError::NoPreviousSibling
    );
    assert_eq!(refused(&mut session, R, true), EditError::AlreadyTopLevel);
    assert_eq!(refused(&mut session, S, true), EditError::AlreadyTopLevel);
    // B follows A, a leaf with a link: indenting B would make A a summary.
    assert!(matches!(
        refused(&mut session, B, false),
        EditError::HasLinks { .. }
    ));
    // A would adopt B, C and D and become a summary with a link.
    assert!(matches!(
        refused(&mut session, A, true),
        EditError::HasLinks { .. }
    ));
    session.clear_selection();
    assert_eq!(session.indent_selected(), Err(EditError::NoSelection));
    assert_eq!(session.history_len(), (0, 0));
}

#[test]
fn indenting_under_a_linked_row_names_the_row() {
    // A (a leaf) links to C. Indenting B under A, or outdenting A to adopt B, would make A a
    // summary with a link: the refusal names A and says what to do, not which dependency.
    let mut session = build(&TREE, &[(1, 4)]);
    let sentence = "\"A\" has links, and a summary takes none. Remove its links first, or link its sub-tasks instead.";
    session.select(B.0);
    let error = session.indent_selected().expect_err("refused");
    assert_eq!(error, EditError::HasLinks { name: "A".into() });
    assert_eq!(error.to_string(), sentence);
    session.select(A.0);
    let error = session.outdent_selected().expect_err("refused");
    assert_eq!(error.to_string(), sentence);
    // Without a link the same indent goes through.
    let mut session = build(&TREE, &[]);
    session.select(B.0);
    assert_eq!(session.indent_selected(), Ok(B));
}

#[test]
fn a_reparent_refuses_under_a_filter_or_window_and_during_a_press() {
    let (mut session, _) = common::outline_session(300);
    let task = *session
        .outline()
        .unwrap()
        .preorder()
        .iter()
        .find(|&&t| session.outline().unwrap().parent(t).is_some())
        .unwrap();
    session.select(task.0);
    assert!(session.set_critical_only(true));
    session.select(task.0);
    let before = state(&session);
    assert_eq!(session.outdent_selected(), Err(EditError::ReparentFiltered));
    assert_eq!(session.indent_selected(), Err(EditError::ReparentFiltered));
    assert_eq!(state(&session), before);
    assert!(session.set_critical_only(false));
    assert!(session.set_lookahead(0, 4));
    assert_eq!(session.outdent_selected(), Err(EditError::ReparentFiltered));

    let (mut pressed, _task, _es, point) = setup();
    pressed.pointer_down(point.0, point.1, false);
    let edit = Edit::Reparent {
        task: TaskId(0),
        place: app::Place {
            parent: None,
            after: None,
        },
        siblings: Siblings::Keep,
    };
    assert_eq!(pressed.edit(edit), Err(EditError::Busy));
}

#[test]
fn a_place_inside_the_moving_block_or_a_short_count_is_refused() {
    let mut session = build(&TREE, &[]);
    let before = state(&session);
    let reparent = |session: &mut Session, task, parent, after, siblings| {
        session.edit(Edit::Reparent {
            task,
            place: app::Place { parent, after },
            siblings,
        })
    };
    // Under its own descendant, and after itself.
    assert_eq!(
        reparent(&mut session, B, Some(B1), None, Siblings::Keep),
        Err(EditError::BadPlace)
    );
    assert_eq!(
        reparent(&mut session, B, Some(R), Some(B), Siblings::Keep),
        Err(EditError::BadPlace)
    );
    // After a sibling it would adopt.
    assert_eq!(
        reparent(&mut session, B, Some(R), Some(C), Siblings::Adopt(1)),
        Err(EditError::BadPlace)
    );
    // Adopting more siblings than follow it, releasing more children than it has.
    assert_eq!(
        reparent(&mut session, B, None, Some(R), Siblings::Adopt(3)),
        Err(EditError::BadPlace)
    );
    assert_eq!(
        reparent(&mut session, B, None, Some(R), Siblings::Release(2)),
        Err(EditError::BadPlace)
    );
    assert_eq!(state(&session), before);
}

#[test]
fn an_oversized_count_is_refused_not_a_panic() {
    let mut session = build(&TREE, &[]);
    let before = state(&session);
    for siblings in [Siblings::Adopt(usize::MAX), Siblings::Release(usize::MAX)] {
        assert_eq!(
            session.edit(Edit::Reparent {
                task: B,
                place: app::Place {
                    parent: None,
                    after: Some(R)
                },
                siblings,
            }),
            Err(EditError::BadPlace)
        );
    }
    assert_eq!(state(&session), before);
}

/// Every summary the table shows equals the one a full roll-up of the current outline gives.
fn assert_rollup_is_full(session: &Session) {
    let outline = session.outline().unwrap();
    let rebuilt = Outline::new(session.meta(), outline.preorder(), session.schedule())
        .expect("the outline stays valid");
    let mut summaries = Summaries::new(session.meta().len());
    rebuilt.rollup_full(&session.solution().dates, &mut summaries);
    for task in 0..session.meta().len() as u32 {
        assert_eq!(
            session.timeline().summaries().get(task),
            summaries.get(task),
            "summary {task}"
        );
    }
}

/// 2,000 tasks, with a gap added to some levels so the levels are not all parent plus one.
fn gapped_session(n: usize) -> Session {
    let mut layout = demo_layout(n);
    for (id, meta) in layout.meta.iter_mut().enumerate() {
        if id % 13 == 0 {
            meta.outline_level += 2;
        }
    }
    let outline =
        Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("a valid outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        clock,
    )
    .with_outline(outline)
    .expect("the outline fits")
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test runs the random rounds and undoes them"
)]
fn two_hundred_random_indents_and_outdents_keep_the_schedule_and_undo_exactly() {
    const N: usize = 2_000;
    const STEPS: usize = 200;
    const ATTEMPTS: usize = 20_000;
    let mut session = gapped_session(N);
    let dates = session.solution().dates.clone();
    let leaves: Vec<usize> = (0..N)
        .filter(|&t| !session.outline().unwrap().is_summary(TaskId(t as u32)))
        .collect();
    let mut rng = Rng::new(SEED ^ 0x4_17E0_0417);
    let mut snapshots = vec![shape(&session)];
    let (mut applied, mut refused) = (0, 0);

    for _ in 0..ATTEMPTS {
        if applied == STEPS {
            break;
        }
        let task = TaskId(rng.range(0, N as i64) as u32);
        assert!(session.go_to(task.0), "a live task can be shown");
        let outdent = rng.chance(0.5);
        let before = state(&session);
        let result = if outdent {
            session.outdent_selected()
        } else {
            session.indent_selected()
        };
        match result {
            Ok(moved) => {
                assert_eq!(moved, task);
                applied += 1;
                snapshots.push(shape(&session));
                assert_eq!(session.selection().map(|s| s.task), Some(task.0));
            }
            Err(error) => {
                refused += 1;
                assert!(
                    matches!(
                        error,
                        EditError::NoPreviousSibling
                            | EditError::AlreadyTopLevel
                            | EditError::BadPlace
                            | EditError::HasLinks { .. }
                    ),
                    "{error}"
                );
                assert_eq!(
                    state(&session),
                    before,
                    "a refusal changes nothing: {error}"
                );
                continue;
            }
        }
        // The solution is a full compute, and no leaf's dates moved.
        let solved = session.schedule().clone().compute().expect("acyclic");
        assert_eq!(session.solution().dates, solved.dates);
        assert_eq!(session.solution().dates, dates);
        for &leaf in &leaves {
            assert_eq!(session.solution().dates[leaf], dates[leaf], "leaf {leaf}");
        }
        assert_rollup_is_full(&session);
    }
    assert!(
        applied == STEPS && refused > 5,
        "applied {applied}, refused {refused}"
    );
    assert_eq!(session.history_len(), (applied, 0));

    // Undo every step: each puts back the exact parents, levels and preorder of the one before.
    let last = snapshots.len() - 1;
    for step in (0..last).rev() {
        assert_eq!(session.undo(), Ok(true));
        assert_eq!(shape(&session), snapshots[step], "undo to step {step}");
        assert_eq!(session.solution().dates, dates);
    }
    assert_eq!(session.undo(), Ok(false));
    assert_rollup_is_full(&session);

    // Redo of a middle step, then all the way back.
    for _ in 0..3 {
        assert_eq!(session.redo(), Ok(true));
    }
    assert_eq!(shape(&session), snapshots[3]);
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(session.redo(), Ok(true));
    assert_eq!(shape(&session), snapshots[3]);
    while session.redo().unwrap() {}
    assert_eq!(shape(&session), snapshots[last]);
    assert_rollup_is_full(&session);
}
