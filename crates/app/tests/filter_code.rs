//! The activity code row filter (gs#703, story 12), split from `filter.rs` to keep it under the size limit.

use crate::common;

use app::{
    Session,
    outline::{Outline, demo_layout},
};
use common::collapse::{N, summaries};
use common::{clock, outline_session};
use engine::TaskId;
use import::ActivityCode;
use std::collections::HashSet;
use timeline::Timeline;

fn hidden_by_collapse(session: &Session, task: TaskId) -> bool {
    let outline = session.outline().expect("test session has an outline");
    let mut parent = outline.parent(task);
    while let Some(task) = parent {
        if session.is_collapsed(task) {
            return true;
        }
        parent = outline.parent(task);
    }
    false
}

// The activity code filter (gs#703, story 12). The expected rows come from a walk of a copy of the tasks' codes that
// the test keeps and of the outline's children; nothing here asks the session which rows a filter keeps.

const PHASES: [&str; 3] = ["Civil", "MEP", "Site"];

fn code(code_type: &str, value: &str) -> ActivityCode {
    ActivityCode {
        code_type: code_type.to_owned(),
        value: value.to_owned(),
    }
}

/// The codes task `i` carries: a Phase on all but every seventh task (summaries too), a Discipline on some.
fn codes_of(i: usize) -> Vec<ActivityCode> {
    let phase = (!i.is_multiple_of(7)).then(|| code("Phase", PHASES[i % 3]));
    let discipline = (i % 5 < 3).then(|| code("Discipline", ["ELEC", "MECH"][i % 2]));
    phase.into_iter().chain(discipline).collect()
}

/// A session over the demo layout whose tasks carry [`codes_of`], and the codes the test keeps for its own walk.
fn coded_session(n: usize) -> (Session, Vec<Vec<ActivityCode>>) {
    let mut layout = demo_layout(n);
    let codes: Vec<_> = (0..layout.meta.len()).map(codes_of).collect();
    for (meta, own) in layout.meta.iter_mut().zip(&codes) {
        meta.activity_codes.clone_from(own);
    }
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let (schedule, solution, meta) = (layout.schedule, layout.solution, layout.meta);
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let session = Session::new(schedule, solution, timeline, meta, clock);
    (
        session.with_outline(outline).expect("a matching outline"),
        codes,
    )
}

type Codes = [Vec<ActivityCode>];

/// A leaf is kept when its own codes hold `wanted`; a summary when a child is kept. A summary's own codes do not count.
fn kept_by_code(s: &Session, codes: &Codes, wanted: &ActivityCode, task: TaskId) -> bool {
    let children = s.outline().expect("an outline").children(task);
    let kept = |&child: &TaskId| kept_by_code(s, codes, wanted, child);
    if children.is_empty() {
        codes[task.0 as usize].contains(wanted)
    } else {
        children.iter().any(kept)
    }
}

fn code_oracle(s: &Session, codes: &Codes, wanted: &ActivityCode) -> Vec<u32> {
    let preorder = s.outline().expect("an outline").preorder();
    let rows = preorder.iter().copied();
    rows.filter(|&t| !hidden_by_collapse(s, t) && kept_by_code(s, codes, wanted, t))
        .map(|t| t.0)
        .collect()
}

fn order(session: &Session) -> Vec<u32> {
    session.timeline().order().to_vec()
}

#[test]
fn the_activity_code_filter_keeps_exactly_the_leaves_with_that_code_and_their_summaries() {
    let (mut session, codes) = coded_session(N);
    let (all_rows, stamp, wanted) = (order(&session), session.stamp(), code("Phase", "Civil"));
    assert!(session.set_activity_code_filter("Phase", "Civil"));
    assert_ne!(session.stamp(), stamp, "[code-stamp]");
    assert_eq!(session.activity_code_filter(), Some(&wanted));
    let expected = code_oracle(&session, &codes, &wanted);
    assert_eq!(order(&session), expected, "[code-rows]");

    // Not vacuous: rows go and rows stay, and a summary that carries the code itself goes when no leaf under it does.
    let outline = session.outline().expect("an outline");
    let visible: HashSet<u32> = expected.iter().copied().collect();
    assert!(!visible.is_empty() && visible.len() < all_rows.len());
    let own = |t: &TaskId| {
        outline.is_summary(*t) && codes[t.0 as usize].contains(&wanted) && !visible.contains(&t.0)
    };
    assert!(
        outline.preorder().iter().any(own),
        "[code-summary-own-code]"
    );

    // Collapse combines with the filter, as it does with the critical filter.
    let kept_summary = |t: &TaskId| visible.contains(&t.0);
    let summary = summaries(&session).into_iter().find(kept_summary);
    assert!(session.toggle(summary.expect("a kept summary")));
    assert_eq!(
        order(&session),
        code_oracle(&session, &codes, &wanted),
        "[code-collapse]"
    );
    assert!(session.toggle(summary.expect("a kept summary")));

    assert!(session.clear_activity_code_filter() && session.activity_code_filter().is_none());
    assert_eq!(order(&session), all_rows, "[code-clear]");
}

#[test]
fn an_activity_code_filter_replaces_other_filters_refuses_unknown_codes_and_lists_codes() {
    let (mut session, codes) = coded_session(N);
    assert!(session.set_critical_only(true));
    assert!(session.set_activity_code_filter("Discipline", "ELEC") && !session.critical_only());
    let wanted = code("Discipline", "ELEC");
    assert_eq!(
        order(&session),
        code_oracle(&session, &codes, &wanted),
        "[code-discipline]"
    );
    assert!(session.set_longest_path(true) && session.activity_code_filter().is_none());
    assert!(
        session.clear_activity_code_filter() && session.longest_path(),
        "[other-filter-stays]"
    );

    let (rows, stamp) = (order(&session), session.stamp());
    assert!(!session.set_activity_code_filter("Phase", "Nowhere"));
    assert!(!session.set_activity_code_filter("Nowhere", "Civil"));
    assert_eq!(
        (order(&session), session.stamp()),
        (rows, stamp),
        "[code-refused]"
    );
    let (mut plain, _) = outline_session(N);
    assert!(!plain.set_activity_code_filter("Phase", "Civil") && plain.activity_codes().is_empty());

    let list = |t: &str, v: &[&str]| (t.to_owned(), v.iter().map(|s| (*s).to_owned()).collect());
    let expected: Vec<(String, Vec<String>)> = vec![
        list("Discipline", &["ELEC", "MECH"]),
        list("Phase", &PHASES),
    ];
    assert_eq!(session.activity_codes(), expected, "[codes-listed]");
}

#[test]
fn activity_code_values_list_digit_runs_as_numbers() {
    let mut layout = demo_layout(N);
    for (meta, value) in layout.meta.iter_mut().zip(["L1", "L10", "L2", "L10"]) {
        meta.activity_codes = vec![code("Phase", value)];
    }
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let (schedule, solution, meta) = (layout.schedule, layout.solution, layout.meta);
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let session = Session::new(schedule, solution, timeline, meta, clock);
    let session = session.with_outline(outline).expect("a matching outline");
    let listed = session.activity_codes();
    assert_eq!(
        listed,
        vec![(
            "Phase".to_owned(),
            vec!["L1".into(), "L2".into(), "L10".into()]
        )],
        "[codes-natural-order]"
    );
}
