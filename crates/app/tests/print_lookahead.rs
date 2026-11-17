//! The printed page names what limits its rows (gs#703): the Lookahead window, and the value a
//! code or status filter has on.

use crate::common;

use app::{PageSize, Session, outline::Outline, outline::demo_layout, status_filter::StatusKind};
use common::{clock, outline_session};
use import::ActivityCode;
use timeline::Timeline;

/// A session over the demo layout whose tasks all carry Trade = Framing, so the code filter holds
/// them and the printed pages are many.
fn framing_session(value: &str) -> Session {
    let mut layout = demo_layout(60);
    for meta in &mut layout.meta {
        meta.activity_codes = vec![ActivityCode {
            code_type: "Trade".to_owned(),
            value: value.to_owned(),
        }];
    }
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        clock,
    )
    .with_outline(outline)
    .expect("a matching outline")
}

#[test]
fn the_page_names_the_window() {
    let mut session = outline_session(60).0;
    for page in session.print_pages("Plan", "") {
        assert!(!page.contains("Lookahead"), "no window, no text");
    }

    let start = session.lookahead_start_text();
    assert!(session.set_lookahead(session.parse_lookahead_day(&start).unwrap(), 52));
    let pages = session.print_pages("Plan", "");
    assert!(pages.len() > 1, "the window keeps rows for several pages");
    let expected = format!("Lookahead: {start}, 52 weeks");
    for page in &pages {
        assert!(page.contains(&expected), "{expected}");
    }

    let day = session.parse_lookahead_day("2027-02-05").unwrap();
    assert!(session.set_lookahead(day, 3));
    assert!(session.print_pages("Plan", "")[0].contains("Lookahead: 2027-02-05, 3 weeks"));
    assert!(session.set_lookahead(day, 1));
    assert!(session.print_pages("Plan", "")[0].contains("Lookahead: 2027-02-05, 1 week<"));

    assert!(session.clear_lookahead());
    assert!(!session.print_pages("Plan", "")[0].contains("Lookahead"));
}

#[test]
fn a_code_filter_prints_its_value() {
    let mut session = framing_session("Framing");
    assert!(session.set_activity_code_filter("Trade", "Framing"));
    let page = &session.print_pages("Plan", "")[0];
    assert!(page.contains("Filter: code Trade = Framing"), "{page}");
}

#[test]
fn a_long_code_value_prints_cut() {
    let long = "Framing ".repeat(40);
    let mut session = framing_session(&long);
    assert!(session.set_activity_code_filter("Trade", &long));
    let page = &session.print_pages("Plan", "2027-02-05")[0];
    assert!(!page.contains(&long), "the header line is cut");
    assert!(page.contains("Filter: code Trade = Framing"), "{page}");
    assert!(page.contains("…</text>"), "{page}");
}

#[test]
fn a_status_filter_prints_its_value() {
    let mut session = outline_session(60).0;
    assert!(session.set_status_filter(Some(StatusKind::InProgress)));
    let page = &session.print_pages("Plan", "")[0];
    assert!(page.contains("Filter: status in-progress"), "{page}");
}

#[test]
fn a_layout_in_use_is_named_on_every_page() {
    let session = framing_session("Framing");
    let on = |layout| session.print_pages_on("Plan", "", PageSize::Letter, None, None, layout);
    let pages = on(Some("Lookahead"));
    assert!(pages.len() > 1);
    assert!(pages.iter().all(|page| page.contains("Layout: Lookahead")));
    for layout in [None, Some(""), Some("  ")] {
        assert!(!on(layout)[0].contains("Layout:"));
    }
}
