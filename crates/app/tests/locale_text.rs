//! `Session::set_locale`'s `texts` on the moved note, the hover card's calendar and deadline text
//! and the compare lines (gantt#1875): each reads in the host's words, English where it set none.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::demo_session;
use app::{CompareError, CompareReport, Session, edit::Edit};
use engine::{DAY, TaskId};

fn say(session: &mut Session, texts: &str) {
    let names = r#""months":["a","b","c","d","e","f","g","h","i","j","k","l"],"weekdays":["a","b","c","d","e","f","g"]"#;
    let locale = format!(r#"{{{names},"weekStart":0,"texts":{texts}}}"#);
    session.set_locale(&locale).unwrap();
}

#[test]
fn the_moved_note_reads_in_the_hosts_words() {
    let (mut session, _) = demo_session(10);
    say(
        &mut session,
        r#"{"movedTasksOne":"{count} Aufgabe verschoben"}"#,
    );
    let edit = Edit::SetDuration {
        task: TaskId(0),
        minutes: 40 * 8 * 60,
    };
    session.edit(edit).unwrap();
    let report = session.move_report().unwrap();
    assert!(report.text.starts_with("1 Aufgabe verschoben"));
    assert!(report.text.contains(" · finish +"), "[a key left out]");
}

#[test]
fn the_cards_calendar_and_deadline_text_read_in_the_hosts_words() {
    let (mut session, _) = demo_session(10);
    let texts =
        r#"{"cardWeekMany":"{days} Tage, {hours} Std.","cardMissed":"{date} ({days} T zu spät)"}"#;
    say(&mut session, texts);
    assert!(session.task_calendar_text(0).ends_with(" Tage, 8 Std."));
    let late = session.solution().dates[0].ef - 2 * DAY;
    let session = session.with_deadlines(vec![Some(late)]);
    let date = session.date_text(late);
    assert_eq!(
        session.task_deadline_text(0),
        format!("{date} (2 T zu spät)")
    );
}

#[test]
fn the_compare_lines_read_in_the_hosts_words() {
    let (mut session, _) = demo_session(10);
    say(
        &mut session,
        r#"{"compareNoSharedId":"keine gemeinsame ID","compareOnlyHere":"Nur hier: {count}"}"#,
    );
    assert_eq!(
        session.say(&CompareError::NoSharedId),
        "keine gemeinsame ID"
    );
    let report = CompareReport {
        only_in_file: 3,
        only_here: 2,
        ..CompareReport::default()
    };
    let lines = r#""lines":["Only in the older file: 3","Nur hier: 2"]"#;
    assert!(session.compare_json(&report).contains(lines));
}
