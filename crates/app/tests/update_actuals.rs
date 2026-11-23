//! A host's record sets a task's actual start, actual finish and remaining work by key, as plain data reads them.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::update_tasks::{keyed, saved};
use app::Session;

/// A task's actual start, actual finish and remaining work.
fn held(session: &Session, key: &str) -> (Option<i64>, Option<i64>, Option<i64>) {
    let task = session.task_for_key(key).unwrap();
    let t = &session.schedule().tasks()[task.0 as usize];
    (t.actual_start, t.actual_finish, t.remaining)
}

#[test]
fn a_date_alone_is_the_first_working_minute_or_the_end_of_its_day_and_a_record_that_leaves_it_out_keeps_it()
 {
    let (mut session, leaves) = keyed();
    let depth = session.history_len();
    let json =
        r#"[{"key":"k4","actualStart":"2026-03-02","actualFinish":"2026-03-03","remaining":300}]"#;
    session.update_tasks(json).unwrap();
    let (mut timed, _) = keyed();
    let at = r#"[{"key":"k4","actualStart":"2026-03-02T00:00","actualFinish":"2026-03-03T00:00"}]"#;
    timed.update_tasks(at).unwrap();
    let (midnight, next, _) = held(&timed, "k4");
    let calendar = session.schedule().calendar_of(leaves[4]);
    let opens = calendar.next_working(midnight.unwrap());
    let closes = calendar.prev_period_end(next.unwrap() + 1440).unwrap();
    assert_ne!(opens, midnight.unwrap());
    assert_eq!(held(&session, "k4"), (Some(opens), Some(closes), Some(300)));
    session
        .update_tasks(r#"[{"key":"k4","name":"x"}]"#)
        .unwrap();
    assert_eq!(held(&session, "k4"), (Some(opens), Some(closes), Some(300)));
    assert_eq!(session.history_len(), depth);
}

#[test]
fn an_actual_after_the_data_date_is_refused_by_name_and_the_whole_call_changes_nothing() {
    let (session, leaves) = keyed();
    let early = session.solution().dates[leaves[4].0 as usize].es - 1;
    let mut session = session.with_data_date(Some(early));
    let before = saved(&session);
    let json = r#"[{"key":"k1","name":"Renamed"},{"key":"k4","actualStart":"2030-01-01"}]"#;
    let error = session.update_tasks(json).unwrap_err();
    assert_eq!(
        (error.kind, error.record, error.member.as_deref()),
        ("actualAfterDataDate", Some(1), Some("actualStart"))
    );
    assert_eq!(saved(&session), before);
}

#[test]
fn actuals_that_plain_data_refuses_are_refused_by_member_and_change_nothing() {
    let (mut session, _) = keyed();
    let before = saved(&session);
    let both = r#"{"key":"k4","actualStart":"2026-03-05","actualFinish":"2026-03-02"}"#;
    for (record, refused) in [
        (
            r#"{"key":"k4","progress":50,"remaining":60}"#,
            "badValue progress",
        ),
        (
            r#"{"key":"k4","actualFinish":"2026-03-03"}"#,
            "badValue actualFinish",
        ),
        (r#"{"key":"k4","remaining":60}"#, "badValue remaining"),
        (both, "finishBeforeStart actualFinish"),
    ] {
        let error = session.update_tasks(&format!("[{record}]")).unwrap_err();
        assert_eq!(format!("{} {}", error.kind, error.member.unwrap()), refused);
        assert_eq!(saved(&session), before, "{record}");
    }
}
