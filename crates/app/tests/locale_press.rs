//! A drag note and the printed filter line read `set_locale`'s words (gantt#1875).

use crate::{common::outline_session, drag_actuals::started_session};
use app::Session;

fn speak(session: &mut Session, texts: &str) {
    let names = r#""months":["a","b","c","d","e","f","g","h","i","j","k","l"],"weekdays":["a","b","c","d","e","f","g"],"weekStart":0"#;
    session
        .set_locale(&format!("{{{names},\"texts\":{texts}}}"))
        .expect("the locale is valid");
}

#[test]
fn a_drag_note_takes_the_hosts_words() {
    let (mut session, (x, y)) = started_session();
    speak(&mut session, r#"{"dragMoveStarted":"{name} läuft schon."}"#);
    session.pointer_down(x, y, false);
    session.pointer_move(x + 300.0, y);
    session.apply_drag();
    assert_eq!(session.drag_note(), Some("Space planning läuft schon."));
}

#[test]
fn the_printed_filter_line_takes_the_hosts_words() {
    let mut session = outline_session(3).0;
    speak(&mut session, r#"{"filterCritical":"kritisch"}"#);
    assert!(session.set_critical_only(true));
    let pages = session.print_pages("T", "2027-01-05");
    assert!(pages[0].contains("Filter: kritisch"));
    assert_eq!(session.row_filter().name(), "critical");
}
