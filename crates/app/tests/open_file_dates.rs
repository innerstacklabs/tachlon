//! `load_bytes`: the file's dates against the engine's, and the line it adds (gs#87 PR 1).

use crate::common;

use app::Loaded;
use common::xer_text::{MINUTES_PER_DAY, loaded, minutes_of, rewrite, stamp_of, text};
use import::{ProjectSelection, parse_xer};
use xer::synth::{Spec, write};

/// The line `load_bytes` adds, if any.
fn file_dates_line(opened: &Loaded) -> Option<&String> {
    opened
        .unsupported
        .iter()
        .find(|line| line.starts_with("in-progress activities:"))
}

/// What the line should say, computed from the XER rows and the solved engine dates with none of
/// the importer's code: the file's early dates against the engine's, in-progress activities only.
struct Oracle {
    compared: usize,
    differing: Vec<String>,
}

fn oracle(bytes: &[u8], opened: &Loaded) -> Oracle {
    let parsed = xer::parse(bytes).expect("parse");
    let data = xer::read(&parsed).expect("read");
    let origin_day = {
        let first = data
            .project_rows
            .iter()
            .filter_map(|project| project.plan_start_date)
            .min()
            .expect("a plan start");
        let day = first.div_euclid(MINUTES_PER_DAY);
        day - (day + 3).rem_euclid(7)
    };
    let origin = origin_day * MINUTES_PER_DAY;
    let mut compared = 0;
    let mut differing = Vec::new();
    for (index, meta) in opened.meta.iter().enumerate() {
        let Some(code) = meta.code.as_deref() else {
            continue;
        };
        let row = data
            .task_rows
            .iter()
            .find(|row| row.task_code.as_deref() == Some(code))
            .expect("every code is a TASK row");
        if row.status_code != Some(xer::StatusCode::Active) {
            continue;
        }
        let dates = &opened.solution.dates[index];
        let day = |minutes: i64| minutes.div_euclid(MINUTES_PER_DAY);
        let start = row.early_start_date.expect("synth states early_start_date") - origin;
        let finish = row.early_end_date.expect("synth states early_end_date") - origin;
        compared += 1;
        if day(start) != day(dates.es) || day(finish) != day(dates.ef) {
            differing.push(code.to_string());
        }
    }
    Oracle {
        compared,
        differing,
    }
}

/// The generated file as P6 writes an activity under way: the early start of every in-progress
/// activity at its actual start, and the early finish of every second one at the finish the
/// engine computes with the actuals applied (the engine's finish does not depend on the file's
/// early dates).
fn with_early_dates_at_actuals(spec: &Spec) -> Vec<u8> {
    let original = text(spec);
    let parsed = xer::parse(original.as_bytes()).expect("parse");
    let data = xer::read(&parsed).expect("read");
    let active = |row: usize| data.task_rows[row].status_code == Some(xer::StatusCode::Active);
    let starts = rewrite(&original, "TASK", "early_start_date", |row, _| {
        active(row).then(|| {
            stamp_of(
                data.task_rows[row]
                    .act_start_date
                    .expect("synth states act_start_date"),
            )
        })
    });
    let opened = loaded(starts.as_bytes(), &ProjectSelection::Unspecified);
    let origin = {
        let first = data.project_rows[0].plan_start_date.expect("plan start");
        let day = first.div_euclid(MINUTES_PER_DAY);
        (day - (day + 3).rem_euclid(7)) * MINUTES_PER_DAY
    };
    let mut seen = 0;
    rewrite(&starts, "TASK", "early_end_date", |row, _| {
        if !active(row) {
            return None;
        }
        seen += 1;
        let code = data.task_rows[row].task_code.as_deref()?;
        let index = opened
            .meta
            .iter()
            .position(|m| m.code.as_deref() == Some(code))?;
        (seen % 2 == 0).then(|| stamp_of(opened.solution.dates[index].ef + origin))
    })
    .into_bytes()
}

#[test]
fn the_file_dates_line_counts_what_the_oracle_counts() {
    for seed in [3, 7, 21] {
        let bytes = with_early_dates_at_actuals(&Spec::small(seed));
        let opened = loaded(&bytes, &ProjectSelection::Unspecified);
        let expected = oracle(&bytes, &opened);
        assert!(
            expected.compared > 0,
            "the fixture has in-progress activities"
        );
        // The actuals are applied: the engine starts each in-progress activity where the file
        // does, so what still differs is only the finish.
        assert!(
            expected.differing.len() < expected.compared,
            "seed {seed}: an activity with actuals agrees with the file"
        );

        let first: Vec<_> = expected.differing.iter().take(5).cloned().collect();
        let line = format!(
            "in-progress activities: {} of {} show different dates in the file than the engine computed (first: {})",
            expected.differing.len(),
            expected.compared,
            first.join(", ")
        );
        assert!(!expected.differing.is_empty(), "and some of them differ");
        assert_eq!(file_dates_line(&opened), Some(&line), "seed {seed}");
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test counts every changed end of the file"
)]
fn a_file_that_agrees_with_the_engine_has_no_line_and_each_changed_end_is_counted() {
    let spec = Spec::small(7);
    let original = text(&spec);
    let bytes = original.as_bytes();
    let opened = loaded(bytes, &ProjectSelection::Unspecified);
    let expected = oracle(bytes, &opened);
    assert!(expected.compared >= 4);

    // The engine's own dates, written back as the file's early dates of every in-progress row.
    let parsed = xer::parse(bytes).expect("parse");
    let data = xer::read(&parsed).expect("read");
    let origin = {
        let first = data.project_rows[0].plan_start_date.expect("plan start");
        let day = first.div_euclid(MINUTES_PER_DAY);
        (day - (day + 3).rem_euclid(7)) * MINUTES_PER_DAY
    };
    let engine_dates: Vec<(String, i64, i64)> = opened
        .meta
        .iter()
        .enumerate()
        .filter_map(|(index, meta)| {
            let code = meta.code.clone()?;
            let dates = &opened.solution.dates[index];
            Some((code, dates.es + origin, dates.ef + origin))
        })
        .collect();
    let active_codes: Vec<String> = data
        .task_rows
        .iter()
        .filter(|row| row.status_code == Some(xer::StatusCode::Active))
        .filter_map(|row| row.task_code.clone())
        .collect();
    assert_eq!(active_codes.len(), expected.compared);

    let code_of_row: Vec<String> = data
        .task_rows
        .iter()
        .map(|row| {
            row.task_code
                .clone()
                .expect("synth gives every activity a code")
        })
        .collect();
    // The file's early dates for every in-progress row set to the engine's, then each listed
    // code's start or finish moved by the stated minutes.
    let agree = |starts: &[(&str, i64)], finishes: &[(&str, i64)]| {
        let moved = |text: &str, field: &str, finish: bool, nudges: &[(&str, i64)]| {
            rewrite(text, "TASK", field, |row, _| {
                let code = &code_of_row[row];
                if !active_codes.contains(code) {
                    return None;
                }
                let (_, es, ef) = engine_dates.iter().find(|(c, ..)| c == code).expect("code");
                let mut minutes = if finish { *ef } else { *es };
                if let Some((_, by)) = nudges.iter().find(|(c, _)| c == code) {
                    minutes += by;
                }
                Some(stamp_of(minutes))
            })
        };
        let text = moved(&original, "early_start_date", false, starts);
        moved(&text, "early_end_date", true, finishes)
    };
    let line_for = |text: &str| {
        let opened = loaded(text.as_bytes(), &ProjectSelection::Unspecified);
        (file_dates_line(&opened).cloned(), opened)
    };

    let (line, opened) = line_for(&agree(&[], &[]));
    assert_eq!(line, None, "{:?}", opened.unsupported);

    // A shift inside the same calendar day is not a different date.
    let (line, _) = line_for(&agree(&[(&active_codes[0], 1)], &[(&active_codes[1], -1)]));
    assert_eq!(line, None);

    // Three in-progress activities differ: one only at its start, one only at its finish, one
    // at both. Each counts once, and the first IDs follow the outline order.
    let (start_only, finish_only, both) = (
        active_codes[1].as_str(),
        active_codes[2].as_str(),
        active_codes[3].as_str(),
    );
    let day = 3 * MINUTES_PER_DAY;
    let (line, opened) = line_for(&agree(
        &[(start_only, day), (both, day)],
        &[(finish_only, day), (both, day)],
    ));
    let in_outline_order: Vec<&str> = opened
        .meta
        .iter()
        .filter_map(|meta| meta.code.as_deref())
        .filter(|code| [start_only, finish_only, both].contains(code))
        .collect();
    assert_eq!(in_outline_order.len(), 3);
    assert_eq!(
        line,
        Some(format!(
            "in-progress activities: 3 of {} show different dates in the file than the engine computed (first: {})",
            expected.compared,
            in_outline_order.join(", ")
        ))
    );
}

#[test]
fn a_file_with_actuals_has_no_actuals_ignored_note() {
    let bytes = write(&Spec::small(7));
    let opened = loaded(&bytes, &ProjectSelection::Unspecified);
    assert!(
        !opened
            .unsupported
            .iter()
            .any(|line| line.contains("actual dates ignored") || line.contains("not applied")),
        "{:?}",
        opened.unsupported
    );
}

#[test]
fn not_started_and_complete_activities_and_mspdi_are_not_compared() {
    // Every activity is made not started: nothing is in progress, so there is no line.
    let none_active = rewrite(&text(&Spec::small(7)), "TASK", "status_code", |_, _| {
        Some("TK_NotStart".to_string())
    });
    let opened = loaded(none_active.as_bytes(), &ProjectSelection::Unspecified);
    assert_eq!(file_dates_line(&opened), None);

    // Every activity is in progress but states no early dates: nothing to compare.
    let undated = rewrite(
        &rewrite(&text(&Spec::small(7)), "TASK", "status_code", |_, _| {
            Some("TK_Active".to_string())
        }),
        "TASK",
        "early_start_date",
        |_, _| Some(String::new()),
    );
    let undated = rewrite(&undated, "TASK", "early_end_date", |_, _| {
        Some(String::new())
    });
    let opened = loaded(undated.as_bytes(), &ProjectSelection::Unspecified);
    assert_eq!(file_dates_line(&opened), None);

    let mspdi = include_str!("../../import/tests/fixtures/summary.xml");
    assert_eq!(
        file_dates_line(&loaded(mspdi.as_bytes(), &ProjectSelection::All)),
        None
    );
}

#[test]
fn the_importer_keeps_the_files_early_dates_from_the_origin_without_clamping() {
    let bytes = write(&Spec::small(7));
    let imported = parse_xer(&bytes, &ProjectSelection::Unspecified).expect("import");
    let parsed = xer::parse(&bytes).expect("parse");
    let data = xer::read(&parsed).expect("read");
    let plan_start = data.project_rows[0].plan_start_date.expect("plan start");
    // The generator's 2024-01-01 is a Monday, so the origin is that midnight.
    let origin = plan_start.div_euclid(MINUTES_PER_DAY) * MINUTES_PER_DAY;

    let mut seen = 0;
    for task in imported.tasks.iter().filter(|task| task.code.is_some()) {
        let row = data
            .task_rows
            .iter()
            .find(|row| row.task_code == task.code)
            .expect("row");
        assert_eq!(task.file_start, row.early_start_date.map(|m| m - origin));
        assert_eq!(task.file_finish, row.early_end_date.map(|m| m - origin));
        assert_eq!(
            task.in_progress,
            row.status_code == Some(xer::StatusCode::Active)
        );
        seen += 1;
    }
    assert_eq!(seen, 40);
    assert!(
        imported
            .tasks
            .iter()
            .filter(|task| task.code.is_none())
            .all(|task| {
                task.file_start.is_none() && task.file_finish.is_none() && !task.in_progress
            })
    );

    // A file date before the origin stays negative; it is not floored to the origin.
    let early = rewrite(
        &text(&Spec::small(7)),
        "TASK",
        "early_start_date",
        |row, _| (row == 0).then(|| "2023-12-20 08:00".to_string()),
    );
    let imported = parse_xer(early.as_bytes(), &ProjectSelection::Unspecified).expect("import");
    let first = imported
        .tasks
        .iter()
        .find(|task| task.code.is_some())
        .expect("an activity");
    assert_eq!(
        first.file_start,
        Some(minutes_of("2023-12-20 08:00") - origin)
    );
    assert!(first.file_start.unwrap() < 0);
}
