//! A host's late dates, floats and critical flag, compared with the chart's (`differs` of an update).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::what_moves;
use app::{Session, edit::Edit, host::BaselineRecordsError, host::KeyTarget};
use engine::{LinkType, TaskId};
use serde_json::{Value, json};

/// The what-moves fixture with task D, which has no link, keyed `d` and four hours long: 2.5 days of float.
fn keyed() -> Session {
    let mut session = what_moves::session();
    session.assign_key(KeyTarget::Task(TaskId(3)), "d").unwrap();
    let call = r#"[{"key":"d","duration":240}]"#;
    session.update_tasks(call).unwrap();
    session
}

/// What `results_json` wrote for D, under the names a host's record takes.
fn handed_back(session: &Session) -> Value {
    let rows: Vec<Value> = serde_json::from_str(&session.results_json().unwrap()).unwrap();
    let row = &rows[3];
    json!({"key": "d", "hostStart": row["start"], "hostFinish": row["finish"], "hostLateStart": row["lateStart"],
        "hostLateFinish": row["lateFinish"], "hostFloat": row["float"], "hostFreeFloat": row["freeFloat"], "hostCritical": row["critical"]})
}

fn differs(session: &mut Session, record: &Value) -> Vec<String> {
    let call = format!("[{record}]");
    session.update_tasks(&call).unwrap().differs
}

#[test]
fn what_results_gave_is_handed_back_unchanged_and_nothing_differs() {
    let mut session = keyed();
    let record = handed_back(&session);
    assert_eq!(record["hostFloat"], json!(2.5));
    assert_eq!(differs(&mut session, &record), Vec::<String>::new());
}

#[test]
fn a_late_finish_a_day_off_a_float_and_a_flag_are_named_and_a_late_start_that_matches_is_not() {
    let mut session = keyed();
    let mut record = handed_back(&session);
    record["hostLateStart"] = json!("2026-01-07");
    record["hostLateFinish"] = json!("2026-01-08T17:00");
    record["hostFloat"] = json!(1.5);
    record["hostCritical"] = json!(true);
    let line = |member, host, chart| {
        format!(r#"{{"record":0,"key":"d","member":"{member}","host":{host},"chart":{chart}}}"#)
    };
    let (late, held) = (r#""2026-01-08T17:00""#, r#""2026-01-07T17:00""#);
    let expected = [
        line("critical", "true", "false"),
        line("float", "1.5", "2.5"),
        line("lateFinish", late, held),
    ];
    assert_eq!(differs(&mut session, &record), expected);
}

#[test]
fn a_fraction_is_read_in_a_host_record_is_a_bad_value_in_a_whole_member_and_is_refused_elsewhere() {
    let mut session = keyed();
    let host = r#"[{"key":"d","hostFloat":2.5}]"#;
    session.update_tasks(host).unwrap();
    for member in ["duration", "progress", "hostFloat"] {
        let value = if member == "hostFloat" { "true" } else { "1.5" };
        let error = session.update_tasks(&format!(r#"[{{"key":"d","{member}":{value}}}]"#));
        let error = error.unwrap_err();
        assert_eq!(
            (error.kind, error.member.as_deref()),
            ("badValue", Some(member))
        );
    }
    let set = session.set_baseline_records(r#"[{"key":"d","start":"2026-01-05","finish":2.5}]"#);
    assert!(matches!(set, Err(BaselineRecordsError::Record(e)) if e.kind == "badJson"));
    assert!(app::document::read(r#"{"version":1,"x":2.5}"#, &|| 0.0).is_err());
}

#[test]
fn a_free_float_is_compared_with_the_free_float_and_not_the_total_float() {
    let mut session = what_moves::sized(5);
    for (n, key) in [(3, "d"), (4, "e")] {
        session.assign_key(KeyTarget::Task(TaskId(n)), key).unwrap();
    }
    let (pred, succ) = (TaskId(3), TaskId(4));
    session
        .edit(Edit::AddLink {
            pred,
            succ,
            kind: LinkType::FS,
            lag: 0,
        })
        .unwrap();
    let rows: Vec<Value> = serde_json::from_str(&session.results_json().unwrap()).unwrap();
    let (total, free) = (&rows[3]["float"], &rows[3]["freeFloat"]);
    assert_ne!(total, free);
    let found = differs(&mut session, &json!({"key": "d", "hostFreeFloat": total}));
    let line =
        format!(r#"{{"record":0,"key":"d","member":"freeFloat","host":{total},"chart":{free}}}"#);
    assert_eq!(found, [line]);
}
