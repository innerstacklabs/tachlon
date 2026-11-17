//! `Session::set_locale`'s `texts` on a printed page (gantt#1875): the header, the legend and the
//! rows-not-printed note read in the host's words, and a key the host leaves out stays English.

use crate::common::outline_session;

#[test]
fn a_hosts_words_reach_the_header_the_legend_and_the_left_out_note() {
    let mut session = outline_session(3_000).0.with_data_date(Some(8 * 60));
    let texts = r#"{"printDataDate":"Datum: {date}","legendTask":"Aufgabe & Co","printLeftOut":"{count} Zeilen nicht gedruckt."}"#;
    session
        .set_locale(&format!(
            r#"{{"months":["a","b","c","d","e","f","g","h","i","j","k","l"],
            "weekdays":["a","b","c","d","e","f","g"],"weekStart":0,"texts":{texts}}}"#
        ))
        .unwrap();
    let pages = session.print_pages("T", "2027-01-05");
    let first = &pages[0];
    assert!(first.contains(&format!("Datum: {}", session.data_date_text().unwrap())));
    assert!(
        first.contains(">Aufgabe &amp; Co</text>"),
        "[legend, escaped]"
    );
    let left = session.print_plan().left_out;
    assert!(pages[99].contains(&format!(">{left} Zeilen nicht gedruckt.</text>")));
    assert!(first.contains(">Page 1 of 100</text>"), "[a key left out]");
    assert!(
        first.contains(">Printed 2027-01-05</text>"),
        "[a key left out]"
    );
}
