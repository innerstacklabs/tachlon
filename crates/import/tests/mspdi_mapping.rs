use engine::{LinkType, TaskId};
use import::{Imported, ImportedTask, parse_mspdi};
use std::fmt::Write as _;

const LINKS: &str = include_str!("fixtures/links.xml");
const HOLIDAYS: &str = include_str!("fixtures/holidays.xml");
const SUMMARY: &str = include_str!("fixtures/summary.xml");
const CONSTRAINTS: &str = include_str!("fixtures/constraints.xml");
const COSTS: &str = include_str!("fixtures/costs.xml");
const BASELINE: &str = include_str!("fixtures/baseline.xml");

fn imported_task(imported: &Imported, uid: u32) -> &ImportedTask {
    imported
        .tasks
        .iter()
        .find(|task| task.uid == uid)
        .expect("fixture task UID should be imported")
}

fn task_id(imported: &Imported, uid: u32) -> TaskId {
    let position = imported
        .tasks
        .iter()
        .position(|task| task.uid == uid)
        .expect("fixture task UID should be imported");
    TaskId(u32::try_from(position).expect("fixture task count should fit in u32"))
}

fn uid(imported: &Imported, id: TaskId) -> u32 {
    imported
        .tasks
        .get(id.0 as usize)
        .expect("engine task ID should have imported metadata")
        .uid
}

#[test]
fn imported_task_metadata_follows_document_order() {
    let imported = parse_mspdi(LINKS).expect("links fixture should import");
    let metadata: Vec<_> = imported
        .tasks
        .iter()
        .map(|task| {
            (
                task.uid,
                task.name.as_str(),
                task.outline_level,
                task.parent,
            )
        })
        .collect();

    assert_eq!(
        metadata,
        [
            (1, "A", 1, None),
            (2, "B", 1, None),
            (3, "C", 1, None),
            (4, "D", 1, None),
            (5, "E", 1, None),
            (6, "F", 1, None),
        ]
    );
}

#[test]
fn link_kinds_and_lags_are_mapped() {
    let imported = parse_mspdi(LINKS).expect("links fixture should import");
    let dependencies: Vec<_> = imported
        .schedule
        .dependencies()
        .iter()
        .map(|dependency| {
            (
                uid(&imported, dependency.pred),
                uid(&imported, dependency.succ),
                dependency.kind,
                dependency.lag,
            )
        })
        .collect();

    assert_eq!(
        dependencies,
        [
            (1, 2, LinkType::FS, 480),
            (1, 3, LinkType::SS, 480),
            (2, 4, LinkType::FF, 0),
            (3, 5, LinkType::SF, 1440),
            (4, 6, LinkType::FS, 0),
            (5, 6, LinkType::FS, 0),
        ]
    );
}

#[test]
fn six_task_schedule_has_the_hand_checked_dates() {
    let mut imported = parse_mspdi(LINKS).expect("links fixture should import");
    let ids: Vec<_> = (1..=6).map(|uid| task_id(&imported, uid)).collect();
    let solution = imported
        .schedule
        .compute()
        .expect("links fixture should compute");
    let expected = [
        (480, 2400, 0),
        (4800, 11040, 0),
        (1920, 3840, 960),
        (10560, 11040, 0),
        (4800, 5280, 960),
        (12000, 12480, 0),
    ];

    for (id, (es, ef, float)) in ids.iter().copied().zip(expected) {
        let dates = solution.get(id);
        assert_eq!((dates.es, dates.ef, dates.float), (es, ef, float));
    }
    assert_eq!(solution.project_finish, 12480);
    let critical: Vec<_> = ids
        .iter()
        .copied()
        .filter(|&id| solution.get(id).is_critical())
        .map(|id| uid(&imported, id))
        .collect();
    assert_eq!(critical, [1, 2, 4, 6]);
    assert_eq!(imported.unsupported, Vec::<String>::new());
}

#[test]
fn old_and_new_holidays_delay_work() {
    let mut imported = parse_mspdi(HOLIDAYS).expect("holidays fixture should import");
    let id = task_id(&imported, 1);

    assert_eq!(
        imported
            .schedule
            .calendars()
            .first()
            .expect("fixture should import its calendar")
            .holidays(),
        [2, 4]
    );
    let solution = imported
        .schedule
        .compute()
        .expect("holidays fixture should compute");
    let dates = solution.get(id);
    assert_eq!((dates.es, dates.ef), (480, 11040));
    assert_eq!(imported.unsupported, Vec::<String>::new());
}

#[test]
fn summary_parents_and_child_dates_are_imported() {
    let mut imported = parse_mspdi(SUMMARY).expect("summary fixture should import");
    let summary_one = task_id(&imported, 1);
    let child_two = task_id(&imported, 2);
    let child_three = task_id(&imported, 3);
    let summary_four = task_id(&imported, 4);
    let child_five = task_id(&imported, 5);

    assert_eq!(imported_task(&imported, 1).parent, None);
    assert_eq!(imported_task(&imported, 2).parent, Some(summary_one));
    assert_eq!(imported_task(&imported, 3).parent, Some(summary_one));
    assert_eq!(imported_task(&imported, 4).parent, None);
    assert_eq!(imported_task(&imported, 5).parent, Some(summary_four));
    assert_eq!(
        imported
            .schedule
            .tasks()
            .get(summary_one.0 as usize)
            .expect("first summary should exist")
            .duration,
        0
    );
    assert_eq!(
        imported
            .schedule
            .tasks()
            .get(summary_four.0 as usize)
            .expect("second summary should exist")
            .duration,
        0
    );
    assert_eq!(imported.unsupported, Vec::<String>::new());

    let solution = imported
        .schedule
        .compute()
        .expect("summary fixture should compute");
    assert_eq!(
        (solution.get(child_two).es, solution.get(child_two).ef),
        (480, 960)
    );
    assert_eq!(
        (solution.get(child_three).es, solution.get(child_three).ef),
        (1920, 2400)
    );
    assert_eq!(
        (solution.get(child_five).es, solution.get(child_five).ef),
        (3360, 3840)
    );
}

#[test]
fn constraints_and_task_date_changers_are_reported() {
    let mut imported = parse_mspdi(CONSTRAINTS).expect("constraints fixture should import");
    let snet = task_id(&imported, 1);
    let expected_snet = 9 * 1440 + 480;

    assert_eq!(
        imported
            .schedule
            .tasks()
            .get(snet.0 as usize)
            .expect("SNET task should exist")
            .snet,
        Some(expected_snet)
    );
    assert_eq!(
        imported.unsupported,
        [
            "Task 3: the finish no later than constraint (type 7) has no date and was not read; the task is scheduled without it, so its dates can differ from the file's.",
            "1 task has resource assignments; only the resource names were read, and no dates are affected.",
        ]
    );
    assert!(
        imported
            .unsupported
            .iter()
            .all(|entry| !entry.starts_with("task 1:"))
    );

    let solution = imported
        .schedule
        .compute()
        .expect("constraints fixture should compute");
    assert_eq!(solution.get(snet).es, expected_snet);
}

#[test]
fn a_link_that_touches_a_summary_task_adds_no_note() {
    let original = "<Name>Second summary</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary><Duration>PT0S</Duration><DurationFormat>7</DurationFormat></Task>";
    let replacement = "<Name>Second summary</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary><Duration>PT0S</Duration><DurationFormat>7</DurationFormat><PredecessorLink><PredecessorUID>3</PredecessorUID><Type>1</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink></Task>";
    let patched = SUMMARY.replacen(original, replacement, 1);
    assert_ne!(patched, SUMMARY);

    let imported = parse_mspdi(&patched).expect("the patched fixture should import");
    // The importer keeps the link; the app drops it and names it once (`load_bytes`).
    assert_eq!(imported.schedule.dependencies().len(), 3);
    assert_eq!(imported.unsupported, Vec::<String>::new());
}

// Elapsed lags (gs#703): a link whose `LagFormat` is an elapsed unit counts its lag in all
// minutes. MS Project writes 2 elapsed days as `<LinkLag>28800</LinkLag>` (2 880 minutes in
// tenths) with `<LagFormat>8</LagFormat>`; no MS Project-written sample is in the repository, so
// the tests build the element from that rule, as `mspdi_elapsed.rs` does for durations.
const WORKING_DAY: &str = "<WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes>";

/// A Mon-Fri 08:00 to 16:00 project of two 1-day tasks starting on Thursday 2026-01-08, task 2
/// linked finish-to-start to task 1 with the given lag element text.
fn lag_project(lag: &str, lag_format: u32) -> String {
    let mut week =
        String::from("<WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>");
    for day in 2..=6 {
        write!(
            week,
            "<WeekDay><DayType>{day}</DayType><DayWorking>1</DayWorking>{WORKING_DAY}</WeekDay>"
        )
        .expect("a String takes writes");
    }
    week.push_str("<WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay>");
    format!(
        "<?xml version=\"1.0\"?><Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>2026-01-08T08:00:00</StartDate><CalendarUID>1</CalendarUID><Calendars><Calendar>\
<UID>1</UID><Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>\
<WeekDays>{week}</WeekDays></Calendar></Calendars><Tasks>\
<Task><UID>1</UID><Name>A</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat></Task>\
<Task><UID>2</UID><Name>B</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>\
<PredecessorLink><PredecessorUID>1</PredecessorUID><Type>1</Type><LinkLag>{lag}</LinkLag><LagFormat>{lag_format}</LagFormat></PredecessorLink></Task>\
</Tasks></Project>"
    )
}

/// Task 2's start and the dependency's lag calendar, after solving.
fn successor_start(xml: &str) -> (i64, Option<engine::LagCalendar>) {
    let mut imported = parse_mspdi(xml).expect("the lag project should import");
    let successor = task_id(&imported, 2);
    let calendar = imported.schedule.dependency_lag_calendar(engine::DepId(0));
    let solution = imported
        .schedule
        .compute()
        .expect("the lag project should compute");
    (solution.get(successor).es, calendar)
}

// Task 1 runs Thursday 08:00 to 16:00 (instant 5 280 from the origin Monday 00:00).
#[test]
fn a_two_day_elapsed_lag_counts_calendar_days_across_a_weekend() {
    let (start, calendar) = successor_start(&lag_project("28800", 8));
    // 5 280 + 2 880 elapsed minutes is Saturday 16:00; the successor starts Monday 08:00.
    assert_eq!(start, 7 * 1_440 + 480);
    assert_eq!(calendar, Some(engine::LagCalendar::TwentyFourHour));
}

#[test]
fn a_two_day_working_lag_still_counts_working_days() {
    let (start, calendar) = successor_start(&lag_project("9600", 7));
    // Two working days of 480 minutes: Friday and Monday, so Monday 16:00 ends the lag.
    assert!(
        start > 7 * 1_440 + 480,
        "working lag ends Monday 16:00: {start}"
    );
    assert_eq!(calendar, None);
}

#[test]
fn an_elapsed_percent_lag_stays_a_dropped_percent_lag() {
    let imported = parse_mspdi(&lag_project("500", 20)).expect("the lag project should import");
    assert_eq!(imported.schedule.dependencies()[0].lag, 0);
    assert_eq!(
        imported.schedule.dependency_lag_calendar(engine::DepId(0)),
        None
    );
}

#[test]
fn a_task_carries_its_wbs_text_and_a_blank_or_missing_one_is_none() {
    let xml = SUMMARY
        .replace(
            "<UID>1</UID><Name>First summary",
            "<UID>1</UID><WBS>1</WBS><Name>First summary",
        )
        .replace(
            "<UID>2</UID><Name>First child",
            "<UID>2</UID><WBS> 1.1 </WBS><Name>First child",
        )
        .replace(
            "<UID>3</UID><Name>Second child",
            "<UID>3</UID><WBS> </WBS><Name>Second child",
        );
    let imported = parse_mspdi(&xml).expect("summary fixture should import");
    let codes: Vec<_> = imported
        .tasks
        .iter()
        .map(|task| (task.uid, task.wbs_code.as_deref()))
        .collect();
    assert_eq!(
        codes,
        [
            (1, Some("1")),
            (2, Some("1.1")),
            (3, None),
            (4, None),
            (5, None)
        ]
    );
}

#[test]
fn many_assigned_tasks_give_one_resource_note() {
    let mut tasks = String::new();
    let mut assignments = String::new();
    for uid in 1..=600 {
        write!(
            tasks,
            "<Task><UID>{uid}</UID><Duration>PT8H0M0S</Duration></Task>"
        )
        .expect("a String takes writes");
        write!(
            assignments,
            "<Assignment><TaskUID>{uid}</TaskUID><ResourceUID>7</ResourceUID></Assignment>"
        )
        .expect("a String takes writes");
    }
    let xml = format!(
        "<Project><StartDate>2026-01-05T08:00:00</StartDate><Tasks>{tasks}</Tasks>\
<Assignments>{assignments}</Assignments></Project>"
    );
    let imported = parse_mspdi(&xml).expect("the project should import");
    let resource_notes: Vec<_> = imported
        .unsupported
        .iter()
        .filter(|note| note.contains("resource"))
        .collect();
    assert_eq!(
        resource_notes,
        [
            "600 tasks have resource assignments; only the resource names were read, and no dates are affected."
        ]
    );
}

fn cost_notes(xml: &str) -> Vec<String> {
    let imported = parse_mspdi(xml).expect("the project should import");
    let mut notes = imported.unsupported;
    notes.retain(|note| note.contains("a cost;"));
    notes
}

/// Three tasks state a `Cost`, a `FixedCost` or an `ActualCost`; a summary states a rolled-up cost
/// and a task with zeros states none, so neither counts.
#[test]
fn a_file_with_task_costs_gets_one_cost_note() {
    assert_eq!(
        cost_notes(COSTS),
        ["3 tasks state a cost; costs were not read, and no dates are affected."]
    );
}

#[test]
fn an_all_zero_cost_file_gets_no_cost_note() {
    assert_eq!(cost_notes(BASELINE), Vec::<String>::new());
}
