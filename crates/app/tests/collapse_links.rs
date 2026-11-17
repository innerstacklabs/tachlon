use crate::common;

use app::Session;
use app::outline::Outline;
use common::clock;
use common::collapse::{N, link_pairs, schedule_with_tasks, session, summaries, task_meta};
use engine::TaskId;
use std::collections::HashSet;
use timeline::{HIDDEN_ROW, Timeline};

#[test]
fn links_with_a_hidden_end_are_absent_and_visible_high_id_links_survive() {
    let mut session = session(N);
    let before = link_pairs(&session);
    let (kept_root, high_pair) = {
        let outline = session.outline().expect("test session has an outline");
        let root_of = |mut task: TaskId| {
            while let Some(parent) = outline.parent(task) {
                task = parent;
            }
            task
        };
        session
            .schedule()
            .dependencies()
            .iter()
            .find_map(|dependency| {
                let root = root_of(dependency.pred);
                let visible_count = outline.roots().len() + outline.subtree(root).len() - 1;
                (root == root_of(dependency.succ)
                    && outline.is_summary(root)
                    && dependency.pred.0.min(dependency.succ.0) as usize >= visible_count)
                    .then_some((root, (dependency.pred.0, dependency.succ.0)))
            })
            .expect("a demo dependency has high-id ends in one root subtree")
    };
    let roots = session
        .outline()
        .expect("test session has an outline")
        .roots()
        .to_vec();
    for root in roots {
        if root != kept_root
            && session
                .outline()
                .expect("test session has an outline")
                .is_summary(root)
        {
            assert!(session.toggle(root), "root collapse must succeed");
        }
    }
    let after = link_pairs(&session);
    let hidden: HashSet<u32> = session
        .timeline()
        .row_of()
        .iter()
        .enumerate()
        .filter(|&(_, &row)| row == HIDDEN_ROW)
        .map(|(task, _)| task as u32)
        .collect();
    assert!(
        after.len() < before.len()
            && after
                .iter()
                .all(|(pred, succ)| !hidden.contains(pred) && !hidden.contains(succ)),
        "[hidden-end] hidden-ended links must disappear"
    );
    assert!(
        high_pair.0.min(high_pair.1) as usize >= session.timeline().order().len()
            && after.contains(&high_pair),
        "[high-id] a visible high-id dependency was lost"
    );
}

#[test]
fn c_skips_hidden_tasks_and_summaries() {
    let mut schedule = schedule_with_tasks(6);
    let solution = schedule.compute().expect("zero-duration schedule computes");
    assert!(
        solution.dates.iter().all(engine::TaskDates::is_critical),
        "[c-precondition] zero-duration tasks must all be critical"
    );
    let meta = vec![
        task_meta(None, 1),
        task_meta(Some(TaskId(0)), 2),
        task_meta(Some(TaskId(0)), 2),
        task_meta(None, 1),
        task_meta(Some(TaskId(3)), 2),
        task_meta(Some(TaskId(3)), 2),
    ];
    let wbs: Vec<_> = (0..6).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("small forest is valid");
    let timeline = Timeline::new((0..6).collect(), 1000.0, 400.0, 1.0);
    let mut session = Session::new(schedule, solution, timeline, Vec::new(), clock)
        .with_outline(outline)
        .expect("outline matches the schedule");
    assert!(
        session.toggle(TaskId(3)),
        "second summary collapse succeeds"
    );
    let mut visited = Vec::new();
    for _ in 0..6 {
        let row = session
            .next_critical()
            .expect("a visible critical leaf exists");
        visited.push(session.timeline().order()[row as usize]);
    }
    assert!(
        visited.iter().all(|task| !matches!(task, 0 | 3)),
        "[c-skips-summaries] critical navigation visited a summary"
    );
    assert!(
        visited.iter().all(|task| !matches!(task, 4 | 5)),
        "[c-skips-hidden] critical navigation visited a hidden leaf"
    );
    assert!(
        session.toggle(TaskId(3)),
        "second summary expansion succeeds"
    );
    visited.clear();
    for _ in 0..8 {
        let row = session
            .next_critical()
            .expect("a visible critical leaf exists");
        visited.push(session.timeline().order()[row as usize]);
    }
    assert!(
        visited.contains(&4) && visited.contains(&5),
        "[c-skips-hidden] expanded leaves must rejoin critical navigation"
    );
}

#[test]
fn hover_and_hit_test_never_find_a_hidden_task() {
    let mut session = session(N);
    let summary = summaries(&session)
        .into_iter()
        .find(|&summary| {
            session
                .outline()
                .expect("test session has an outline")
                .subtree(summary)
                .len()
                > 2
        })
        .expect("demo has a nontrivial summary");
    assert!(session.toggle(summary), "collapse must succeed");
    let rows = session.timeline().order().len();
    let height = 40.0 + rows as f64 * session.timeline().viewport.row_height + 2.0;
    session.resize(10_000.0, height, 1.0);
    session.fit(0.0, session.solution().project_finish as f64);
    let visible: HashSet<_> = session.timeline().order().iter().copied().collect();
    for row in 0..rows {
        let task = session.timeline().order()[row];
        let dates = &session.solution().dates[task as usize];
        let x = session
            .timeline()
            .viewport
            .x_of((dates.es + dates.ef) as f64 / 2.0);
        let y =
            session.timeline().viewport.row_top(row) + session.timeline().viewport.row_height / 2.0;
        let hit = session.timeline().hit_test(&session.solution().dates, x, y);
        assert!(
            hit.is_none_or(|found| found == task && visible.contains(&found)),
            "[hit-hidden] visible-row hit test returned a hidden or different task"
        );
        session.pointer_move(x, y);
        let hovered = session.timeline().hovered(&session.solution().dates);
        assert!(
            hovered.is_none_or(|found| found == task && visible.contains(&found)),
            "[hover-hidden] hover returned a hidden or different task"
        );
    }
    // The x of the first row's bar: a point below the last row must still hit
    // nothing, even at an x where a row-0 task has its bar.
    let first = &session.solution().dates[session.timeline().order()[0] as usize];
    let first_x = session
        .timeline()
        .viewport
        .x_of((first.es + first.ef) as f64 / 2.0);
    let below = session.timeline().viewport.row_top(rows) + 0.5;
    assert_eq!(
        session
            .timeline()
            .hit_test(&session.solution().dates, first_x, below),
        None,
        "[hit-below] point below the last visible row must hit nothing"
    );
}
