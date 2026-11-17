use crate::common;

use app::{
    SEED, Session,
    outline::{Outline, OutlineError, demo_layout},
};
use common::clock;
use common::collapse::{N, link_pairs, schedule_with_tasks, session, summaries, task_meta};
use engine::{Dependency, LinkType, Rng, TaskId};
use std::collections::{BTreeSet, HashSet};
use timeline::{HIDDEN_ROW, Timeline};

fn visible_oracle(session: &Session) -> Vec<u32> {
    let outline = session.outline().expect("test session has an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| {
            let mut ancestor = outline.parent(task);
            while let Some(parent) = ancestor {
                if session.is_collapsed(parent) {
                    return false;
                }
                ancestor = outline.parent(parent);
            }
            true
        })
        .map(|task| task.0)
        .collect()
}

fn hidden_count(session: &Session) -> usize {
    session
        .outline()
        .expect("test session has an outline")
        .len()
        - visible_oracle(session).len()
}

fn dependency_pairs(session: &Session) -> BTreeSet<(u32, u32)> {
    session
        .schedule()
        .dependencies()
        .iter()
        .map(|dependency| (dependency.pred.0, dependency.succ.0))
        .collect()
}

#[test]
fn random_collapse_sets_keep_the_row_index_consistent() {
    let mut session = session(N);
    let summaries = summaries(&session);
    let mut rng = Rng::new(SEED ^ 0x47B2_C011_A95E_5EED);
    assert_eq!(
        link_pairs(&session),
        dependency_pairs(&session),
        "[links-visible-only] the full-view link oracle must see every expanded link"
    );

    for iteration in 0..200 {
        for &summary in &summaries {
            let wanted = match iteration {
                0 => false,
                1 => true,
                _ => rng.chance(0.3),
            };
            if session.is_collapsed(summary) != wanted {
                assert!(session.toggle(summary), "summary toggle must succeed");
            }
        }

        let oracle = visible_oracle(&session);
        assert_eq!(
            session.timeline().order(),
            oracle,
            "[rows-oracle] iteration {iteration} differs from the ancestor walk"
        );
        for (row, &task) in session.timeline().order().iter().enumerate() {
            assert_eq!(
                session.timeline().row_of()[task as usize],
                row as u32,
                "[row-of-inverse] task {task} does not map back from row {row}"
            );
            assert_eq!(
                session.timeline().row_of_task(task),
                Some(row),
                "[row-of-inverse] row_of_task disagrees for task {task}"
            );
        }
        let visible: HashSet<u32> = oracle.iter().copied().collect();
        for task in 0..N as u32 {
            if !visible.contains(&task) {
                assert_eq!(
                    session.timeline().row_of()[task as usize],
                    HIDDEN_ROW,
                    "[row-of-inverse] hidden task {task} has a row"
                );
                assert_eq!(
                    session.timeline().row_of_task(task),
                    None,
                    "[row-of-inverse] row_of_task finds hidden task {task}"
                );
            }
        }
        assert_eq!(
            session.timeline().order().len(),
            N - hidden_count(&session),
            "[row-count] visible plus hidden must equal the task count"
        );
        assert_eq!(
            session.timeline().viewport.rows,
            session.timeline().order().len(),
            "[row-count] viewport row count must track the order"
        );
        let expected_links: BTreeSet<_> = session
            .schedule()
            .dependencies()
            .iter()
            .filter(|dependency| {
                visible.contains(&dependency.pred.0) && visible.contains(&dependency.succ.0)
            })
            .map(|dependency| (dependency.pred.0, dependency.succ.0))
            .collect();
        assert_eq!(
            link_pairs(&session),
            expected_links,
            "[links-visible-only] iteration {iteration} retained a hidden-ended link"
        );
    }
}

#[test]
fn collapse_then_expand_returns_the_original_rows() {
    let mut session = session(N);
    let original_order = session.timeline().order().to_vec();
    let original_row_of = session.timeline().row_of().to_vec();
    let original_links = link_pairs(&session);
    let summaries = summaries(&session);
    let mut rng = Rng::new(SEED ^ 0x47B2_0002);
    let mut chosen = BTreeSet::new();
    while chosen.len() < 50 {
        let index = rng.range(0, summaries.len() as i64 - 1) as usize;
        chosen.insert(summaries[index]);
    }
    let chosen: Vec<_> = chosen.into_iter().collect();
    for &summary in &chosen {
        assert!(session.toggle(summary), "collapse must succeed");
    }
    for &summary in chosen.iter().rev() {
        assert!(session.toggle(summary), "reverse expansion must succeed");
    }
    assert_eq!(
        (session.timeline().order(), session.timeline().row_of()),
        (original_order.as_slice(), original_row_of.as_slice()),
        "[round-trip] rows and inverse index did not return exactly"
    );
    assert_eq!(
        link_pairs(&session),
        original_links,
        "[round-trip-links] links did not return exactly"
    );
}

#[test]
fn a_nested_collapsed_child_stays_collapsed_after_its_parent_reexpands() {
    let mut session = session(N);
    let (parent, child) = {
        let outline = session.outline().expect("test session has an outline");
        outline
            .preorder()
            .iter()
            .copied()
            .find_map(|parent| {
                outline
                    .children(parent)
                    .iter()
                    .copied()
                    .find(|&child| outline.is_summary(child))
                    .map(|child| (parent, child))
            })
            .expect("demo outline has nested summaries")
    };
    assert!(session.toggle(child), "child collapse must succeed");
    assert!(session.toggle(parent), "parent collapse must succeed");
    assert!(session.toggle(parent), "parent expansion must succeed");
    assert!(
        session.is_collapsed(child),
        "[nested-flag] child collapse flag was lost"
    );
    let order: HashSet<_> = session.timeline().order().iter().copied().collect();
    let descendants = &session
        .outline()
        .expect("test session has an outline")
        .subtree(child)[1..];
    assert!(
        order.contains(&child.0) && descendants.iter().all(|task| !order.contains(&task.0)),
        "[nested-rows] child must be visible while all its descendants stay hidden"
    );
}

#[test]
fn toggle_during_a_press_changes_nothing() {
    let mut session = session(N);
    let summary = summaries(&session)[0];
    let (row, task) = {
        let outline = session.outline().expect("test session has an outline");
        session
            .timeline()
            .order()
            .iter()
            .copied()
            .enumerate()
            .find(|&(_, task)| {
                !outline.is_summary(TaskId(task))
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
    assert_eq!(
        session
            .timeline()
            .hit_test(&session.solution().dates, point.0, point.1),
        Some(task),
        "press precondition must land on the leaf bar"
    );
    session.pointer_down(point.0, point.1, false);
    let before = (
        session.timeline().order().to_vec(),
        session.timeline().row_of().to_vec(),
        session.stamp(),
        session.is_collapsed(summary),
    );
    assert!(
        !session.toggle(summary),
        "[press-refused] a toggle during a bar press must be refused"
    );
    assert_eq!(
        (
            session.timeline().order().to_vec(),
            session.timeline().row_of().to_vec(),
            session.stamp(),
            session.is_collapsed(summary),
        ),
        before,
        "[press-rows] a refused toggle changed row state, stamp, or flag"
    );
    session.pointer_up();
    assert!(
        session.toggle(summary),
        "[press-released] toggle must succeed after release"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test lists every case the toggle refuses"
)]
fn toggle_refuses_what_it_cannot_do() {
    let mut session = session(N);
    let leaf = session
        .outline()
        .expect("test session has an outline")
        .preorder()
        .iter()
        .copied()
        .find(|&task| {
            !session
                .outline()
                .expect("test session has an outline")
                .is_summary(task)
        })
        .expect("demo has a leaf");
    let before = (
        session.timeline().order().to_vec(),
        session.timeline().row_of().to_vec(),
        session.stamp(),
    );
    assert!(
        !session.toggle(leaf),
        "[leaf] a leaf toggle must be refused"
    );
    assert_eq!(
        (
            session.timeline().order().to_vec(),
            session.timeline().row_of().to_vec(),
            session.stamp(),
        ),
        before,
        "[leaf] a refused leaf toggle changed state"
    );
    assert!(
        !session.toggle(TaskId(u32::MAX)) && !session.toggle(TaskId(N as u32)),
        "[out-of-range] unknown task ids must be refused"
    );

    let layout = demo_layout(200);
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
        .expect("demo layout has a valid outline");
    let flat_summaries: Vec<_> = outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| outline.is_summary(task))
        .collect();
    let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
    let mut flat = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    );
    assert!(
        flat.outline().is_none() && flat_summaries.iter().all(|&task| !flat.toggle(task)),
        "[flat] a session without an outline must refuse summary ids"
    );

    let small = demo_layout(100);
    let wrong_outline = Outline::new(&small.meta, &small.wbs, &small.schedule)
        .expect("small demo outline is valid");
    let large = demo_layout(200);
    let large_timeline = Timeline::new(large.order, 1000.0, 400.0, 1.0);
    let error = Session::new(
        large.schedule,
        large.solution,
        large_timeline,
        Vec::new(),
        clock,
    )
    .with_outline(wrong_outline)
    .err();
    assert_eq!(
        error,
        Some(OutlineError::WrongLength),
        "[with-outline-length] mismatched outline length must be refused"
    );

    let meta = vec![
        task_meta(None, 1),
        task_meta(Some(TaskId(0)), 2),
        task_meta(Some(TaskId(0)), 2),
    ];
    let wbs = [TaskId(0), TaskId(1), TaskId(2)];
    let clean = schedule_with_tasks(3);
    let outline = Outline::new(&meta, &wbs, &clean).expect("dependency-free outline is valid");
    let mut linked = schedule_with_tasks(3);
    linked
        .add_dependency(Dependency {
            pred: TaskId(0),
            succ: TaskId(1),
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("dependency names known tasks");
    let solution = linked.compute().expect("single dependency is acyclic");
    let timeline = Timeline::new(vec![0, 1, 2], 1000.0, 400.0, 1.0);
    let error = Session::new(linked, solution, timeline, Vec::new(), clock)
        .with_outline(outline)
        .err();
    assert_eq!(
        error,
        Some(OutlineError::SummaryHasDependency(engine::DepId(0))),
        "[with-outline-dep] dependency on a summary must be refused"
    );
}

#[test]
fn toggle_bumps_the_stamp_and_nothing_else() {
    let mut session = session(N);
    let summary = summaries(&session)[0];
    let leaf = session
        .outline()
        .expect("test session has an outline")
        .preorder()
        .iter()
        .copied()
        .find(|&task| {
            !session
                .outline()
                .expect("test session has an outline")
                .is_summary(task)
        })
        .expect("demo has a leaf");
    let (stamp, revision) = (session.stamp(), session.revision());
    let dates = session.solution().dates.clone();
    let finish = session.solution().project_finish;
    assert!(session.toggle(summary), "summary toggle must succeed");
    assert_eq!(
        session.stamp(),
        stamp.wrapping_add(1),
        "[stamp] successful toggle must increment the stamp once"
    );
    assert_eq!(
        (
            session.revision(),
            session.solution().dates.as_slice(),
            session.solution().project_finish,
        ),
        (revision, dates.as_slice(), finish),
        "[revision] toggle must not alter revision or schedule dates"
    );
    let before = (
        session.timeline().order().to_vec(),
        session.timeline().row_of().to_vec(),
        session.stamp(),
        session.revision(),
        link_pairs(&session),
    );
    assert!(
        !session.toggle(leaf),
        "refused leaf toggle must return false"
    );
    assert_eq!(
        (
            session.timeline().order().to_vec(),
            session.timeline().row_of().to_vec(),
            session.stamp(),
            session.revision(),
            link_pairs(&session),
        ),
        before,
        "[refused-no-change] a refused toggle must change no observable field"
    );
}

#[test]
fn selection_stays_keyed_by_task_id_across_collapse() {
    let mut session = session(N);
    let (summary, task, other) = {
        let outline = session.outline().expect("test session has an outline");
        outline
            .preorder()
            .iter()
            .copied()
            .filter(|&task| outline.is_summary(task))
            .find_map(|summary| {
                let leaves: Vec<_> = outline
                    .subtree(summary)
                    .iter()
                    .copied()
                    .filter(|&task| !outline.is_summary(task))
                    .take(2)
                    .collect();
                (leaves.len() == 2).then_some((summary, leaves[0], leaves[1]))
            })
            .expect("demo summary has two leaf descendants")
    };
    session.select(task.0);
    assert!(session.toggle(summary), "collapse must succeed");
    assert_eq!(
        session.selection().expect("selection remains").task,
        task.0,
        "[selection-kept] collapse changed the selected task id"
    );
    assert_eq!(
        session.selection().expect("selection remains").row,
        HIDDEN_ROW,
        "[selection-hidden-row] hidden selection must use the sentinel row"
    );
    session.select(other.0);
    assert_eq!(
        session.selection().expect("selection remains").task,
        task.0,
        "[select-hidden] selecting a hidden task must preserve the selection"
    );
    assert!(session.toggle(summary), "expansion must succeed");
    assert_eq!(
        session.selection().expect("selection remains").row as usize,
        session
            .timeline()
            .row_of_task(task.0)
            .expect("expanded selected task has a row"),
        "[selection-row] expanded selection row must follow the row index"
    );
}
