//! `Session::set_locale` (gantt#1875): the names the app's date labels write follow the host's
//! locale, and a reopened session keeps them. The names here are `m1` to `m12`, so a label that
//! still used the English array fails.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Session, edit::Edit};
use engine::{TaskId, WEEK};

const NAMES: &str = r#"{"months":["m1","m2","m3","m4","m5","m6","m7","m8","m9","m10","m11","m12"],
    "weekdays":["d1","d2","d3","d4","d5","d6","d7"],"weekStart":6}"#;

/// The day of `t` as the labels write it with [`NAMES`]: `14 m10`.
fn label(session: &Session, t: engine::Instant) -> String {
    let date = session.origin().date_of(t);
    format!("{} m{}", date.day, date.month)
}

/// `session`'s what-if finish after a task is lengthened, as `(was, now)`.
fn what_if_finish(session: &mut Session) -> (String, String) {
    session.begin_whatif().unwrap();
    let minutes = session.schedule().tasks()[0].duration + 480;
    session
        .edit(Edit::SetDuration {
            task: TaskId(0),
            minutes,
        })
        .unwrap();
    let summary = session.whatif_summary().unwrap();
    (summary.finish_was, summary.finish_now)
}

#[test]
fn the_labels_use_the_names_the_host_set() {
    let (mut session, _) = common::demo_session(60);
    session.set_data_date_text("2026-10-14").unwrap();
    session.edit(Edit::SetBaseline { baseline: None }).unwrap();
    assert_eq!(session.baseline_data_date_text().as_deref(), Some("14 Oct"));
    session.set_locale(NAMES).unwrap();
    assert_eq!(session.baseline_data_date_text().as_deref(), Some("14 m10"));

    let finish = session.solution().project_finish;
    let (was, _) = what_if_finish(&mut session);
    assert_eq!(was, label(&session, finish));

    common::zoom_to(&mut session, 100.0, (500 * WEEK) as f64);
    let status: serde_json::Value =
        serde_json::from_str(&session.status_json(&app::RenderFacts::default())).unwrap();
    let before = status["offscreen"]["b"]
        .as_str()
        .expect("work lies before the window");
    assert!(
        before.split(' ').nth(1).is_some_and(|m| m.starts_with('m')),
        "{before}"
    );
}

#[test]
fn a_reopened_session_keeps_the_names() {
    let (mut session, _) = common::demo_session(60);
    session.set_locale(NAMES).unwrap();
    let (schedule, solution) = (session.schedule().clone(), session.solution().clone());
    let order = session.timeline().order().to_vec();
    let meta = common::demo_session(60).1;
    let mut opened = session
        .reopen(
            schedule,
            solution,
            order,
            meta,
            None,
            None,
            session.origin(),
        )
        .unwrap();
    let finish = opened.solution().project_finish;
    let (was, _) = what_if_finish(&mut opened);
    assert_eq!(was, label(&opened, finish), "named from the set names");
}

#[test]
fn a_names_text_that_does_not_fit_is_refused_and_changes_nothing() {
    let (mut session, _) = common::demo_session(60);
    session.set_locale(NAMES).unwrap();
    let week_start_7 = NAMES.replace("6}", "7}");
    for bad in [
        "not json",
        r#"{"months":["a"],"weekdays":[],"weekStart":0}"#,
        r#"{"months":["a","b","c","d","e","f","g","h","i","j","k","l"]}"#,
        &week_start_7,
        r#"{"months":[1,2,3,4,5,6,7,8,9,10,11,12],"weekdays":[1,2,3,4,5,6,7],"weekStart":0}"#,
    ] {
        assert!(session.set_locale(bad).is_err(), "{bad}");
    }
    session.set_data_date_text("2026-10-14").unwrap();
    session.edit(Edit::SetBaseline { baseline: None }).unwrap();
    assert_eq!(session.baseline_data_date_text().as_deref(), Some("14 m10"));
}

#[test]
fn the_axis_weeks_start_on_the_day_the_host_set() {
    let (mut session, _) = common::demo_session(60);
    let weeks = |s: &mut Session| {
        s.fit(0.0, (100 * engine::DAY) as f64);
        s.scene()
            .labels
            .into_iter()
            .map(|l| l.text)
            .collect::<Vec<_>>()
    };
    assert!(weeks(&mut session).contains(&"Jan 12".into()), "Mondays");
    session.set_locale(NAMES).unwrap();
    let sundays = weeks(&mut session);
    assert!(sundays.contains(&"Jan 11".into()) && !sundays.contains(&"Jan 12".into()));
}

#[test]
fn the_status_range_labels_use_the_names_the_host_set() {
    let (mut session, _) = common::demo_session(60);
    session.set_locale(NAMES).unwrap();
    let status = session.status_json(&app::RenderFacts::default());
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    for end in ["from", "to"] {
        // A weekday, a day, a month, a year and a time: `d1 5 m1 2026 00:00`.
        let label = status[end].as_str().unwrap();
        assert!(label.starts_with('d') && label.split(' ').nth(2).unwrap().starts_with('m'));
    }
}
