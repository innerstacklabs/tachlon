//! A print layout saved as data (gs#703): written as JSON, read back, applied to a second file,
//! and the parts that file cannot take named in a report.

use crate::common::{self, baseline, clock};
use app::{
    Session,
    layout::Layout,
    outline::{Outline, demo_layout},
};
use import::ActivityCode;
use timeline::Timeline;

/// A session over `demo_layout(n)` with its outline; with `coded`, every task carries Phase = Civil.
fn session(n: usize, coded: bool) -> Session {
    let mut layout = demo_layout(n);
    for meta in layout.meta.iter_mut().filter(|_| coded) {
        meta.activity_codes = vec![ActivityCode {
            code_type: "Phase".to_owned(),
            value: "Civil".to_owned(),
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

fn window() -> (String, String) {
    ("2026-01-13".to_owned(), "2026-01-16".to_owned())
}

fn columns() -> Vec<String> {
    ["Task", "Start", "Finish"].map(str::to_owned).to_vec()
}

#[test]
fn a_layout_read_back_and_applied_gives_a_second_file_the_same_setup() {
    let mut first = session(40, true);
    assert!(first.set_activity_code_filter("Phase", "Civil"));
    let monday = first.parse_lookahead_day("2026-01-12").expect("a day");
    assert!(first.set_lookahead(monday, 2));
    let saved = first
        .layout("Weekly", "a4", Some(columns()), Some(window()))
        .expect("a layout");

    let read = Layout::from_json(&saved.to_json()).expect("the text reads back");
    assert_eq!(read, saved);
    assert_eq!(read.columns, Some(columns()));

    let mut second = session(25, true);
    assert_eq!(second.apply_layout(&read), Vec::<String>::new());
    assert_eq!(second.row_filter(), first.row_filter());
    assert_eq!(second.lookahead(), first.lookahead());
    let again = second.layout("Weekly", "a4", Some(columns()), Some(window()));
    assert_eq!(again, Ok(saved));
}

#[test]
fn every_part_the_second_file_cannot_take_is_reported_and_left_alone() {
    let mut first = session(40, true);
    assert!(first.set_activity_code_filter("Phase", "Civil"));
    let wanted = ["ID", "Task", "Start variance"].map(str::to_owned).to_vec();
    let saved = first
        .layout("Weekly", "letter", Some(wanted), None)
        .expect("a layout");

    let (mut second, _) = common::outline_session(25);
    let report = second.apply_layout(&saved);
    assert_eq!(
        report,
        [
            "column ID: not applied, the file has no such column",
            "column Start variance: not applied, the file has no such column",
            "filter code Phase Civil: not applied, no task in the file carries that code",
        ]
    );
    assert_eq!(second.row_filter(), app::RowFilter::Off);
}

#[test]
fn a_baseline_filter_with_no_baseline_is_reported() {
    let mut first = session(40, false);
    baseline::set(&mut first);
    assert!(first.set_slipped_only(true));
    let saved = first
        .layout("Slipped", "letter", None, None)
        .expect("a layout");

    let mut second = session(25, false);
    assert_eq!(
        second.apply_layout(&saved),
        ["filter slipped: not applied, the file has no baseline"]
    );
}

#[test]
fn text_that_is_not_a_layout_is_refused_and_a_bad_paper_is_not_saved() {
    let saved = session(5, false)
        .layout("n", "letter", None, None)
        .expect("a layout");
    let extra = saved
        .to_json()
        .replace("\"version\":1", "\"version\":1,\"extra\":0");
    assert_eq!(
        Layout::from_json(&extra),
        Err("extra: unknown key".to_owned())
    );
    assert!(Layout::from_json("{").is_err());
    assert!(session(5, false).layout("n", "legal", None, None).is_err());
    assert!(
        session(5, false)
            .layout(
                "n",
                "a4",
                None,
                Some(("2026-01-16".into(), "2026-01-13".into()))
            )
            .is_err()
    );
}
