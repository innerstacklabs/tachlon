use engine::TaskId;
use import::{Imported, parse_mspdi};

fn task_id(imported: &Imported, uid: u32) -> TaskId {
    let position = imported
        .tasks
        .iter()
        .position(|task| task.uid == uid)
        .expect("fixture task UID should be imported");
    TaskId(u32::try_from(position).expect("fixture task count should fit in u32"))
}

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

// Project-start floor (gs#124): the project starts Wednesday 2026-01-07 08:00, instant 3_360 from
// the origin Monday. Every task is floored at it, a later own constraint date winning.
const FLOOR_START: &str = "2026-01-07T08:00:00";
const FLOOR_INSTANT: i64 = 3_360;

fn task_xml(uid: u32, extra: &str) -> String {
    format!(
        "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>{extra}</Task>"
    )
}

fn link_xml(predecessor: u32, kind: u32, lag_tenths: i64) -> String {
    format!(
        "<PredecessorLink><PredecessorUID>{predecessor}</PredecessorUID><Type>{kind}</Type>\
<LinkLag>{lag_tenths}</LinkLag><LagFormat>7</LagFormat></PredecessorLink>"
    )
}

fn snet_of(imported: &Imported, uid: u32) -> Option<i64> {
    imported.schedule.tasks()[task_id(imported, uid).0 as usize].snet
}

/// Task 1 unlinked, task 2 linked to it by `kind` and `lag_tenths`; returns the import.
fn linked_pair(kind: u32, lag_tenths: i64) -> Imported {
    let tasks = format!(
        "{}{}",
        task_xml(1, ""),
        task_xml(2, &link_xml(1, kind, lag_tenths))
    );
    parse_mspdi(&project(FLOOR_START, "", &tasks)).expect("linked pair should import")
}

#[test]
fn floor_ff_link_successor_at_project_start() {
    let imported = linked_pair(0, 0);
    assert_eq!(snet_of(&imported, 1), Some(FLOOR_INSTANT));
    assert_eq!(snet_of(&imported, 2), Some(FLOOR_INSTANT));
}

#[test]
fn floor_ss_link_successor_at_project_start() {
    let imported = linked_pair(3, 0);
    assert_eq!(snet_of(&imported, 2), Some(FLOOR_INSTANT));
}

#[test]
fn floor_sf_link_successor_at_project_start() {
    let imported = linked_pair(2, 0);
    assert_eq!(snet_of(&imported, 2), Some(FLOOR_INSTANT));
}

#[test]
fn floor_lead_link_successor_at_project_start() {
    // A 16-hour lead on a finish-to-start link would start the successor on Tuesday 08:00,
    // before the project starts.
    let mut imported = linked_pair(1, -9_600);
    assert_eq!(snet_of(&imported, 2), Some(FLOOR_INSTANT));
    let successor = task_id(&imported, 2);
    let solution = imported.schedule.compute().expect("lead schedule computes");
    assert_eq!(solution.get(successor).es, FLOOR_INSTANT);
}

#[test]
fn floor_overrides_an_earlier_constraint_and_keeps_a_later_one() {
    // SNET codes: 4. Tuesday 08:00 is after the origin and before the project start.
    let tasks = format!(
        "{}{}",
        task_xml(
            1,
            "<ConstraintType>4</ConstraintType><ConstraintDate>2026-01-06T08:00:00</ConstraintDate>"
        ),
        task_xml(
            2,
            "<ConstraintType>4</ConstraintType><ConstraintDate>2026-01-08T08:00:00</ConstraintDate>"
        ),
    );
    let imported = parse_mspdi(&project(FLOOR_START, "", &tasks)).expect("constraints import");
    assert_eq!(snet_of(&imported, 1), Some(FLOOR_INSTANT));
    assert_eq!(snet_of(&imported, 2), Some(4_800));
}

#[test]
fn floor_applies_to_a_summary_and_its_child_not_to_the_summary_constraint() {
    // The importer drops a summary's own constraint (gs#47) and passes nothing from it to the
    // children, so both are floored at the project start and not at the summary's later date.
    let summary = "<Task><UID>1</UID><Name>S</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary>\
<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>\
<ConstraintType>4</ConstraintType><ConstraintDate>2026-01-12T08:00:00</ConstraintDate></Task>";
    let child = "<Task><UID>2</UID><Name>C</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary>\
<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat></Task>";
    let tasks = format!("{summary}{child}");
    let imported = parse_mspdi(&project(FLOOR_START, "", &tasks)).expect("summary imports");
    assert_eq!(snet_of(&imported, 1), Some(FLOOR_INSTANT));
    assert_eq!(snet_of(&imported, 2), Some(FLOOR_INSTANT));
}

#[test]
fn a_project_without_a_start_date_is_noted_and_not_floored() {
    let tasks = format!(
        "{}{}",
        task_xml(1, ""),
        task_xml(
            2,
            "<ConstraintType>4</ConstraintType><ConstraintDate>2000-01-06T08:00:00</ConstraintDate>"
        ),
    );
    let xml = project(FLOOR_START, "", &tasks)
        .replace(&format!("<StartDate>{FLOOR_START}</StartDate>"), "");
    assert!(!xml.contains("<StartDate>"));
    let imported = parse_mspdi(&xml).expect("a project without StartDate imports");
    assert!(
        imported
            .unsupported
            .contains(&"Project: the file has no start date, so the schedule starts on Monday 2000-01-03 and every date follows from that.".to_owned())
    );
    assert_eq!(snet_of(&imported, 1), None);
    assert_eq!(snet_of(&imported, 2), Some(4_800));
}
