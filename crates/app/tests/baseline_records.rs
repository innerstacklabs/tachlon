//! A host hands the chart one baseline as dates by key (tl#2086): the strips and variance cells read against it,
//! one undo puts the old baseline back, and a refusal names its record and sets nothing.

use crate::common::outline_session;
use app::host::{BaselineRecordsError, KeyTarget};
use app::{Session, edit::Edit};
use std::sync::Arc;
use timeline::scene::span_extent;

/// A session whose first three leaves hold the keys `t1` to `t3`, fitted to 40 days.
fn keyed() -> (Session, Vec<u32>) {
    let (mut session, _) = outline_session(80);
    let outline = session.outline().expect("an outline");
    let leaves = outline
        .preorder()
        .iter()
        .filter(|&&t| !outline.is_summary(t));
    let tasks: Vec<_> = leaves.take(3).copied().collect();
    for (n, &task) in tasks.iter().enumerate() {
        session
            .assign_key(KeyTarget::Task(task), &format!("t{}", n + 1))
            .expect("a key");
    }
    session.fit(0.0, 40.0 * 1440.0);
    (session, tasks.iter().map(|t| t.0).collect())
}

fn rec(key: &str, start: &str, finish: &str) -> String {
    format!(r#"{{"key":"{key}","start":"{start}","finish":"{finish}"}}"#)
}

#[test]
fn records_are_the_baseline_and_one_undo_puts_back_the_old_one() {
    let (mut session, tasks) = keyed();
    session
        .edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let old = Arc::clone(session.baseline().expect("set"));
    let json = [
        rec("t1", "2026-01-06", "2026-01-08"),
        rec("t2", "2026-01-13", "2026-01-14"),
    ];
    let json = format!(
        "[{},{}]",
        json.join(","),
        rec("t3", "2026-01-17T09:00", "2026-01-25T17:00")
    );
    session
        .set_baseline_records(&json)
        .expect("the records set");
    let mut scene = timeline::Scene::default();
    session.add_baseline_bars(&mut scene);
    let day = |text: &str| session.origin().parse_day(text).expect("a day");
    let want = [
        (day("2026-01-06"), day("2026-01-09")),
        (day("2026-01-13"), day("2026-01-15")),
        (day("2026-01-17") + 9 * 60, day("2026-01-25") + 17 * 60),
    ];
    assert_eq!(
        scene.rects.len(),
        3,
        "a strip for each record, the day-only finish at its day's end"
    );
    for (rect, (from, to)) in scene.rects.iter().zip(want) {
        let (x0, x1) = span_extent(&session.timeline().viewport, from, to);
        let (left, right) = (
            f64::from(rect.pos[0]),
            f64::from(rect.pos[0] + rect.size[0]),
        );
        assert!(
            (left - x0).abs() < 1.0 && (right - x1).abs() < 1.0,
            "{left} {right}: {x0} {x1}"
        );
    }
    let rows: serde_json::Value =
        serde_json::from_str(&session.table_rows_json(0, u32::MAX)).expect("JSON");
    let cell = |task: &u32, i: usize| {
        let row = rows
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row[0] == *task);
        row.expect("a row")[i].as_str().unwrap_or("none")
    };
    let cells: Vec<_> = tasks
        .iter()
        .map(|task| [cell(task, 6), cell(task, 7)])
        .collect();
    assert_eq!(
        cells,
        [["-1d", "-2.8d"], ["-4.9d", "+2.9d"], ["-8.9d", "-7.6d"]],
        "read against the records"
    );
    assert!(session.undo().expect("undoes"));
    assert!(
        Arc::ptr_eq(session.baseline().expect("back"), &old),
        "one undo, the old baseline"
    );
    assert!(
        session.undo().expect("undoes") && session.baseline().is_none(),
        "then none"
    );
    session
        .set_baseline_records(&json)
        .expect("the records set");
    assert!(
        session.undo().expect("undoes") && session.baseline().is_none(),
        "one undo, none"
    );
}

#[test]
fn a_refusal_names_its_record_and_leaves_the_baseline_as_it_was() {
    let (mut session, _) = keyed();
    let ok = rec("t1", "2026-01-06", "2026-01-08");
    let two = |second: String| format!("[{ok},{second}]");
    let cases = [
        ("not json".to_string(), "badJson", None, None),
        ("[]".to_string(), "badJson", None, None),
        (
            two(rec("zz", "2026-01-06", "2026-01-08")),
            "unknownKey",
            Some(1),
            None,
        ),
        (two("{}".to_string()), "noKey", Some(1), None),
        (two(ok.clone()), "duplicateKey", Some(1), None),
        (
            two(rec("t2", "6 Jan", "2026-01-08")),
            "badValue",
            Some(1),
            Some("start"),
        ),
        (
            two(rec("t2", "2026-01-06", "")),
            "badValue",
            Some(1),
            Some("finish"),
        ),
        (
            two(rec("t2", "2026-01-09", "2026-01-06T08:00")),
            "finishBeforeStart",
            Some(1),
            None,
        ),
        (
            two(rec("t2", "2026-01-09", "2026-01-08")),
            "finishBeforeStart",
            Some(1),
            None,
        ),
    ];
    for set in [false, true] {
        if set {
            session
                .edit(Edit::SetBaseline { baseline: None })
                .expect("a baseline sets");
        }
        let before = session.baseline().cloned();
        for (json, kind, record, member) in &cases {
            let Err(BaselineRecordsError::Record(got)) = session.set_baseline_records(json) else {
                panic!("{json} was not refused as a record");
            };
            assert_eq!(
                (got.kind, got.record, got.member.as_deref()),
                (*kind, *record, *member),
                "{json}"
            );
            assert_eq!(
                session.baseline().cloned(),
                before,
                "{json} changed the baseline"
            );
        }
    }
    session.set_read_only(true);
    let refused = session.set_baseline_records(&two(rec("t2", "2026-01-06", "2026-01-08")));
    assert!(
        matches!(refused, Err(BaselineRecordsError::Edit(_))),
        "read-only is the edit's refusal"
    );
}
