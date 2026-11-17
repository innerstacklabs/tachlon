use engine::Schedule;
use xer::{CalendarType, ProjectRow};

use super::XerCalendars;
use crate::notes::Notes;
use crate::units::days_from_civil;

const MONDAY_AND_SUNDAY: &str = concat!(
    "(0||CalendarData()((0||DaysOfWeek()(",
    "(0||1()((0||0(s|23:00|f|00:00)())))",
    "(0||2()((0||0(s|08:00|f|09:00)())))",
    "))))"
);

const BASE_WEEK: &str = "A calendar states no working week of its own, so its base calendar's week is used; working days can differ from the file's";

fn invalid_period(id: u32) -> String {
    format!(
        "Calendar {id}: a working period is invalid or overlaps another and was not read; working hours can differ from the file's: 1"
    )
}

fn holidays_note(id: u32) -> String {
    format!(
        "Calendar {id}: holidays listed on its base calendar are not applied to it; working days can differ from the file's: 1"
    )
}

#[test]
fn maps_p6_weekdays_and_midnight_finish() {
    let data = data(&[(1, None, "CA_Project", MONDAY_AND_SUNDAY)]);
    let (schedule, notes) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    let calendar = &schedule.calendars()[0];
    assert_eq!(calendar.week()[0], [(480, 540)]);
    assert_eq!(calendar.week()[6], [(1_380, 1_440)]);
    assert_eq!(notes.finish(), Vec::<String>::new());
}

#[test]
fn keeps_only_whole_day_exceptions_on_or_after_origin() {
    let blob = concat!(
        "(0||CalendarData()((0||DaysOfWeek()(",
        "(0||2()((0||0(s|08:00|f|17:00)())))",
        "))(0||Exceptions()(",
        "(0||0(d|45291)())(0||1(d|45292)())",
        "))))"
    );
    let data = data(&[(1, None, "CA_Project", blob)]);
    let (schedule, _) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].holidays(), [0]);
}

fn week_blob(exceptions: &str) -> String {
    format!(
        concat!(
            "(0||CalendarData()((0||DaysOfWeek()(",
            "(0||2()((0||0(s|08:00|f|17:00)())))",
            "))(0||Exceptions()({}))))"
        ),
        exceptions
    )
}

#[test]
fn working_exception_is_applied_without_a_note() {
    // 45292 is Monday 2024-01-01 (day 0); 45297 is Saturday 2024-01-06 (day 5).
    let blob = week_blob(
        "(0||0(d|45292)((0||0(s|09:00|f|12:00)())))(0||1(d|45297)((0||0(s|08:00|f|12:00)())))",
    );
    let data = data(&[(1, None, "CA_Project", &blob)]);
    let (schedule, notes) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    let calendar = &schedule.calendars()[0];
    assert_eq!(
        calendar.exceptions(),
        [(0, vec![(540, 720)]), (5, vec![(480, 720)])]
    );
    assert_eq!(calendar.holidays(), []);
    assert_eq!(notes.finish(), Vec::<String>::new());
}

#[test]
fn working_exception_before_origin_is_ignored() {
    let blob = week_blob("(0||0(d|45291)((0||0(s|09:00|f|12:00)())))");
    let data = data(&[(1, None, "CA_Project", &blob)]);
    let (schedule, notes) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].exceptions(), []);
    assert_eq!(notes.finish(), Vec::<String>::new());
}

#[test]
fn working_exception_periods_are_sorted() {
    let blob = week_blob(
        "(0||0(d|45297)((0||0(s|13:00|f|17:00)())(0||1(s|08:00|f|12:00)())(0||2(s|11:00|f|14:00)())))",
    );
    let data = data(&[(1, None, "CA_Project", &blob)]);
    let (schedule, _) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    // The xer layer drops the overlapping 11:00 to 14:00 period and
    // reports it; the import sees the sorted survivors.
    assert_eq!(
        schedule.calendars()[0].exceptions(),
        [(5, vec![(480, 720), (780, 1_020)])]
    );
}

#[test]
fn convert_periods_drops_an_overlap_and_counts_it() {
    let source = [
        xer::Period {
            start: 480,
            finish: 720,
        },
        xer::Period {
            start: 660,
            finish: 780,
        },
        xer::Period {
            start: 780,
            finish: 1_020,
        },
    ];
    let mut notes = Notes::new();
    assert_eq!(
        super::convert_periods(&source, 1, &mut notes),
        [(480, 720), (780, 1_020)]
    );
    assert!(
        notes
            .finish()
            .iter()
            .any(|line| line.contains("a working period is invalid or overlaps another and was not read; working hours can differ from the file's"))
    );
}

#[test]
fn duplicate_dates_keep_the_first_and_are_noted() {
    let blob = week_blob(concat!(
        "(0||0(d|45297)((0||0(s|08:00|f|12:00)())))",
        "(0||1(d|45297)((0||0(s|09:00|f|10:00)())))",
        "(0||2(d|45293)())(0||3(d|45293)())"
    ));
    let data = data(&[(1, None, "CA_Project", &blob)]);
    let (schedule, notes) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    let calendar = &schedule.calendars()[0];
    assert_eq!(calendar.exceptions(), [(1, vec![]), (5, vec![(480, 720)])]);
    let lines = notes.finish();
    assert!(
        // The repeat of 45293 is identical, so only the differing repeat of 45297 counts.
        lines.iter().any(
            |line| line.contains("two exceptions fall on one date and only one was read; working days can differ from the file's")
                && line.ends_with(": 1")
        ),
        "{lines:?}"
    );
}

#[test]
fn missing_week_uses_its_base_week() {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let data = data(&[
        (1, None, "CA_Base", MONDAY_AND_SUNDAY),
        (2, Some(1), "CA_Project", no_week),
    ]);
    let (schedule, notes) = build_default(&data, 2, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 540)]);
    let lines = notes.finish();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("states no working week of its own") && line.contains('2')),
        "{lines:?}"
    );
}

#[test]
fn unrecognized_calendar_type_is_imported_and_counted() {
    let data = data(&[(1, None, "CA_Weird", MONDAY_AND_SUNDAY)]);
    let (schedule, notes) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars().len(), 1);
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 540)]);
    let lines = notes.finish();
    assert!(
        lines.iter().any(|line| line.contains("CA_Weird")
            && line.contains("it was read as an ordinary calendar")),
        "{lines:?}"
    );
}

#[test]
fn missing_week_and_base_uses_fallback_and_note() {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let data = data(&[(1, None, "CA_Project", no_week)]);
    let (schedule, notes) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 1_020)]);
    assert!(
        notes
            .finish()
            .iter()
            .any(|line| line.contains("states no working week"))
    );
}

#[test]
fn resource_calendar_is_not_built() {
    let data = data(&[
        (1, None, "CA_Project", MONDAY_AND_SUNDAY),
        (2, None, "CA_Rsrc", MONDAY_AND_SUNDAY),
    ]);
    let mut schedule = Schedule::new();
    let mut notes = Notes::new();
    let project = project(1);
    let mut calendars = XerCalendars::new(&data, days_from_civil(2024, 1, 1));
    let default = calendars
        .project_default(&project, &mut schedule, &mut notes)
        .expect("project calendar");
    let selected = calendars
        .for_activity(Some(2), default, "A1", &mut schedule, &mut notes)
        .expect("activity fallback");
    assert_eq!(selected, default);
    assert_eq!(schedule.calendars().len(), 1);
}

#[test]
fn overlapping_period_is_dropped_and_counted() {
    let blob = concat!(
        "(0||CalendarData()((0||DaysOfWeek()(",
        "(0||2()((0||0(s|08:00|f|12:00)())(0||1(s|11:00|f|13:00)())))",
        "))))"
    );
    let data = data(&[(1, None, "CA_Project", blob)]);
    let mut schedule = Schedule::new();
    let mut notes = Notes::new();
    let project = project(1);
    let mut calendars = XerCalendars::new(&data, days_from_civil(2024, 1, 1));
    calendars.report_warnings(&[&project], &mut notes);
    calendars
        .project_default(&project, &mut schedule, &mut notes)
        .expect("calendar should build");
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 720)]);
    assert!(
        notes
            .finish()
            .iter()
            .any(|line| line.contains("overlaps another"))
    );
}

#[test]
fn a_calendar_with_its_own_week_does_not_take_its_bases() {
    let own_week = concat!(
        "(0||CalendarData()((0||DaysOfWeek()(",
        "(0||3()((0||0(s|10:00|f|14:00)())))",
        "))))"
    );
    let data = data(&[
        (1, None, "CA_Base", MONDAY_AND_SUNDAY),
        (2, Some(1), "CA_Project", own_week),
    ]);
    let (schedule, notes) = build_default(&data, 2, days_from_civil(2024, 1, 1));
    let week = schedule.calendars()[0].week();
    assert_eq!(week[1], [(600, 840)]);
    assert!(week[0].is_empty() && week[6].is_empty());
    assert_eq!(notes.finish(), Vec::<String>::new());
}

#[test]
fn a_base_chain_that_loops_ends_with_the_fallback_week_and_a_note() {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    for start in [10, 11, 12] {
        let data = data(&[
            (10, Some(11), "CA_Base", no_week),
            (11, Some(10), "CA_Base", no_week),
            (12, Some(12), "CA_Base", no_week),
        ]);
        let (schedule, notes) = build_default(&data, start, days_from_civil(2024, 1, 1));
        assert_eq!(schedule.calendars()[0].week()[0], [(480, 1_020)]);
        let lines = notes.finish();
        assert!(
            lines
                .iter()
                .any(|line| line == &format!("Calendar {start}: its base calendars form a loop and the loop was not followed; working days can differ from the file's: 1")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("states no working week")),
            "{lines:?}"
        );
    }
}

#[test]
fn a_missing_base_ends_with_the_fallback_week_and_a_note() {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let data = data(&[(7, Some(99), "CA_Base", no_week)]);
    let (schedule, notes) = build_default(&data, 7, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 1_020)]);
    let lines = notes.finish();
    assert!(
        lines
            .iter()
            .any(|line| line == "Calendar 7: its base calendar 99 is not in the file, so the base's working time is not used; working days can differ from the file's: 1"),
        "{lines:?}"
    );
}

#[test]
fn a_base_calendars_holidays_are_not_inherited_and_say_so() {
    let base = week_blob("(0||0(d|45293)())");
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let data = data(&[
        (1, None, "CA_Base", &base),
        (2, Some(1), "CA_Project", no_week),
    ]);
    let (schedule, notes) = build_default(&data, 2, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 1_020)]);
    assert_eq!(schedule.calendars()[0].exceptions(), []);
    let lines = notes.finish();
    assert!(
        lines.iter().any(|line| line == holidays_note(2).as_str()),
        "{lines:?}"
    );
}

#[test]
fn a_base_calendars_holidays_are_noted_when_the_calendar_states_its_own_week() {
    let base = week_blob("(0||0(d|45293)())");
    let own = week_blob("");
    let data = data(&[
        (1, None, "CA_Base", &base),
        (2, Some(1), "CA_Project", &own),
    ]);
    let (schedule, notes) = build_default(&data, 2, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].exceptions(), []);
    assert_eq!(notes.finish(), [holidays_note(2).as_str()]);
}

#[test]
fn holidays_only_on_the_global_calendar_are_noted_when_the_regional_one_gives_the_week() {
    let global = week_blob("(0||0(d|45293)())");
    let regional = week_blob("");
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let data = data(&[
        (1, None, "CA_Base", &global),
        (2, Some(1), "CA_Base", &regional),
        (3, Some(2), "CA_Project", no_week),
    ]);
    let (_, notes) = build_default(&data, 3, days_from_civil(2024, 1, 1));
    let lines = notes.finish();
    assert!(
        lines.iter().any(|line| line == holidays_note(3).as_str()),
        "{lines:?}"
    );
}

#[test]
fn a_calendar_with_only_its_own_exceptions_or_a_looping_base_chain_adds_no_holiday_note() {
    let with_exception = week_blob("(0||0(d|45293)())");
    let no_exceptions = week_blob("");
    let alone = data(&[(1, None, "CA_Project", &with_exception)]);
    let (_, notes) = build_default(&alone, 1, days_from_civil(2024, 1, 1));
    assert_eq!(notes.finish(), Vec::<String>::new());

    let looped = data(&[
        (1, Some(2), "CA_Base", &no_exceptions),
        (2, Some(1), "CA_Project", &no_exceptions),
    ]);
    let (_, notes) = build_default(&looped, 2, days_from_civil(2024, 1, 1));
    assert_eq!(notes.finish(), Vec::<String>::new());

    let looped_holiday = data(&[
        (1, Some(2), "CA_Base", &with_exception),
        (2, Some(1), "CA_Project", &no_exceptions),
    ]);
    let (_, notes) = build_default(&looped_holiday, 2, days_from_civil(2024, 1, 1));
    assert_eq!(notes.finish(), [holidays_note(2).as_str()]);
}
#[test]
fn a_chain_problem_beyond_the_calendar_that_states_a_week_is_not_noted() {
    let data = data(&[(7, Some(99), "CA_Base", MONDAY_AND_SUNDAY)]);
    let (schedule, notes) = build_default(&data, 7, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 540)]);
    assert_eq!(notes.finish(), Vec::<String>::new());

    let loop_after_week = data_with_loop();
    let (_, notes) = build_default(&loop_after_week, 2, days_from_civil(2024, 1, 1));
    assert_eq!(notes.finish(), [format!("{BASE_WEEK}: 1 (first: 2)")]);
}

/// Calendar 2 has no week and is based on 1, which states one and is based on 3, which is
/// based on 1 again: the loop lies beyond the calendar that supplies the week.
fn data_with_loop() -> xer::Data {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    data(&[
        (1, Some(3), "CA_Base", MONDAY_AND_SUNDAY),
        (2, Some(1), "CA_Project", no_week),
        (3, Some(1), "CA_Base", no_week),
    ])
}

/// The XER reader already drops an overlapping period, so put one into calendar `id`'s
/// Monday afterwards, the way a week the reader let through would look.
fn overlap_monday(data: &mut xer::Data, id: u32) {
    let row = data
        .calendar_rows
        .iter_mut()
        .find(|row| row.clndr_id == id)
        .expect("calendar row");
    let week = row
        .clndr_data
        .as_mut()
        .and_then(|data| data.week.as_mut())
        .expect("calendar week");
    week[1] = vec![
        xer::Period {
            start: 480,
            finish: 720,
        },
        xer::Period {
            start: 600,
            finish: 800,
        },
    ];
}

#[test]
fn calendars_sharing_a_base_week_with_an_invalid_period_each_note_it() {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let mut data = data(&[
        (1, None, "CA_Base", MONDAY_AND_SUNDAY),
        (2, Some(1), "CA_Project", no_week),
        (3, Some(1), "CA_Project", no_week),
    ]);
    overlap_monday(&mut data, 1);
    let mut schedule = Schedule::new();
    let mut notes = Notes::new();
    let mut calendars = XerCalendars::new(&data, days_from_civil(2024, 1, 1));
    let default = calendars
        .project_default(&project(2), &mut schedule, &mut notes)
        .expect("Calendar 2");
    calendars
        .for_activity(Some(3), default, "A1", &mut schedule, &mut notes)
        .expect("Calendar 3");
    calendars
        .for_activity(Some(1), default, "A2", &mut schedule, &mut notes)
        .expect("Calendar 1");
    assert_eq!(
        notes.finish(),
        [
            invalid_period(2).as_str(),
            "A calendar states no working week of its own, so its base calendar's week is used; working days can differ from the file's: 2 (first: 2, 3)",
            invalid_period(3).as_str(),
            invalid_period(1).as_str(),
        ]
    );
    for calendar in schedule.calendars() {
        assert_eq!(calendar.week()[0], [(480, 720)]);
    }
}

#[test]
fn week_notes_come_before_exception_notes() {
    // A dropped week period, then a repeated exception date: main notes them in that order.
    let blob = week_blob("(0||0(d|45297)((0||0(s|08:00|f|12:00)())))(0||1(d|45297)())");
    let mut data = data(&[(1, None, "CA_Project", &blob)]);
    overlap_monday(&mut data, 1);
    let (_, notes) = build_default(&data, 1, days_from_civil(2024, 1, 1));
    assert_eq!(
        notes.finish(),
        [
            invalid_period(1).as_str(),
            "Calendar 1: two exceptions fall on one date and only one was read; working days can differ from the file's: 1",
        ]
    );
}

#[test]
fn a_long_base_chain_resolves_without_quadratic_blowup() {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let mut rows = vec![(1, None, "CA_Base", MONDAY_AND_SUNDAY)];
    for id in 2..=20_000 {
        rows.push((id, Some(id - 1), "CA_Project", no_week));
    }
    let data = data(&rows);
    let (schedule, notes) = build_default(&data, 20_000, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 540)]);
    // No missing-base or cycle note: the chain ends at a calendar that states a week.
    assert_eq!(notes.finish(), [format!("{BASE_WEEK}: 1 (first: 20000)")]);
}

#[test]
fn the_base_chain_holiday_walk_takes_time_linear_in_the_chain_length() {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let length = 400_000;
    let mut rows = vec![(1, None, "CA_Base", MONDAY_AND_SUNDAY)];
    rows.extend((2..=length).map(|id| (id, Some(id - 1), "CA_Project", no_week)));
    let data = data(&rows);
    let calendars = XerCalendars::new(&data, days_from_civil(2024, 1, 1));
    let started = std::time::Instant::now();
    // No calendar lists a holiday, so the walk goes to the top; a list scan of those met takes seconds.
    assert!(!calendars.base_chain_has_holidays(Some(length - 1), length));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn a_resource_calendar_on_the_base_chain_counts_for_holidays_and_week() {
    let no_week = "(0||CalendarData()((0||Exceptions()())))";
    let base = week_blob("(0||0(d|45292)())");
    let data = data(&[
        (1, None, "CA_Rsrc", &base),
        (2, Some(1), "CA_Project", no_week),
    ]);
    let (schedule, notes) = build_default(&data, 2, days_from_civil(2024, 1, 1));
    assert_eq!(schedule.calendars()[0].week()[0], [(480, 1_020)]);
    let notes = notes.finish();
    assert!(notes.contains(&holidays_note(2)));
}

#[test]
fn every_generated_project_calendar_supplies_its_own_week_without_notes() {
    let spec = xer::synth::Spec {
        projects: 3,
        ..xer::synth::Spec::small(1)
    };
    let bytes = xer::synth::write(&spec);
    let parsed = xer::parse(&bytes).expect("generated file parses");
    let data = xer::read(&parsed).expect("generated file reads");
    assert_eq!(data.project_rows.len(), 3);
    for project_row in &data.project_rows {
        let clndr_id = project_row.clndr_id.expect("project calendar");
        let (schedule, notes) = build_default(&data, clndr_id, days_from_civil(2024, 1, 1));
        // The generated week works Monday 08:00 to 12:00 and 13:00 to 17:00: no fallback week,
        // no missing-week, cycle or missing-base note.
        assert_eq!(
            schedule.calendars()[0].week()[0],
            [(480, 720), (780, 1_020)]
        );
        assert_eq!(notes.finish(), Vec::<String>::new());
    }
}

fn build_default(data: &xer::Data, calendar: u32, origin: i64) -> (Schedule, Notes) {
    let mut schedule = Schedule::new();
    let mut notes = Notes::new();
    let project = project(calendar);
    XerCalendars::new(data, origin)
        .project_default(&project, &mut schedule, &mut notes)
        .expect("calendar should build");
    (schedule, notes)
}

fn project(calendar: u32) -> ProjectRow {
    ProjectRow {
        proj_id: 7,
        proj_short_name: Some("TEST".to_owned()),
        clndr_id: Some(calendar),
        plan_start_date: None,
        plan_end_date: None,
        last_recalc_date: None,
        export_flag: Some(true),
        complete_pct_type: None,
    }
}

fn data(rows: &[(u32, Option<u32>, &str, &str)]) -> xer::Data {
    let mut text = String::from(
        "ERMHDR\t19.12\t2024-01-01 00:00\n%T\tCALENDAR\n%F\tclndr_id\tbase_clndr_id\tclndr_type\tclndr_data\n",
    );
    for (id, base, kind, blob) in rows {
        text.push_str("%R\t");
        text.push_str(&id.to_string());
        text.push('\t');
        if let Some(base) = base {
            text.push_str(&base.to_string());
        }
        text.push('\t');
        text.push_str(kind);
        text.push('\t');
        text.push_str(blob);
        text.push('\n');
    }
    text.push_str("%E\n");
    let parsed = xer::parse(text.as_bytes()).expect("test XER parses");
    let data = xer::read(&parsed).expect("test XER reads");
    for (row, (_, _, kind, _)) in data.calendar_rows.iter().zip(rows.iter()) {
        let expected = match *kind {
            "CA_Base" => CalendarType::Base,
            "CA_Project" => CalendarType::Project,
            "CA_Rsrc" => CalendarType::Resource,
            other => CalendarType::Other(other.to_owned()),
        };
        assert_eq!(row.clndr_type, Some(expected));
    }
    data
}
