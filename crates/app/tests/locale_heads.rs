//! Printed column heads use `set_locale` words while CSV keeps its English heads (gantt#1875).

use crate::common::outline_session;

#[test]
fn printed_heads_use_the_hosts_words_but_csv_does_not() {
    let mut session = outline_session(3).0;
    session
        .set_locale(
            r#"{"months":["a","b","c","d","e","f","g","h","i","j","k","l"],
            "weekdays":["a","b","c","d","e","f","g"],"weekStart":0,
            "texts":{"headTask":"Aufgabe & Co","headStart":"Beginn","headFinish":"Ende"}}"#,
        )
        .unwrap();

    let pages = session.print_pages("T", "2027-01-05");
    let first = &pages[0];
    assert!(first.contains(">Aufgabe &amp; Co</text>"));
    assert!(first.contains(">Beginn</text>"));
    assert!(first.contains(">Ende</text>"));
    assert!(first.contains(">Dur</text>"));

    let csv = session.list_csv();
    let head = csv.lines().next().unwrap();
    assert!(
        ["Task", "Start", "Finish"]
            .iter()
            .all(|word| head.split(',').any(|cell| cell == *word))
    );
    assert!(!head.contains("Aufgabe") && !head.contains("Beginn"));
}
