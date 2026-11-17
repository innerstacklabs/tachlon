//! What the baseline tests (`baseline*.rs`) share: setting a baseline, and the summary a person
//! would get by subtracting two full `compute()` results.

use app::Session;
use app::baseline::{Baseline, BaselineSummary};
use app::edit::Edit;
use engine::{DAY, Solution, TaskId};
use std::collections::BTreeSet;
use std::sync::Arc;

pub fn set(session: &mut Session) -> Arc<Baseline> {
    session
        .edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    Arc::clone(session.baseline().expect("the baseline is in force"))
}

/// What a person who subtracted two full `compute()` results would count: `then` is the solution
/// when the baseline was set, `live_then` which tasks were live in it.
pub fn brute_force(session: &Session, then: &Solution, live_then: &[bool]) -> BaselineSummary {
    let now = session.schedule().clone().compute().expect("computes");
    assert_eq!(
        *session.solution(),
        now,
        "the session is on its full compute"
    );
    let live_now =
        |i: usize| i < now.dates.len() && !session.schedule().is_deleted(TaskId(i as u32));
    let was = |i: usize| live_then.get(i).copied().unwrap_or(false);
    let mut s = BaselineSummary {
        finish_days: now.project_finish.div_euclid(DAY) - then.project_finish.div_euclid(DAY),
        ..BaselineSummary::default()
    };
    for i in 0..now.dates.len().max(live_then.len()) {
        match (was(i), live_now(i)) {
            (true, true) => {
                let (a, b) = (&then.dates[i], &now.dates[i]);
                s.slipped += u32::from(b.ef > a.ef);
                s.joined += u32::from(b.float == 0 && a.float != 0);
                s.left += u32::from(b.float != 0 && a.float == 0);
            }
            (false, true) => s.added += 1,
            (true, false) => s.removed += 1,
            (false, false) => {}
        }
    }
    s
}

pub fn live_set(session: &Session) -> Vec<bool> {
    (0..session.schedule().tasks().len())
        .map(|i| !session.schedule().is_deleted(TaskId(i as u32)))
        .collect()
}

/// The leaves whose early finish is later than their baseline entry's, counted from the entries
/// themselves. A task with no entry (inserted since) is not one.
pub fn slipped_leaves(session: &Session) -> BTreeSet<u32> {
    let outline = session.outline().expect("test session has an outline");
    let baseline = session.baseline().expect("a baseline is set");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| !outline.is_summary(task))
        .filter(|&task| {
            baseline
                .task(task)
                .is_some_and(|then| session.solution().dates[task.0 as usize].ef > then.ef)
        })
        .map(|task| task.0)
        .collect()
}

pub fn kept_as_slipped(session: &Session, task: TaskId, slipped: &BTreeSet<u32>) -> bool {
    let outline = session.outline().expect("test session has an outline");
    let children = outline.children(task);
    if children.is_empty() {
        return slipped.contains(&task.0);
    }
    children
        .iter()
        .any(|&child| kept_as_slipped(session, child, slipped))
}

pub fn leaf_count(session: &Session) -> usize {
    let outline = session.outline().expect("test session has an outline");
    let leaves = outline.preorder().iter();
    leaves.filter(|&&task| !outline.is_summary(task)).count()
}

/// A duration edit that makes more leaves, but not all of them, finish later than the baseline.
/// The session is left as it found it.
pub fn slipping_edit(session: &mut Session, skip: &[u32]) -> Edit {
    let outline = session.outline().expect("test session has an outline");
    let leaves: Vec<u32> = outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| !outline.is_summary(task) && !skip.contains(&task.0))
        .map(|task| task.0)
        .collect();
    let total = leaf_count(session);
    let already = slipped_leaves(session).len();
    for task in leaves {
        let duration = session.schedule().tasks()[task as usize].duration;
        let edit = Edit::SetDuration {
            task: TaskId(task),
            minutes: duration.saturating_mul(3).max(960),
        };
        let inverse = session.apply(edit.clone()).expect("candidate applies");
        let slipped = slipped_leaves(session).len();
        session.apply(inverse).expect("candidate restores");
        if slipped > already && slipped < total {
            return edit;
        }
    }
    panic!("no leaf slips some of the leaves when it is lengthened");
}
