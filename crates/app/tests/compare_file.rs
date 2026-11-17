//! Matching last month's file to the open one, task by task (gs#703, row C1 of the compare-two-files
//! plan): `Session::compare_file` reads a second parsed file and returns a baseline of it, keyed by
//! activity ID for an XER file and by task UID for an MS Project file.

use app::edit::Edit;
use app::{CompareReport, FILE_ONLY_MAX, Loaded, Session, fitted_timeline, load_bytes};
use engine::{DAY, TaskId};
use import::ProjectSelection;
use std::sync::Arc;
use xer::synth::{Spec, write};

const BASELINED: &str = include_str!("fixtures/baselined.xml");
const NO_SHARED_ID: &str = "no task has the same ID in both files";

/// `baselined.xml`'s calendar with these tasks, `(uid, name, hours)`, and the project starting on
/// `start` (a Monday, `2026-01-05` in the fixture).
fn file(start: &str, tasks: &[(u32, &str, u32)]) -> String {
    let head = &BASELINED[..BASELINED.find("<Tasks>").expect("the fixture has tasks")];
    let head = head.replace("2026-01-05T08:00:00", &format!("{start}T08:00:00"));
    let rows: String = tasks
        .iter()
        .map(|(uid, name, hours)| {
            format!(
                "<Task><UID>{uid}</UID><Name>{name}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
                 <Duration>PT{hours}H0M0S</Duration><DurationFormat>7</DurationFormat></Task>"
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!("{head}<Tasks>{rows}</Tasks></Project>")
}

fn load(bytes: &[u8]) -> Loaded {
    load_bytes(bytes, &ProjectSelection::Unspecified, &|| 0.0).expect("loads")
}

fn session_of(loaded: &Loaded) -> Session {
    let timeline = fitted_timeline(loaded.order.clone(), &loaded.solution, 1000.0, 400.0, 1.0);
    Session::new(
        loaded.schedule.clone(),
        loaded.solution.clone(),
        timeline,
        loaded.meta.clone(),
        || 0.0,
    )
    .with_origin(loaded.origin)
}

fn compare(
    session: &Session,
    other: &Loaded,
) -> Result<app::baseline::Baseline, app::CompareError> {
    session.compare_file(&other.schedule, &other.solution, &other.meta, other.origin)
}

#[test]
fn a_task_that_moved_reads_as_slipped_matched_by_uid_not_by_position() {
    // Last month Dug took one day and Poured two. This month Poured is listed first and Dug
    // takes two, so by position Poured would be read against Dug's old dates and not slip.
    let last = load(file("2026-01-05", &[(1, "Dug", 8), (2, "Poured", 16)]).as_bytes());
    let now = load(file("2026-01-05", &[(2, "Poured", 16), (1, "Dug", 16)]).as_bytes());
    let session = session_of(&now);
    let (poured, dug) = (TaskId(0), TaskId(1));

    let baseline = compare(&session, &last).expect("both files are MS Project files");

    let (last_dug, last_poured) = (last.solution.get(TaskId(0)), last.solution.get(TaskId(1)));
    let held_dug = baseline.task(dug).expect("Dug is in both files");
    let held_poured = baseline.task(poured).expect("Poured is in both files");
    assert_eq!((held_dug.es, held_dug.ef), (last_dug.es, last_dug.ef));
    assert_eq!(
        (held_poured.es, held_poured.ef),
        (last_poured.es, last_poured.ef)
    );
    assert_eq!(
        held_dug.float,
        Some(last_dug.float),
        "last month's float, as solved there"
    );
    assert_eq!(baseline.project_finish(), last.solution.project_finish);

    let now_dug = session.solution().get(dug);
    assert!(
        now_dug.ef > held_dug.ef,
        "Dug finishes later than last month's file said"
    );
    assert_eq!(
        session.solution().get(poured).ef,
        held_poured.ef,
        "Poured did not move"
    );

    let summary = session
        .with_baseline(baseline)
        .baseline_diff()
        .expect("a baseline is set");
    assert_eq!(summary.slipped, 1, "Dug alone");
}

#[test]
fn the_other_files_dates_move_by_the_difference_of_the_two_origins() {
    // This month's project starts a week later, so the same instants are a week apart on the
    // calendar: last month's Dug started on Jan 5 and now starts on Jan 12.
    let last = load(file("2026-01-05", &[(1, "Dug", 8)]).as_bytes());
    let now = load(file("2026-01-12", &[(1, "Dug", 8)]).as_bytes());
    let session = session_of(&now);
    assert_eq!(now.origin.day() - last.origin.day(), 7);

    let baseline = compare(&session, &last).expect("same file format");

    let held = baseline.task(TaskId(0)).expect("Dug is in both files");
    assert_eq!(session.date_text(held.es), "2026-01-05 08:00");
    assert_eq!(held.es, last.solution.get(TaskId(0)).es - 7 * DAY);
    assert_eq!(
        baseline.project_finish(),
        last.solution.project_finish - 7 * DAY
    );
}

#[test]
fn a_task_with_no_partner_in_the_other_file_has_no_entry() {
    let last = load(file("2026-01-05", &[(1, "Dug", 8), (3, "Gone", 8)]).as_bytes());
    let now = load(file("2026-01-05", &[(1, "Dug", 8), (2, "Poured", 8)]).as_bytes());

    let baseline = compare(&session_of(&now), &last).expect("Dug is in both");

    assert_eq!(baseline.len(), 1);
    assert!(baseline.task(TaskId(0)).is_some());
    assert!(baseline.task(TaskId(1)).is_none(), "Poured is new");
}

#[test]
fn an_xer_file_is_matched_by_activity_id() {
    let bytes = write(&Spec::small(5));
    let (last, now) = (load(&bytes), load(&bytes));
    let session = session_of(&now);

    let baseline = compare(&session, &last).expect("both files are XER files");

    let activities = now.meta.iter().filter(|task| task.code.is_some()).count();
    assert!(activities > 0);
    assert_eq!(
        baseline.len(),
        activities,
        "every activity has the same ID in both"
    );
    let first = now
        .meta
        .iter()
        .position(|task| task.code.is_some())
        .expect("an activity");
    let held = baseline.task(TaskId(first as u32)).expect("an entry");
    assert_eq!(held.es, session.solution().get(TaskId(first as u32)).es);
}

#[test]
fn a_file_of_another_format_is_refused() {
    let xer = load(&write(&Spec::small(5)));
    let mspdi = load(file("2026-01-05", &[(1, "Dug", 8), (2, "Poured", 8)]).as_bytes());

    let against_xer = compare(&session_of(&xer), &mspdi).expect_err("MS Project against XER");
    let against_mspdi = compare(&session_of(&mspdi), &xer).expect_err("XER against MS Project");

    assert_eq!(against_xer.to_string(), NO_SHARED_ID);
    assert_eq!(against_mspdi.to_string(), NO_SHARED_ID);
}

#[test]
fn two_files_of_one_format_that_share_no_id_are_refused() {
    let last = load(file("2026-01-05", &[(7, "Elsewhere", 8)]).as_bytes());
    let now = load(file("2026-01-05", &[(1, "Dug", 8)]).as_bytes());

    let error = compare(&session_of(&now), &last).expect_err("no UID is in both");

    assert_eq!(error.to_string(), NO_SHARED_ID);
}

#[test]
fn the_summary_counts_the_files_unmatched_tasks_as_removed() {
    // Last month's file has Gone and Also gone as well; this month's has neither, and has Poured.
    let last = load(
        file(
            "2026-01-05",
            &[(1, "Dug", 8), (3, "Gone", 8), (4, "Also gone", 8)],
        )
        .as_bytes(),
    );
    let now = load(file("2026-01-05", &[(1, "Dug", 8), (2, "Poured", 8)]).as_bytes());
    let session = session_of(&now);

    let baseline = compare(&session, &last).expect("Dug is in both");
    let summary = session
        .with_baseline(baseline)
        .baseline_diff()
        .expect("a baseline is set");

    assert_eq!(
        summary.removed, 2,
        "Gone and Also gone are only in the older file"
    );
    assert_eq!(summary.added, 1, "Poured is only in this one");
}

#[test]
fn setting_the_file_as_baseline_is_one_undo_step_and_undo_hands_back_the_old_baseline() {
    // Dug is a day longer now, Gone is only in the older file, Poured only in this one.
    let last = load(file("2026-01-05", &[(1, "Dug", 8), (3, "Gone", 8)]).as_bytes());
    let now = load(file("2026-01-05", &[(1, "Dug", 16), (2, "Poured", 8)]).as_bytes());
    let mut session = session_of(&now);
    session
        .edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let old = Arc::clone(session.baseline().expect("it is in force"));

    let report = session
        .set_baseline_from_file(&last.schedule, &last.solution, &last.meta, last.origin)
        .expect("Dug is in both");

    let counts = CompareReport {
        matched: 1,
        only_in_file: 1,
        only_here: 1,
        file_only: vec!["3 Gone".to_string()],
        here_only: vec!["2 Poured".to_string()],
        file_only_links: vec![],
        here_only_links: vec![],
    };
    assert_eq!(report, counts);
    assert_eq!(
        report.json(),
        r#"{"matched":1,"only_in_file":1,"only_here":1,"lines":["Only in the older file: 1","Only in this file: 1"],"file_only":["3 Gone"],"here_only":["2 Poured"],"file_only_links":[],"here_only_links":[]}"#
    );
    let summary = session.baseline_diff().expect("the file is the baseline");
    assert_eq!((summary.slipped, summary.added, summary.removed), (1, 1, 1));

    assert!(session.undo().unwrap(), "one undo takes the file back");
    assert!(Arc::ptr_eq(session.baseline().unwrap(), &old));
    assert!(session.undo().unwrap(), "the next undo is the step before");
    assert!(session.baseline().is_none());
    assert!(session.redo().unwrap() && session.redo().unwrap());
    assert_eq!(session.baseline_diff().unwrap().slipped, 1);
}

#[test]
fn a_file_that_matches_nothing_sets_no_baseline_and_no_step() {
    let last = load(file("2026-01-05", &[(7, "Other", 8)]).as_bytes());
    let now = load(file("2026-01-05", &[(1, "Dug", 8)]).as_bytes());
    let mut session = session_of(&now);

    let error = session
        .set_baseline_from_file(&last.schedule, &last.solution, &last.meta, last.origin)
        .expect_err("no UID is in both");

    assert_eq!(error.to_string(), NO_SHARED_ID);
    assert!(session.baseline().is_none());
    assert!(!session.undo().unwrap(), "no step was recorded");
}

#[test]
fn the_file_only_lines_name_the_id_and_the_name_in_file_order_and_cap() {
    // Last month's file lists Dug and then 202 tasks this month's lacks, their UIDs falling, so the
    // file's order is neither the UID order nor the name order.
    let names: Vec<String> = (0..202).map(|n| format!("Gone {n}")).collect();
    let mut tasks = vec![(1, "Dug", 8)];
    tasks.extend(
        names
            .iter()
            .enumerate()
            .map(|(n, name)| (5000 - n as u32, name.as_str(), 8)),
    );
    let last = load(file("2026-01-05", &tasks).as_bytes());
    let now = load(file("2026-01-05", &[(1, "Dug", 8)]).as_bytes());
    let mut session = session_of(&now);

    let report = session
        .set_baseline_from_file(&last.schedule, &last.solution, &last.meta, last.origin)
        .expect("Dug is in both");

    assert_eq!(report.only_in_file, 202);
    assert_eq!(report.file_only.len(), FILE_ONLY_MAX + 1);
    assert_eq!(report.file_only[0], "5000 Gone 0");
    assert_eq!(report.file_only[1], "4999 Gone 1");
    assert_eq!(report.file_only[FILE_ONLY_MAX - 1], "4801 Gone 199");
    assert_eq!(report.file_only[FILE_ONLY_MAX], "and 2 more");

    // The same list again, in the host's words.
    assert!(session.undo().expect("undoes"));
    let locale = serde_json::json!({"months": vec!["m"; 12], "weekdays": vec!["d"; 7], "weekStart": 0, "texts": {"compareMore": "und {count} weitere"}});
    session.set_locale(&locale.to_string()).expect("a locale");
    let report = session
        .set_baseline_from_file(&last.schedule, &last.solution, &last.meta, last.origin)
        .expect("Dug is in both");
    assert_eq!(report.file_only[FILE_ONLY_MAX], "und 2 weitere");
}

#[test]
fn an_xer_files_only_lines_carry_the_activity_id_and_a_name_with_a_quote_is_escaped() {
    let (last, now) = (
        load(&write(&Spec::with_activities(5, 40))),
        load(&write(&Spec::with_activities(5, 38))),
    );
    let mut session = session_of(&now);
    let report = session
        .set_baseline_from_file(&last.schedule, &last.solution, &last.meta, last.origin)
        .expect("the first 38 activities are in both");
    assert_eq!(report.file_only, ["A1-39 Activity 39", "A1-40 Activity 40"]);
    assert!(
        report.here_only.is_empty(),
        "every activity here is in the older file"
    );

    let quoted = CompareReport {
        matched: 0,
        only_in_file: 1,
        only_here: 0,
        file_only: vec![r#"7 The "slab""#.to_string()],
        here_only: vec![],
        file_only_links: vec![],
        here_only_links: vec![],
    };
    assert!(
        quoted
            .json()
            .contains(r#""file_only":["7 The \"slab\""],"here_only":[]"#)
    );
}

#[test]
fn the_here_only_lines_name_the_tasks_only_this_file_has_in_its_order_and_cap() {
    // This month's file lists Dug and then 202 tasks last month's lacks, their UIDs falling, so
    // this file's order is neither the UID order nor the name order.
    let names: Vec<String> = (0..202).map(|n| format!("New {n}")).collect();
    let mut tasks = vec![(1, "Dug", 8)];
    tasks.extend(
        names
            .iter()
            .enumerate()
            .map(|(n, name)| (5000 - n as u32, name.as_str(), 8)),
    );
    let last = load(file("2026-01-05", &[(1, "Dug", 8), (3, "Gone", 8)]).as_bytes());
    let now = load(file("2026-01-05", &tasks).as_bytes());
    let mut session = session_of(&now);

    let report = session
        .set_baseline_from_file(&last.schedule, &last.solution, &last.meta, last.origin)
        .expect("Dug is in both");

    assert_eq!(report.only_here, 202);
    assert_eq!(report.here_only.len(), FILE_ONLY_MAX + 1);
    assert_eq!(report.here_only[0], "5000 New 0");
    assert_eq!(report.here_only[FILE_ONLY_MAX - 1], "4801 New 199");
    assert_eq!(report.here_only[FILE_ONLY_MAX], "and 2 more");
    assert_eq!(
        report.file_only,
        ["3 Gone"],
        "the file's own list is unchanged"
    );
}

/// A file of tasks `(uid, predecessors)`, each predecessor `(uid, MS Project link type)`: 1 is
/// finish-to-start and 3 is start-to-start.
fn linked_file(tasks: &[(u32, &[(u32, u32)])]) -> Loaded {
    let rows = tasks
        .iter()
        .map(|(uid, preds)| {
            let links = preds
                .iter()
                .map(|(pred, kind)| {
                    format!("<PredecessorLink><PredecessorUID>{pred}</PredecessorUID><Type>{kind}</Type></PredecessorLink>")
                })
                .collect::<Vec<_>>()
                .concat();
            format!(
                "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
                 <Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>{links}</Task>"
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let head = &BASELINED[..BASELINED.find("<Tasks>").expect("the fixture has tasks")];
    load(format!("{head}<Tasks>{rows}</Tasks></Project>").as_bytes())
}

#[test]
fn the_link_lines_match_links_by_ids_and_type_and_name_each_side_in_its_own_order() {
    // 1 -> 2 and 1 -> 4 are in both files. 2 -> 3 changed type (a link of another type is another
    // link), 3 -> 4 is gone and 2 -> 4 is new.
    let last = linked_file(&[
        (1, &[]),
        (2, &[(1, 1)]),
        (3, &[(2, 1)]),
        (4, &[(3, 1), (1, 3)]),
    ]);
    let now = linked_file(&[
        (1, &[]),
        (2, &[(1, 1)]),
        (3, &[(2, 3)]),
        (4, &[(2, 1), (1, 3)]),
    ]);
    let mut session = session_of(&now);

    let report = session
        .set_baseline_from_file(&last.schedule, &last.solution, &last.meta, last.origin)
        .expect("all four tasks are in both");

    assert_eq!((report.only_in_file, report.only_here), (0, 0));
    assert_eq!(report.file_only_links, ["2 → 3 (FS)", "3 → 4 (FS)"]);
    assert_eq!(report.here_only_links, ["2 → 3 (SS)", "2 → 4 (FS)"]);
}
