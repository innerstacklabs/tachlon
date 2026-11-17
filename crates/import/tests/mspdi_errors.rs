use engine::TaskId;
use import::parse_mspdi;

const LINKS: &str = include_str!("fixtures/links.xml");

const WEEK: &str = "<WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>\
<WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>3</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>4</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>5</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>6</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay>";

/// A one-calendar Mon-Fri project with the given exceptions and task elements.
fn project(start: &str, exceptions: &str, tasks: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>{start}</StartDate><CalendarUID>1</CalendarUID><Calendars><Calendar><UID>1</UID>\
<Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>\
<WeekDays>{WEEK}</WeekDays><Exceptions>{exceptions}</Exceptions></Calendar></Calendars>\
<Tasks>{tasks}</Tasks></Project>"
    )
}

fn one_task(extra: &str) -> String {
    format!(
        "<Task><UID>1</UID><Name>T</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>{extra}</Task>"
    )
}

fn exception(entered: u32, from: &str, to: &str, extra: &str) -> String {
    format!(
        "<Exception><EnteredByOccurrences>{entered}</EnteredByOccurrences><TimePeriod>\
<FromDate>{from}T00:00:00</FromDate><ToDate>{to}T23:59:00</ToDate></TimePeriod>{extra}\
<DayWorking>0</DayWorking></Exception>"
    )
}

#[test]
fn recurring_exceptions_are_reported_and_one_off_holidays_still_map() {
    let exceptions = [
        // Yearly Christmas without the Month and MonthDay fields it needs.
        exception(0, "2026-12-25", "2030-12-25", "<Type>2</Type>"),
        // The second Friday of every month: a weekday of the month is not expanded.
        exception(
            0,
            "2026-01-09",
            "2026-03-13",
            "<Type>5</Type><Period>1</Period><MonthItem>7</MonthItem><MonthPosition>1</MonthPosition>",
        ),
        // Weekly with no day of the week named.
        exception(0, "2026-02-02", "2026-02-20", "<Type>6</Type><Period>1</Period>"),
        // What MS Project writes for an ordinary one-day holiday, in both variants.
        exception(
            1,
            "2026-01-12",
            "2026-01-12",
            "<Occurrences>1</Occurrences><Type>1</Type>",
        ),
        exception(0, "2026-01-13", "2026-01-14", "<Type>1</Type>"),
        exception(0, "2026-01-15", "2026-01-15", ""),
    ]
    .concat();
    let xml = project("2026-01-05T08:00:00", &exceptions, &one_task(""));
    let imported = parse_mspdi(&xml).expect("recurring exception project should import");

    assert_eq!(
        imported
            .schedule
            .calendars()
            .first()
            .expect("project should import its calendar")
            .holidays(),
        [7, 8, 9, 10]
    );
    assert_eq!(
        imported.unsupported,
        [
            "Calendar 1: a recurring exception was not read; working days can differ from the file's."
        ]
    );
}

#[test]
fn manual_actuals_and_leveling_fields_are_reported_per_task() {
    let tasks = [
        "<Task><UID>1</UID><OutlineLevel>1</OutlineLevel><Duration>PT8H0M0S</Duration><Manual>1</Manual></Task>",
        "<Task><UID>2</UID><OutlineLevel>1</OutlineLevel><Duration>PT8H0M0S</Duration><PercentComplete>40</PercentComplete><ActualStart>2026-01-05T08:00:00</ActualStart></Task>",
        "<Task><UID>3</UID><OutlineLevel>1</OutlineLevel><Duration>PT8H0M0S</Duration><LevelingDelay>4800</LevelingDelay></Task>",
        "<Task><UID>4</UID><OutlineLevel>1</OutlineLevel><Duration>PT8H0M0S</Duration><ActualDuration>PT4H0M0S</ActualDuration></Task>",
        // MS Project writes these defaults on an untouched task: no note.
        "<Task><UID>5</UID><OutlineLevel>1</OutlineLevel><Duration>PT8H0M0S</Duration><Manual>0</Manual><PercentComplete>0</PercentComplete><ActualDuration>PT0H0M0S</ActualDuration><LevelingDelay>0</LevelingDelay></Task>",
    ]
    .concat();
    let xml = project("2026-01-05T08:00:00", "", &tasks);
    let imported = parse_mspdi(&xml).expect("tracked project should import");

    assert_eq!(
        imported.unsupported,
        [
            "Tasks scheduled manually in the file: they were imported as automatically scheduled, so their dates can differ from the file's: 1 (first: 1)",
            "Tasks with a leveling delay: the delay was not read, so their dates can differ from the file's: 1 (first: 3)",
            "Task 4: the actual duration was not read because the task has no actual dates; no dates are affected.",
        ]
    );
}

#[test]
fn a_milestone_with_a_duration_reports_the_dropped_duration() {
    let tasks = [
        "<Task><UID>1</UID><OutlineLevel>1</OutlineLevel><Milestone>1</Milestone><Duration>PT16H0M0S</Duration></Task>",
        "<Task><UID>2</UID><OutlineLevel>1</OutlineLevel><Milestone>1</Milestone><Duration>PT0H0M0S</Duration></Task>",
    ]
    .concat();
    let xml = project("2026-01-05T08:00:00", "", &tasks);
    let imported = parse_mspdi(&xml).expect("milestone project should import");

    assert_eq!(
        imported.unsupported,
        [
            "Task 1: the milestone's duration of 960 minutes was not read; a milestone takes no time, so dates after it can differ from the file's."
        ]
    );
    assert!(
        imported
            .schedule
            .tasks()
            .iter()
            .all(|task| task.duration == 0)
    );
}

#[test]
fn a_negative_utc_offset_is_accepted() {
    let xml = project("2026-01-05T08:00:00-07:00", "", &one_task(""));
    let imported = parse_mspdi(&xml).expect("negative offset should import");
    assert_eq!(imported.unsupported, Vec::<String>::new());
}

#[test]
fn truncated_xml_is_an_xml_error() {
    assert!(matches!(
        parse_mspdi("<Project><Tasks>"),
        Err(import::ImportError::Xml { .. })
    ));
}

#[test]
fn mismatched_tags_are_an_xml_error() {
    assert!(matches!(
        parse_mspdi("<Project><Tasks></Project>"),
        Err(import::ImportError::Xml { .. })
    ));
}

#[test]
fn empty_input_is_an_xml_error() {
    assert!(matches!(
        parse_mspdi(""),
        Err(import::ImportError::Xml { .. })
    ));
}

#[test]
fn plain_text_is_an_xml_error() {
    assert!(matches!(
        parse_mspdi("not XML"),
        Err(import::ImportError::Xml { .. })
    ));
}

#[test]
fn a_non_project_root_is_not_a_schedule() {
    let error = parse_mspdi("<NotProject />").expect_err("the Project root should be required");

    assert_eq!(
        error,
        import::ImportError::NotASchedule {
            what: "an XML file whose root is NotProject".to_owned(),
        }
    );
    assert!(error.to_string().contains("NotProject"));
}

#[test]
fn a_link_to_a_missing_task_is_dropped_with_a_note() {
    let tasks = "<Task><UID>1</UID><OutlineLevel>1</OutlineLevel><Duration>PT8H0M0S</Duration></Task>\
        <Task><UID>7</UID><OutlineLevel>1</OutlineLevel><Duration>PT8H0M0S</Duration>\
        <PredecessorLink><PredecessorUID>99</PredecessorUID></PredecessorLink>\
        <PredecessorLink><PredecessorUID>1</PredecessorUID></PredecessorLink></Task>";
    let xml = project("2026-01-05T08:00:00", "", tasks);
    let imported = parse_mspdi(&xml).expect("a link to a task outside the file should not fail");

    assert_eq!(imported.schedule.dependencies().len(), 1);
    assert_eq!(
        imported.unsupported,
        [
            "Task 7: a link from task 99 was dropped because that task is not in the file; the task is not held back by it, so its dates can differ from the file's."
        ]
    );
}

#[test]
fn duplicate_task_uids_are_rejected() {
    let xml = "<Project><Tasks><Task><UID>7</UID></Task>\
        <Task><UID>7</UID></Task></Tasks></Project>";

    assert_eq!(
        parse_mspdi(xml).expect_err("duplicate UIDs should be rejected"),
        import::ImportError::DuplicateUid { uid: 7 }
    );
}

#[test]
fn an_unknown_link_type_is_invalid() {
    let xml = "<Project><Tasks><Task><UID>1</UID></Task><Task><UID>2</UID>\
        <PredecessorLink><PredecessorUID>1</PredecessorUID><Type>9</Type>\
        </PredecessorLink></Task></Tasks></Project>";
    let error = parse_mspdi(xml).expect_err("link type 9 should be invalid");

    assert_eq!(
        error,
        import::ImportError::Invalid {
            element: "Type".to_owned(),
            value: "9".to_owned(),
        }
    );
    assert!(error.to_string().contains("Type"));
}

#[test]
fn a_non_iso_duration_is_invalid() {
    let xml = "<Project><Tasks><Task><UID>1</UID><Duration>8 hours</Duration>\
        </Task></Tasks></Project>";
    let error = parse_mspdi(xml).expect_err("the duration should be ISO 8601");

    assert_eq!(
        error,
        import::ImportError::Invalid {
            element: "Duration".to_owned(),
            value: "8 hours".to_owned(),
        }
    );
    assert!(error.to_string().contains("Duration"));
}

#[test]
fn a_task_without_a_uid_is_missing_uid() {
    let error = parse_mspdi("<Project><Tasks><Task /></Tasks></Project>")
        .expect_err("a task UID should be required");

    assert_eq!(
        error,
        import::ImportError::Missing {
            element: "UID".to_owned(),
        }
    );
    assert!(error.to_string().contains("UID"));
}

#[test]
fn a_self_link_is_a_schedule_error() {
    let xml = "<Project><Tasks><Task><UID>1</UID><PredecessorLink>\
        <PredecessorUID>1</PredecessorUID></PredecessorLink></Task></Tasks></Project>";

    assert_eq!(
        parse_mspdi(xml).expect_err("a task should not depend on itself"),
        import::ImportError::Schedule(engine::ScheduleError::SelfDependency(TaskId(0)))
    );
}

#[test]
fn one_hundred_levels_of_nesting_are_too_deep() {
    let xml = format!("{}{}", "<x>".repeat(100), "</x>".repeat(100));

    assert!(matches!(
        parse_mspdi(&xml),
        Err(import::ImportError::TooDeep)
    ));
}

#[test]
fn an_ms_project_binary_file_says_to_save_it_as_xml() {
    let mut bytes = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    bytes.extend_from_slice(&[0; 512]);

    let error = import::parse_bytes(&bytes, &import::ProjectSelection::default())
        .expect_err("a binary file is not XML");

    assert_eq!(
        error.to_string(),
        "this is an MS Project binary file (.mpp); in MS Project choose File > Save As, pick the XML format, and open that file"
    );
}

#[test]
fn truncating_the_links_fixture_never_panics() {
    for offset in (0..=LINKS.len()).step_by(7) {
        if !LINKS.is_char_boundary(offset) {
            continue;
        }
        let result = std::panic::catch_unwind(|| parse_mspdi(&LINKS[..offset]));
        assert!(result.is_ok(), "parser panicked at byte offset {offset}");
    }
}
