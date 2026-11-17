//! The MSPDI `StatusDate` is the data date (gs#563).

use import::{Imported, parse_mspdi};

/// Starts on Monday 2026-01-05 08:00, so instant 0 is that Monday at 00:00.
const LINKS: &str = include_str!("fixtures/links.xml");
const DAY: i64 = 1440;

fn with_status(value: &str) -> Imported {
    let xml = LINKS.replace(
        "<StartDate>2026-01-05T08:00:00</StartDate>",
        &format!("<StartDate>2026-01-05T08:00:00</StartDate><StatusDate>{value}</StatusDate>"),
    );
    assert_ne!(xml, LINKS, "the fixture's StartDate line changed");
    parse_mspdi(&xml).expect("the fixture with a StatusDate should import")
}

fn status_notes(imported: &Imported) -> Vec<&String> {
    imported
        .unsupported
        .iter()
        .filter(|line| line.contains("status date"))
        .collect()
}

#[test]
fn a_status_date_is_the_data_date() {
    let imported = with_status("2026-01-08T16:30:00");
    assert_eq!(imported.data_date, Some(3 * DAY + 16 * 60 + 30));
    assert_eq!(status_notes(&imported), Vec::<&String>::new());
}

#[test]
fn a_file_without_a_status_date_has_no_data_date() {
    let imported = parse_mspdi(LINKS).expect("links fixture should import");
    assert_eq!(imported.data_date, None);
    assert_eq!(status_notes(&imported), Vec::<&String>::new());
}

#[test]
fn a_date_with_no_time_is_midnight_of_that_day() {
    assert_eq!(with_status("2026-01-08").data_date, Some(3 * DAY));
}

#[test]
fn a_zone_suffix_does_not_move_the_day() {
    // Late evening with an offset that a UTC conversion would carry into the next or previous day.
    let expected = Some(3 * DAY + 23 * 60 + 30);
    assert_eq!(with_status("2026-01-08T23:30:00").data_date, expected);
    assert_eq!(with_status("2026-01-08T23:30:00Z").data_date, expected);
    assert_eq!(with_status("2026-01-08T23:30:00-08:00").data_date, expected);
    assert_eq!(
        with_status("2026-01-08T00:15:00+10:00").data_date,
        Some(3 * DAY + 15)
    );
}

#[test]
fn a_status_date_before_the_start_monday_is_dropped_and_noted() {
    let imported = with_status("2026-01-02T08:00:00");
    assert_eq!(imported.data_date, None);
    assert_eq!(
        status_notes(&imported).len(),
        1,
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn an_unreadable_status_date_is_dropped_and_noted() {
    for value in [
        "NA",
        "",
        "2026-13-40T00:00:00",
        "next Tuesday",
        "2026-01-08T25:00:00",
    ] {
        let imported = with_status(value);
        assert_eq!(imported.data_date, None, "{value:?}");
        assert_eq!(
            status_notes(&imported).len(),
            1,
            "{value:?}: {:?}",
            imported.unsupported
        );
        assert_eq!(imported.tasks.len(), 6, "the tasks still import: {value:?}");
    }
}

#[test]
fn an_actual_start_after_the_status_date_is_counted_once_for_its_task() {
    // The status date is Thursday 2026-01-08 08:00: task A starts the next day, task B the day before.
    let xml = LINKS
        .replace(
            "<StartDate>2026-01-05T08:00:00</StartDate>",
            "<StartDate>2026-01-05T08:00:00</StartDate><StatusDate>2026-01-08T08:00:00</StatusDate>",
        )
        .replace(
            "<Name>A</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>",
            "<Name>A</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><ActualStart>2026-01-09T08:00:00</ActualStart>",
        )
        .replace(
            "<Name>B</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>",
            "<Name>B</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><ActualStart>2026-01-07T08:00:00</ActualStart>",
        );
    let imported = parse_mspdi(&xml).expect("the fixture with two actuals should import");
    let notes: Vec<&String> = imported
        .unsupported
        .iter()
        .filter(|line| line.contains("after the data date"))
        .collect();
    assert_eq!(notes.len(), 1, "{:?}", imported.unsupported);
    assert!(notes[0].ends_with(": 1 (first: 1)"), "{}", notes[0]);
}
