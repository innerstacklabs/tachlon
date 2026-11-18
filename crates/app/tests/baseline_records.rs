//! A host hands the chart one baseline as dates by key (tl#2086): the bars and columns read it; a refusal sets nothing.

use crate::common::outline_session;
use app::host::{BaselineRecordsError, KeyTarget};
use app::{Session, edit::Edit};
use std::sync::Arc;
use timeline::scene::span_extent;

/// A session whose first three leaves hold the keys `t1` to `t3`, fitted to 40 days.
fn keyed() -> (Session, Vec<u32>) {
    let (mut session, _) = outline_session(80);
    let outline = session.outline().expect("an outline");
    let leaves = (outline.preorder().iter()).filter(|&&t| !outline.is_summary(t));
    let tasks: Vec<_> = leaves.take(3).copied().collect();
    for (n, &task) in tasks.iter().enumerate() {
        let key = format!("t{}", n + 1);
        let target = KeyTarget::Task(task);
        session.assign_key(target, &key).expect("a key");
    }
    session.fit(0.0, 40.0 * 1440.0);
    (session, tasks.iter().map(|t| t.0).collect())
}

/// A record in January 2026 from a day of the month, with a time or not (`"17T09:00"`).
fn r(key: &str, start: &str, finish: &str) -> String {
    format!(r#"{{"key":"{key}","start":"2026-01-{start}","finish":"2026-01-{finish}"}}"#)
}

#[test]
fn records_are_the_baseline_and_one_undo_puts_back_the_old_one() {
    let (mut session, tasks) = keyed();
    session.edit(Edit::SetBaseline { baseline: None }).unwrap();
    let old = Arc::clone(session.baseline().expect("set"));
    let rows = [
        r("t1", "06", "08"),
        r("t2", "13", "14"),
        r("t3", "17T09:00", "25T17:00"),
    ];
    let json = format!("[{}]", rows.join(","));
    session.set_baseline_records(&json).unwrap();
    let mut scene = timeline::Scene::default();
    session.add_baseline_bars(&mut scene);
    let o = session.origin();
    let day = |t: &str| o.parse_day(&format!("2026-01-{t}")).unwrap();
    let want = [(day("06"), day("09")), (day("13"), day("15"))];
    let want = [want[0], want[1], (day("17") + 9 * 60, day("25") + 17 * 60)];
    assert_eq!(scene.rects.len(), 3, "a strip for each record");
    for (rect, (from, to)) in scene.rects.iter().zip(want) {
        let (x0, x1) = span_extent(&session.timeline().viewport, from, to);
        let x = f64::from(rect.pos[0]);
        let near = |a: f64, b: f64| (a - b).abs() < 1.0;
        assert!(near(x, x0) && near(x + f64::from(rect.size[0]), x1), "{x}");
    }
    let table = session.table_rows_json(0, u32::MAX);
    let rows: serde_json::Value = serde_json::from_str(&table).unwrap();
    let cell = |task: &u32, i: usize| {
        let row = rows.as_array().unwrap().iter().find(|row| row[0] == *task);
        row.unwrap()[i].as_str().unwrap_or("none")
    };
    let cells: Vec<_> = tasks
        .iter()
        .flat_map(|t| [cell(t, 6), cell(t, 7)])
        .collect();
    assert_eq!(cells.join(" "), "-1d -2.8d -4.9d +2.9d -8.9d -7.6d");
    assert!(session.undo().unwrap(), "one step");
    assert!(Arc::ptr_eq(session.baseline().expect("back"), &old));
}

#[test]
fn a_refusal_names_its_record_and_leaves_the_baseline_as_it_was() {
    let (mut session, _) = keyed();
    let two = |second: String| format!("[{},{second}]", r("t1", "06", "08"));
    let cases = [
        ("not json".to_string(), "badJson"),
        ("[]".to_string(), "badJson"),
        (two(r("zz", "06", "08")), "unknownKey@1"),
        (two("{}".to_string()), "noKey@1"),
        (two(r("t1", "06", "08")), "duplicateKey@1"),
        (two(r("t2", "6 Jan", "08")), "badValue@1/start"),
        (two(r("t2", "09", "06T08:00")), "finishBeforeStart@1"),
        (two(r("t2", "09", "08")), "finishBeforeStart@1"),
    ];
    for set in [false, true] {
        if set {
            session.edit(Edit::SetBaseline { baseline: None }).unwrap();
        }
        let before = session.baseline().cloned();
        for (json, want) in &cases {
            let Err(BaselineRecordsError::Record(got)) = session.set_baseline_records(json) else {
                panic!("{json} was not refused as a record");
            };
            let at = got.record.map_or(String::new(), |n| format!("@{n}"));
            let of = got.member.map_or(String::new(), |m| format!("/{m}"));
            assert_eq!(format!("{}{at}{of}", got.kind), *want, "{json}");
            assert_eq!(
                session.baseline().cloned(),
                before,
                "{json} changed the baseline"
            );
        }
    }
    session.set_read_only(true);
    let refused = session.set_baseline_records(&two(r("t2", "06", "08")));
    assert!(
        matches!(refused, Err(BaselineRecordsError::Edit(_))),
        "read-only is the edit's refusal"
    );
}
