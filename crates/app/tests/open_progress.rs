//! Percent complete read from XER (`phys_complete_pct`, `complete_pct`) into `Loaded::progress`
//! (gs#326). It is display only: no computed date depends on it.

use crate::common;

use crate::document_round_trip::session_of;
use app::Session;
use common::xer_text::{loaded, rewrite, text};
use import::{ProjectSelection, parse_xer};
use xer::synth::Spec;

/// Five activities, `A1-1` to `A1-5`, in `TASK` row order.
fn base() -> String {
    text(&Spec::with_activities(3, 5))
}

/// `base` with the `TASK` type column set to `kind` and one extra `TASK` column, `name`, holding
/// `values` by row (an empty cell past the end).
fn with_column(kind: &str, name: &str, values: &[&str]) -> String {
    let typed = rewrite(&base(), "TASK", "complete_pct_type", |_, _| {
        Some(kind.to_owned())
    });
    let mut in_task = false;
    let mut row = 0;
    let mut out = String::new();
    for line in typed.split_inclusive('\n') {
        let body = line.trim_end_matches('\n');
        let mut added = None;
        if body.starts_with("%T\t") {
            in_task = body == "%T\tTASK";
        } else if in_task && body.starts_with("%F\t") {
            added = Some(format!("{body}\t{name}\n"));
        } else if in_task && body.starts_with("%R\t") {
            added = Some(format!("{body}\t{}\n", values.get(row).unwrap_or(&"")));
            row += 1;
        }
        out.push_str(added.as_deref().unwrap_or(line));
    }
    out
}

fn with_phys(values: &[&str]) -> String {
    let mut row = 0;
    let phys = rewrite(&base(), "TASK", "phys_complete_pct", |_, _| {
        row += 1;
        values.get(row - 1).map(|value| (*value).to_owned())
    });
    rewrite(&phys, "TASK", "complete_pct_type", |_, _| {
        Some("CP_Phys".to_owned())
    })
}

/// The progress of `A1-1` to `A1-5`.
fn activities(text: &str) -> (Vec<Option<u8>>, Vec<String>) {
    let imported = parse_xer(text.as_bytes(), &ProjectSelection::Unspecified).expect("imports");
    let mut found: Vec<_> = imported
        .tasks
        .iter()
        .filter_map(|task| Some((task.code.clone()?, task.progress)))
        .collect();
    found.sort();
    assert_eq!(found.len(), 5);
    (
        found.into_iter().map(|(_, progress)| progress).collect(),
        imported.unsupported,
    )
}

fn percent_notes(notes: &[String]) -> Vec<&String> {
    notes
        .iter()
        .filter(|note| note.contains("percent complete"))
        .collect()
}

#[test]
fn physical_percent_complete_is_read_and_an_empty_cell_has_none() {
    let (progress, notes) = activities(&with_phys(&["100", "50", "0", "", "33"]));
    assert_eq!(progress, [Some(100), Some(50), Some(0), None, Some(33)]);
    assert!(percent_notes(&notes).is_empty(), "{notes:?}");
}

#[test]
fn the_declared_type_picks_the_column() {
    let both = with_column("CP_Drtn", "complete_pct", &["10", "20", "", "40", "50"]);
    let (progress, notes) = activities(&both);
    assert_eq!(progress, [Some(10), Some(20), None, Some(40), Some(50)]);
    assert!(percent_notes(&notes).is_empty(), "{notes:?}");

    // The same values under CP_Phys are not read: the physical column is empty.
    let physical = with_column("CP_Phys", "complete_pct", &["10", "20", "", "40", "50"]);
    assert_eq!(activities(&physical).0, [None; 5]);
}

#[test]
fn a_type_whose_column_is_absent_gives_no_progress_and_one_note() {
    // The generated file declares CP_Drtn and has no complete_pct column.
    let (progress, notes) = activities(&base());
    assert_eq!(progress, [None; 5]);
    let found = percent_notes(&notes);
    assert_eq!(found.len(), 1, "{notes:?}");
    assert!(
        found[0].contains("duration or units percent complete"),
        "{found:?}"
    );
}

#[test]
fn a_value_outside_zero_to_one_hundred_is_clamped_and_noted_once() {
    let (progress, notes) = activities(&with_phys(&["150", "-20", "100", "7", "250"]));
    assert_eq!(
        progress,
        [Some(100), Some(0), Some(100), Some(7), Some(100)]
    );
    assert_eq!(
        percent_notes(&notes),
        [
            "A percent complete outside 0 to 100 was changed to the nearest limit; no dates are affected."
        ]
    );
}

#[test]
fn loaded_progress_follows_the_task_order_and_leaves_summaries_without() {
    let opened = loaded(
        with_phys(&["100", "50", "0", "", "33"]).as_bytes(),
        &ProjectSelection::Unspecified,
    );
    let tasks = opened.schedule.tasks().len();
    assert_eq!(opened.progress.len(), tasks);
    let mut values: Vec<u8> = opened.progress.iter().copied().filter(|&p| p > 0).collect();
    values.sort_unstable();
    assert_eq!(values, [33, 50, 100]);
}

#[test]
fn percentages_change_no_computed_date() {
    let without = loaded(base().as_bytes(), &ProjectSelection::Unspecified);
    let with = loaded(
        with_phys(&["100", "50", "0", "20", "33"]).as_bytes(),
        &ProjectSelection::Unspecified,
    );
    assert!(with.progress.iter().any(|&p| p > 0));
    assert!(without.progress.iter().all(|&p| p == 0));
    assert_eq!(with.solution.dates, without.solution.dates);
    assert_eq!(with.order, without.order);
}

/// Seven activities, `A1-6` the active one: ten days of work with eight left (a day is 8 hours),
/// and `cell` as its physical percent. The session, that task and the open's notes.
fn started(cell: &str) -> (Session, u32, Vec<String>) {
    let mut file = text(&Spec::with_activities(3, 7));
    for (column, value) in [
        ("target_drtn_hr_cnt", "80"),
        ("remain_drtn_hr_cnt", "64"),
        ("phys_complete_pct", cell),
    ] {
        file = rewrite(&file, "TASK", column, |row, _| {
            (row == 5).then(|| value.to_owned())
        });
    }
    let opened = loaded(file.as_bytes(), &ProjectSelection::Unspecified);
    let notes = opened.unsupported.clone();
    let task = opened
        .meta
        .iter()
        .position(|meta| meta.code.as_deref() == Some("A1-6"))
        .expect("A1-6 is a task");
    let task = u32::try_from(task).expect("a small index");
    (session_of(opened), task, notes)
}

#[test]
fn a_physical_percent_sits_beside_the_percent_derived_from_the_remaining_work() {
    for (cell, shown) in [
        ("50", "50%"),
        ("", ""),
        ("12.345", "12.35%"),
        ("100.4", "100%"),
    ] {
        let (session, task, notes) = started(cell);
        assert_eq!(session.task_physical(task), shown, "cell {cell:?}");
        assert_eq!(session.task_percent(task), "20%", "cell {cell:?}");
        let clamped = notes
            .iter()
            .filter(|note| note.contains("outside 0 to 100"))
            .count();
        assert_eq!(clamped, usize::from(cell == "100.4"), "{cell:?}: {notes:?}");
    }
}
