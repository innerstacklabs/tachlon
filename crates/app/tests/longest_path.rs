use crate::common;

use app::{RowFilter, Session, edit::Edit};
use common::outline_session;
use engine::TaskId;
use std::collections::BTreeSet;

const N: usize = 800;

fn session(n: usize) -> Session {
    outline_session(n).0
}

fn longest_path_leaves(session: &Session) -> BTreeSet<u32> {
    let outline = session.outline().expect("test session has an outline");
    session
        .schedule()
        .longest_path(session.solution())
        .into_iter()
        .filter(|&task| !outline.is_summary(task))
        .map(|task| task.0)
        .collect()
}

fn critical_leaves(session: &Session) -> BTreeSet<u32> {
    let outline = session.outline().expect("test session has an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| {
            !outline.is_summary(task) && session.solution().dates[task.0 as usize].is_critical()
        })
        .map(|task| task.0)
        .collect()
}

fn hidden_by_collapse(session: &Session, task: TaskId) -> bool {
    let outline = session.outline().expect("test session has an outline");
    let mut parent = outline.parent(task);
    while let Some(ancestor) = parent {
        if session.is_collapsed(ancestor) {
            return true;
        }
        parent = outline.parent(ancestor);
    }
    false
}

fn reference_rows(session: &Session, leaves: &BTreeSet<u32>) -> Vec<u32> {
    let outline = session.outline().expect("test session has an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| {
            !hidden_by_collapse(session, task)
                && outline
                    .subtree(task)
                    .iter()
                    .any(|descendant| leaves.contains(&descendant.0))
        })
        .map(|task| task.0)
        .collect()
}

fn longest_reference(session: &Session) -> Vec<u32> {
    reference_rows(session, &longest_path_leaves(session))
}

#[test]
fn longest_path_shows_exactly_kept_leaves_and_context_summaries() {
    let mut session = session(N);
    let unfiltered = session.timeline().order().len();
    assert!(session.set_longest_path(true));
    let leaves = longest_path_leaves(&session);
    let expected = reference_rows(&session, &leaves);
    assert_eq!(session.timeline().order(), expected);

    let outline = session.outline().expect("test session has an outline");
    let context = expected
        .iter()
        .filter(|&&task| outline.is_summary(TaskId(task)))
        .count();
    assert!(!leaves.is_empty(), "demo must have longest-path leaves");
    assert!(context > 0, "demo must have longest-path context summaries");
    assert_eq!(expected.len(), leaves.len() + context);
    assert!(expected.len() < unfiltered);
}

#[test]
fn modes_are_exclusive_and_turning_off_restores_rows() {
    let mut session = session(N);
    let unfiltered = session.timeline().order().to_vec();
    assert!(session.set_longest_path(true));
    assert_eq!(session.row_filter(), RowFilter::LongestPath);
    assert!(session.longest_path());

    assert!(session.set_critical_only(true));
    assert_eq!(session.row_filter(), RowFilter::Critical);
    assert!(!session.longest_path());
    assert_eq!(
        session.timeline().order(),
        reference_rows(&session, &critical_leaves(&session))
    );

    let stamp = session.stamp();
    let rows = session.timeline().order().to_vec();
    assert!(session.set_longest_path(false));
    assert_eq!(
        (session.stamp(), session.timeline().order()),
        (stamp, rows.as_slice())
    );

    assert!(session.set_longest_path(true));
    assert_eq!(session.row_filter(), RowFilter::LongestPath);
    assert!(!session.critical_only());
    assert_eq!(session.timeline().order(), longest_reference(&session));
    assert!(session.set_longest_path(false));
    assert_eq!(session.timeline().order(), unfiltered);
}

fn status(session: &Session) -> serde_json::Value {
    serde_json::from_str(&session.status_json(&app::RenderFacts::default()))
        .expect("status is JSON")
}

#[test]
fn the_status_names_the_filter_that_is_on_and_still_counts_every_task() {
    let mut session = session(N);
    let fields = |s: &Session| {
        let v = status(s);
        (
            v["filter"].clone(),
            v["criticalOnly"].clone(),
            v["longestPath"].clone(),
            v["n"].clone(),
            v["rowCount"].clone(),
        )
    };
    let all = session.timeline().order().len();
    assert_eq!(
        fields(&session),
        (
            "off".into(),
            false.into(),
            false.into(),
            N.into(),
            all.into()
        )
    );

    assert!(session.set_critical_only(true));
    let critical = session.timeline().order().len();
    assert!(critical < all);
    assert_eq!(
        fields(&session),
        (
            "critical".into(),
            true.into(),
            false.into(),
            N.into(),
            critical.into()
        ),
        "critical only: the rows drop, the task count does not"
    );

    assert!(session.set_longest_path(true));
    let longest = session.timeline().order().len();
    assert_eq!(
        fields(&session),
        (
            "longest".into(),
            false.into(),
            true.into(),
            N.into(),
            longest.into()
        ),
        "longest path replaces critical only"
    );

    assert!(session.set_longest_path(false));
    assert_eq!(
        fields(&session),
        (
            "off".into(),
            false.into(),
            false.into(),
            N.into(),
            all.into()
        )
    );
}

#[test]
fn the_status_counts_the_activities_a_filter_keeps_not_the_rows() {
    let mut session = session(N);
    let kept = |s: &Session| status(s)["keptLeaves"].as_u64().map(|n| n as usize);
    assert_eq!(kept(&session), None, "no filter: no count");

    assert!(session.set_critical_only(true));
    assert_eq!(kept(&session), Some(critical_leaves(&session).len()));
    assert!(
        critical_leaves(&session).len() < session.timeline().order().len(),
        "the rows also hold the summaries above the activities"
    );

    assert!(session.set_longest_path(true));
    assert_eq!(kept(&session), Some(longest_path_leaves(&session).len()));

    assert!(session.collapse_to_level(1));
    assert_eq!(
        kept(&session),
        Some(longest_path_leaves(&session).len()),
        "collapse hides rows, not activities"
    );

    assert!(session.set_longest_path(false));
    assert_eq!(kept(&session), None);
}

/// What the status's `path` owes: the first and last activity by name, the count and the calendar days.
#[expect(
    clippy::similar_names,
    reason = "by_es and by_ef are the two orders of one list"
)]
fn path_line(session: &Session) -> serde_json::Value {
    let leaves: Vec<u32> = longest_path_leaves(session).into_iter().collect();
    let dates = &session.solution().dates;
    let (mut by_es, mut by_ef) = (leaves.clone(), leaves.clone());
    by_es.sort_by_key(|&t| dates[t as usize].es);
    by_ef.sort_by_key(|&t| dates[t as usize].ef);
    let (first, last) = (by_es[0], by_ef[by_ef.len() - 1]);
    let day = |t: i64| t.div_euclid(engine::DAY);
    let days = day(session.solution().project_finish) - day(dates[first as usize].es);
    let name = |t| session.task_label(t);
    serde_json::json!({"first": name(first), "last": name(last), "count": leaves.len(), "days": days})
}

#[test]
fn the_status_carries_the_line_that_names_the_longest_path() {
    let mut session = session(N);
    let path = |s: &Session| status(s)["path"].clone();
    assert!(path(&session).is_null(), "no filter: no line");
    assert!(session.set_longest_path(true));
    let before = path(&session);
    assert_eq!(before, path_line(&session));

    let task = TaskId(*longest_path_leaves(&session).first().unwrap());
    let minutes = session.schedule().tasks()[task.0 as usize].duration + 5 * 1_440;
    session
        .edit(Edit::SetDuration { task, minutes })
        .expect("duration applies");
    assert_eq!(
        path(&session),
        path_line(&session),
        "the line follows an edit"
    );
    assert!(path(&session)["days"].as_i64() > before["days"].as_i64());

    assert!(session.set_longest_path(false));
    assert!(path(&session).is_null(), "off: no line");
}

#[test]
fn collapse_composes_with_longest_path_filter() {
    let mut session = session(N);
    assert!(session.set_longest_path(true));
    assert!(session.collapse_to_level(1));
    assert_eq!(session.timeline().order(), longest_reference(&session));
    assert!(session.timeline().order().iter().all(|&task| {
        session
            .outline()
            .expect("test session has an outline")
            .depth(TaskId(task))
            == 1
    }));

    assert!(session.set_longest_path(false));
    let outline = session.outline().expect("test session has an outline");
    let expected: Vec<_> = outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| !hidden_by_collapse(&session, task))
        .map(|task| task.0)
        .collect();
    assert_eq!(session.timeline().order(), expected);
}

#[test]
fn longest_path_rows_follow_duration_move_undo_and_redo() {
    let (mut session, _) = outline_session(N);
    assert!(session.set_longest_path(true));
    let original_rows = session.timeline().order().to_vec();
    let original_leaves = longest_path_leaves(&session);

    let mut chosen = None;
    for task in original_leaves.iter().copied().take(128) {
        let old = session.schedule().tasks()[task as usize].duration;
        for minutes in [0, 1, old / 2, old.saturating_add(2_000)] {
            if minutes == old {
                continue;
            }
            let edit = Edit::SetDuration {
                task: TaskId(task),
                minutes,
            };
            let Ok(inverse) = session.apply(edit.clone()) else {
                continue;
            };
            let changed = session.timeline().order() != original_rows;
            session.apply(inverse).expect("candidate duration restores");
            if changed {
                chosen = Some(edit);
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }
    let duration = chosen.expect("a longest-path duration edit changes the filtered rows");
    session.edit(duration).expect("duration applies");
    let duration_rows = session.timeline().order().to_vec();
    assert_eq!(duration_rows, longest_reference(&session));
    assert_ne!(duration_rows, original_rows);
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(session.timeline().order(), original_rows);
    assert_eq!(session.redo(), Ok(true));
    assert_eq!(session.timeline().order(), duration_rows);

    let task = longest_path_leaves(&session)
        .into_iter()
        .next()
        .expect("edited schedule still has a longest-path leaf");
    let old_snet = session.schedule().tasks()[task as usize].snet;
    let es = session.solution().dates[task as usize].es;
    session
        .edit(Edit::Move {
            task: TaskId(task),
            snet: Some(es.saturating_add(60)),
        })
        .expect("move applies");
    let move_rows = session.timeline().order().to_vec();
    assert_eq!(move_rows, longest_reference(&session));
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(session.timeline().order(), duration_rows);
    assert_eq!(session.redo(), Ok(true));
    assert_eq!(session.timeline().order(), move_rows);
    assert_ne!(session.schedule().tasks()[task as usize].snet, old_snet);
}

#[test]
fn find_and_go_to_respect_longest_path_filter() {
    let (mut session, meta) = outline_session(N);
    assert!(session.set_longest_path(true));
    let visible: BTreeSet<_> = session.timeline().order().iter().copied().collect();
    let outline = session.outline().expect("test session has an outline");
    let kept = outline
        .preorder()
        .iter()
        .copied()
        .find(|task| visible.contains(&task.0) && !outline.is_summary(*task))
        .expect("filter keeps a leaf");
    let hidden = outline
        .preorder()
        .iter()
        .copied()
        .find(|task| !visible.contains(&task.0) && !outline.is_summary(*task))
        .expect("filter hides a leaf");

    assert!(session.find(&meta[kept.0 as usize].name).contains(&kept.0));
    assert!(
        !session
            .find(&meta[hidden.0 as usize].name)
            .contains(&hidden.0)
    );
    assert!(!session.go_to(hidden.0));
    assert!(session.go_to(kept.0));
}
