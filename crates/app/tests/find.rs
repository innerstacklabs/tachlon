use crate::common;

use app::{
    Session,
    find::matches,
    meta::{TaskMeta, outline_path},
    outline::Outline,
};
use common::outline_session;
use engine::{Calendar, Schedule, Task, TaskId};

fn schedule_with_tasks(task_count: usize) -> Schedule {
    let day = vec![(480, 720), (780, 1_020)];
    let calendar = Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("valid standard calendar");
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar);
    for _ in 0..task_count {
        schedule
            .add_task(Task {
                duration: 0,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    schedule
}

fn kept_by_filter(session: &Session, task: TaskId) -> bool {
    let outline = session.outline().expect("test session has an outline");
    let children = outline.children(task);
    children.is_empty() && session.solution().dates[task.0 as usize].is_critical()
        || children
            .iter()
            .copied()
            .any(|child| kept_by_filter(session, child))
}

fn summaries(session: &Session) -> Vec<TaskId> {
    let outline = session.outline().expect("test session has an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| outline.is_summary(task))
        .collect()
}

#[test]
fn matches_names_case_insensitively_in_the_requested_order() {
    let schedule = schedule_with_tasks(5);
    let meta = vec![
        TaskMeta {
            name: "école roof".to_string(),
            parent: Some(TaskId(3)),
            outline_level: 2,
            code: None,
            ..TaskMeta::default()
        },
        TaskMeta {
            name: "East".to_string(),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        },
        TaskMeta {
            name: "Roof deck".to_string(),
            parent: Some(TaskId(1)),
            outline_level: 2,
            code: None,
            ..TaskMeta::default()
        },
        TaskMeta {
            name: "West ROOF".to_string(),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        },
        // An imported name keeps its own " / ": this one is a top-level task, not a child of West.
        TaskMeta {
            name: "West / Foundation".to_string(),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        },
    ];
    let wbs = [TaskId(3), TaskId(0), TaskId(4), TaskId(1), TaskId(2)];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("valid forest");

    assert_eq!(matches(&meta, Some(&outline), "roof"), vec![3, 0, 2]);
    assert_eq!(matches(&meta, None, "roof"), vec![0, 2, 3]);
    assert_eq!(matches(&meta, Some(&outline), "ÉCOLE"), vec![0]);
    assert_eq!(matches(&meta, Some(&outline), ""), [] as [u32; 0]);
    assert_eq!(matches(&meta, Some(&outline), "missing"), [] as [u32; 0]);
}

#[test]
fn a_hidden_match_is_revealed_selected_and_scrolled_into_view() {
    let (mut session, meta) = outline_session(2_000);
    let (summary, descendant) = {
        let outline = session.outline().expect("test session has an outline");
        outline
            .preorder()
            .iter()
            .copied()
            .find_map(|summary| {
                (outline.parent(summary).is_none() && outline.subtree(summary).len() > 1)
                    .then(|| (summary, outline.subtree(summary)[1]))
            })
            .expect("demo outline has a level-1 summary with descendants")
    };
    assert!(session.toggle(summary));
    assert!(session.timeline().row_of_task(descendant.0).is_none());

    let query = outline_path(&meta, descendant.0 as usize).to_uppercase();
    assert!(matches(&meta, session.outline(), &query).contains(&descendant.0));
    assert!(session.go_to(descendant.0));

    let outline = session.outline().expect("test session has an outline");
    let mut ancestor = outline.parent(descendant);
    while let Some(parent) = ancestor {
        assert!(!session.is_collapsed(parent));
        ancestor = outline.parent(parent);
    }
    assert_eq!(
        session.selection().expect("match is selected").task,
        descendant.0
    );
    let row = session
        .timeline()
        .row_of_task(descendant.0)
        .expect("revealed match has a row");
    assert!(session.timeline().viewport.visible_rows().contains(&row));
}

#[test]
fn every_match_can_be_visited_in_outline_order() {
    let (mut session, meta) = outline_session(2_000);
    let needle = "level";
    let found = matches(&meta, session.outline(), needle);
    let expected: Vec<u32> = session
        .outline()
        .expect("test session has an outline")
        .preorder()
        .iter()
        .filter(|task| {
            outline_path(&meta, task.0 as usize)
                .to_lowercase()
                .contains(needle)
        })
        .map(|task| task.0)
        .collect();
    assert!(found.len() > 1, "query should produce several matches");
    assert_eq!(found, expected);

    for task in found {
        assert!(session.go_to(task));
        assert_eq!(session.selection().expect("match is selected").task, task);
    }
}

#[test]
fn critical_filter_excludes_hidden_matches_without_expanding_their_ancestors() {
    let (mut session, meta) = outline_session(2_000);
    let (summary, critical_leaf, noncritical_leaf, query) = {
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
                    .collect();
                let critical = leaves
                    .iter()
                    .copied()
                    .find(|task| session.solution().dates[task.0 as usize].is_critical());
                let noncritical = leaves
                    .iter()
                    .copied()
                    .find(|task| !session.solution().dates[task.0 as usize].is_critical());
                critical.zip(noncritical).map(|(critical, noncritical)| {
                    (
                        summary,
                        critical,
                        noncritical,
                        outline_path(&meta, summary.0 as usize),
                    )
                })
            })
            .expect("demo has a summary with critical and noncritical leaves")
    };
    let unfiltered = matches(&meta, session.outline(), &query);
    assert!(unfiltered.contains(&critical_leaf.0));
    assert!(unfiltered.contains(&noncritical_leaf.0));

    assert!(session.toggle(summary), "ancestor must collapse first");
    assert!(session.set_critical_only(true), "filter must turn on");
    let found = session.find(&query);
    assert!(found.contains(&critical_leaf.0), "critical match is kept");
    assert!(
        !found.contains(&noncritical_leaf.0),
        "noncritical match is filtered out"
    );

    session.select(summary.0);
    let selected = session.selection().map(|selection| selection.task);
    let summaries = summaries(&session);
    let collapsed: Vec<_> = summaries
        .iter()
        .map(|&task| session.is_collapsed(task))
        .collect();
    assert!(!session.go_to(noncritical_leaf.0));
    assert_eq!(
        session.selection().map(|selection| selection.task),
        selected,
        "refused navigation leaves selection unchanged"
    );
    assert_eq!(
        summaries
            .iter()
            .map(|&task| session.is_collapsed(task))
            .collect::<Vec<_>>(),
        collapsed,
        "refused navigation leaves every collapse flag unchanged"
    );
    assert!(session.is_collapsed(summary));

    assert!(session.set_critical_only(false), "filter must turn off");
    assert!(
        session.find(&query).contains(&noncritical_leaf.0),
        "filter-off search restores the match"
    );
}

#[test]
fn critical_match_hidden_only_by_collapse_is_found_and_revealed() {
    let (mut session, meta) = outline_session(2_000);
    let (summary, critical_leaf) = {
        let outline = session.outline().expect("test session has an outline");
        outline
            .preorder()
            .iter()
            .copied()
            .filter(|&task| outline.is_summary(task))
            .find_map(|summary| {
                outline
                    .subtree(summary)
                    .iter()
                    .copied()
                    .find(|&task| {
                        !outline.is_summary(task)
                            && session.solution().dates[task.0 as usize].is_critical()
                    })
                    .map(|leaf| (summary, leaf))
            })
            .expect("demo has a summary above a critical leaf")
    };
    assert!(session.set_critical_only(true), "filter must turn on");
    assert!(session.toggle(summary), "kept summary must collapse");
    assert!(session.is_collapsed(summary));

    let query = &outline_path(&meta, critical_leaf.0 as usize);
    assert!(session.find(query).contains(&critical_leaf.0));
    assert!(session.go_to(critical_leaf.0));
    assert!(!session.is_collapsed(summary));
    assert_eq!(
        session
            .selection()
            .expect("critical match is selected")
            .task,
        critical_leaf.0
    );
}

#[test]
fn filtered_find_count_matches_an_independent_recursive_oracle() {
    let (mut session, meta) = outline_session(2_000);
    let query = "level";
    assert!(session.set_critical_only(true), "filter must turn on");
    let expected: Vec<_> = matches(&meta, session.outline(), query)
        .into_iter()
        .filter(|&task| kept_by_filter(&session, TaskId(task)))
        .collect();
    assert!(expected.len() > 1, "oracle query has multiple matches");
    assert_eq!(session.find(query), expected);
}

#[test]
fn going_to_a_collapsed_summary_does_not_expand_it() {
    let (mut session, _) = outline_session(2_000);
    let summary = session
        .outline()
        .expect("test session has an outline")
        .preorder()
        .iter()
        .copied()
        .find(|&task| {
            session
                .outline()
                .is_some_and(|outline| outline.is_summary(task))
        })
        .expect("demo outline has a summary");
    assert!(session.toggle(summary));

    assert!(session.go_to(summary.0));
    assert!(session.is_collapsed(summary));
    assert_eq!(
        session.selection().expect("summary is selected").task,
        summary.0
    );
}

#[test]
fn going_to_a_task_during_a_bar_press_changes_nothing() {
    let (mut session, _) = outline_session(2_000);
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
    session.pointer_down(point.0, point.1, false);
    assert!(session.timeline().bar_drag().is_some());
    let before = (
        session.timeline().viewport.clone(),
        session.selection().map(|selection| selection.task),
        session.stamp(),
    );

    assert!(!session.go_to(task));
    assert_eq!(
        (
            session.timeline().viewport.clone(),
            session.selection().map(|selection| selection.task),
            session.stamp(),
        ),
        before
    );
    session.pointer_up();
}

#[test]
fn going_to_an_unknown_task_changes_nothing() {
    let (mut session, _) = outline_session(2_000);
    let before = (
        session.timeline().viewport.clone(),
        session.selection().map(|selection| selection.task),
        session.stamp(),
    );
    assert!(!session.go_to(u32::MAX));
    assert_eq!(
        (
            session.timeline().viewport.clone(),
            session.selection().map(|selection| selection.task),
            session.stamp(),
        ),
        before
    );
}

#[test]
fn find_go_selects_the_kth_result_of_the_last_find() {
    let (mut session, meta) = outline_session(2_000);
    assert_eq!(session.find_go(0), None, "nothing is found before a search");

    let leaf = meta
        .iter()
        .position(|task| task.outline_level == 3)
        .expect("demo has a leaf task") as u32;
    let found = session.find(&outline_path(&meta, leaf as usize));
    let k = found
        .iter()
        .position(|&task| task == leaf)
        .expect("the leaf matches its own name");
    assert_eq!(session.find_go(k as u32), Some(leaf));
    assert_eq!(
        session.selection().map(|selection| selection.task),
        Some(leaf)
    );
    assert_eq!(
        session.find_go(found.len() as u32),
        None,
        "past the last result"
    );
}

#[test]
fn the_text_accessors_read_the_sessions_names() {
    let (session, meta) = outline_session(300);
    assert_eq!(session.meta(), meta);
    for (index, task) in meta.iter().enumerate() {
        let index = index as u32;
        assert_eq!(
            session.outline_path(index),
            outline_path(&meta, index as usize)
        );
        assert_eq!(session.task_code(index), task.code_text());
        assert_eq!(session.task_label(index), task.name);
        assert_eq!(session.task_outline_level(index), task.outline_level);
    }
    assert!(!session.has_codes(), "the demo has no activity IDs");
    let past = meta.len() as u32;
    assert_eq!(session.outline_path(past), "");
    assert_eq!(session.task_code(past), "");
    assert_eq!(session.task_label(past), "");
    assert_eq!(session.task_outline_level(past), 0);
}

#[test]
fn find_matches_the_derived_path_and_a_name_that_holds_the_separator() {
    let schedule = schedule_with_tasks(3);
    let task = |name: &str, parent: Option<u32>, outline_level| TaskMeta {
        name: name.to_string(),
        parent: parent.map(TaskId),
        outline_level,
        code: None,
        ..TaskMeta::default()
    };
    let meta = vec![
        task("Level 3", None, 1),
        task("Pour / slab", Some(0), 2),
        task("Level 4", None, 1),
    ];
    let wbs = [TaskId(0), TaskId(1), TaskId(2)];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("valid forest");

    assert_eq!(outline_path(&meta, 1), "Level 3 / Pour / slab");
    // A parent's name finds its children, and a path that crosses the separators finds the child.
    assert_eq!(matches(&meta, Some(&outline), "level 3"), vec![0, 1]);
    assert_eq!(matches(&meta, Some(&outline), "pour / slab"), vec![1]);
    assert_eq!(matches(&meta, Some(&outline), "level 3 / pour"), vec![1]);
    assert_eq!(matches(&meta, None, "slab"), vec![1]);
}

#[test]
fn find_matches_a_wbs_code() {
    let schedule = schedule_with_tasks(3);
    let task = |name: &str, wbs_code: Option<&str>| TaskMeta {
        name: name.to_string(),
        parent: None,
        outline_level: 1,
        code: None,
        wbs_code: wbs_code.map(str::to_string),
        ..TaskMeta::default()
    };
    let meta = vec![
        task("Footing", Some("HAND.1.2")),
        task("Slab", Some("HAND.1.3")),
        task("Roof", None),
    ];
    let wbs = [TaskId(0), TaskId(1), TaskId(2)];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("valid forest");

    assert_eq!(matches(&meta, Some(&outline), "hand.1.2"), vec![0]);
    assert_eq!(matches(&meta, Some(&outline), "hand.1"), vec![0, 1]);
    assert_eq!(matches(&meta, Some(&outline), "1.3"), vec![1]);
    assert_eq!(matches(&meta, None, "hand.1"), vec![0, 1]);
    // A file with no WBS codes finds nothing by code.
    let plain = vec![task("Footing", None)];
    assert_eq!(matches(&plain, None, "hand"), [] as [u32; 0]);
}

#[test]
fn an_exact_activity_id_comes_first() {
    let schedule = schedule_with_tasks(3);
    let task = |name: &str, code: &str| TaskMeta {
        name: name.to_string(),
        parent: None,
        outline_level: 1,
        code: Some(code.to_string()),
        ..TaskMeta::default()
    };
    let meta = vec![
        task("Footing", "A100"),
        task("Slab", "A10"),
        task("Roof", "A101"),
    ];
    let wbs = [TaskId(0), TaskId(1), TaskId(2)];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("valid forest");

    // The exact ID leads, in any case; the other matches keep their outline order.
    assert_eq!(matches(&meta, Some(&outline), "a10"), vec![1, 0, 2]);
    assert_eq!(matches(&meta, Some(&outline), "A10"), vec![1, 0, 2]);
    assert_eq!(matches(&meta, None, "a10"), vec![1, 0, 2]);
    // A query that is no task's whole ID changes nothing.
    assert_eq!(matches(&meta, Some(&outline), "a1"), vec![0, 1, 2]);
}

#[test]
fn found_reads_the_last_finds_tasks_and_the_stamp_an_insert_moves() {
    let (mut session, _) = outline_session(200);
    let tasks = session.find("Level");
    assert_ne!(tasks, [] as [u32; 0]);
    let stamp = session.stamp();
    assert_eq!(session.found(), (&tasks[..], stamp));

    session.insert_below_selection().expect("an insert");
    assert_ne!(session.found().1, stamp, "an insert moves the stamp");
}
