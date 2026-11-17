//! `load_bytes`: the data date a file opens with (gs#87 PR 1).

use crate::common;

use common::xer_text::{FIRST_PROJECT, MINUTES_PER_DAY, loaded, rewrite, text, three_projects};
use import::{ProjectSelection, parse_xer};
use xer::synth::{Spec, write};

fn set_recalc(text: &str, dates: &[&str]) -> String {
    rewrite(text, "PROJECT", "last_recalc_date", |row, _| {
        dates.get(row).map(|date| (*date).to_string())
    })
}

#[test]
fn the_data_date_is_the_projects_last_recalc_date_from_the_origin_monday() {
    // plan_start_date is 2024-01-01 08:00 (a Monday), so the origin is 2024-01-01 00:00.
    let base = text(&Spec::small(3));
    let at_plan_start = loaded(base.as_bytes(), &ProjectSelection::Unspecified);
    assert_eq!(at_plan_start.data_date, Some(8 * 60));

    let thursday = set_recalc(&base, &["2024-01-04 16:30"]);
    let opened = loaded(thursday.as_bytes(), &ProjectSelection::Unspecified);
    assert_eq!(opened.data_date, Some(3 * MINUTES_PER_DAY + 16 * 60 + 30));
    assert_eq!(
        opened.origin.stamp(opened.data_date.unwrap()),
        "2024-01-04 16:30"
    );

    let imported = parse_xer(thursday.as_bytes(), &ProjectSelection::Unspecified).expect("import");
    assert_eq!(imported.data_date, opened.data_date);
}

#[test]
fn a_data_date_before_the_origin_is_none_and_noted() {
    let early = set_recalc(&text(&Spec::small(3)), &["2023-12-29 08:00"]);
    let opened = loaded(early.as_bytes(), &ProjectSelection::Unspecified);
    assert_eq!(opened.data_date, None);
    assert!(
        opened
            .unsupported
            .iter()
            .any(|line| line == "The data date is before the schedule start and was not read; progress is measured without it, so dates can differ from the file's: 1 (first: 1000)"),
        "{:?}",
        opened.unsupported
    );

    // A project with no last_recalc_date states no data date and adds no note.
    let blank = set_recalc(&text(&Spec::small(3)), &[""]);
    let opened = loaded(blank.as_bytes(), &ProjectSelection::Unspecified);
    assert_eq!(opened.data_date, None);
    assert!(
        opened
            .unsupported
            .iter()
            .all(|line| !line.starts_with("The data date is before")
                && !line.contains("different data dates"))
    );
}

#[test]
fn the_data_date_of_several_projects_is_the_latest_and_each_is_named_when_they_differ() {
    let bytes = write(&three_projects());
    // The generator puts project n's last_recalc_date at 2024-01-(5n + 1) 08:00.
    let opened = loaded(&bytes, &ProjectSelection::All);
    assert_eq!(opened.data_date, Some(10 * MINUTES_PER_DAY + 8 * 60));
    let line = opened
        .unsupported
        .iter()
        .find(|line| line.starts_with("Projects have different data dates"))
        .expect("the differing dates are noted");
    assert!(
        line.starts_with("Projects have different data dates; the latest is used for all of them, so dates can differ from the file's: 3 (first: ")
    );
    for named in [
        "1000 2024-01-01 08:00",
        "1001 2024-01-06 08:00",
        "1002 2024-01-11 08:00",
    ] {
        assert!(line.contains(named), "{line}");
    }

    // One project of the three: its own date, measured from the origin of the projects opened.
    let second = loaded(&bytes, &ProjectSelection::Only(vec![FIRST_PROJECT + 1]));
    assert_eq!(second.data_date, Some(5 * MINUTES_PER_DAY + 8 * 60));
    assert!(
        second
            .unsupported
            .iter()
            .all(|line| !line.contains("different data dates"))
    );

    // Equal dates: no note.
    let same = set_recalc(
        &text(&three_projects()),
        &["2024-01-08 08:00", "2024-01-08 08:00", "2024-01-08 08:00"],
    );
    let opened = loaded(same.as_bytes(), &ProjectSelection::All);
    assert_eq!(opened.data_date, Some(7 * MINUTES_PER_DAY + 8 * 60));
    assert!(
        opened
            .unsupported
            .iter()
            .all(|line| !line.contains("different data dates"))
    );
}

#[test]
fn mspdi_has_no_data_date() {
    for fixture in [
        include_str!("../../import/tests/fixtures/constraints.xml"),
        include_str!("../../import/tests/fixtures/links.xml"),
        include_str!("../../import/tests/fixtures/summary.xml"),
    ] {
        assert_eq!(
            loaded(fixture.as_bytes(), &ProjectSelection::All).data_date,
            None
        );
    }
}
