use super::*;
use crate::dom;
use crate::units::days_from_civil;

/// Reads the calendars of `xml` as the reader does: each `Calendar` is added as it closes.
fn build_from(
    xml: &str,
    schedule: &mut Schedule,
    notes: &mut Notes,
) -> Result<Calendars, ImportError> {
    let origin_day = days_from_civil(2026, 1, 5);
    let mut set = None;
    let mut failure = None;
    dom::parse(xml, |root, item| {
        if let dom::Item::Calendar(calendar) = item {
            let result = match &mut set {
                Some(set) => Ok(set),
                None => CalendarSet::new(root).map(|new| set.insert(new)),
            };
            if let Err(error) = result.and_then(|set| set.add(&calendar, origin_day, notes)) {
                failure.get_or_insert(error);
            }
        }
    })
    .expect("test XML should parse");
    if let Some(error) = failure {
        return Err(error);
    }
    let root = Node {
        name: "Project",
        text: "".into(),
        children: Vec::new(),
    };
    let set = match set {
        Some(set) => set,
        None => CalendarSet::new(&root)?,
    };
    set.build(schedule, notes)
}

fn import_calendars(xml: &str) -> (Schedule, Calendars, Vec<String>) {
    let mut schedule = Schedule::new();
    let mut notes = Notes::new();
    let calendars =
        build_from(xml, &mut schedule, &mut notes).expect("test calendars should import");
    (schedule, calendars, notes.finish())
}

fn working_time(from: &str, to: &str) -> String {
    format!(
        "<WorkingTimes><WorkingTime><FromTime>{from}</FromTime><ToTime>{to}</ToTime></WorkingTime></WorkingTimes>"
    )
}

#[test]
fn maps_monday_and_sunday_weekdays() {
    let periods = working_time("08:00:00", "17:00:00");
    let xml = format!(
        "<Project><Calendars><Calendar><UID>1</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         <WeekDay><DayType>1</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         </WeekDays></Calendar></Calendars></Project>"
    );
    let (schedule, _, _) = import_calendars(&xml);
    let week = schedule.calendars()[0].week();

    assert_eq!(week[0], vec![(480, 1020)]);
    assert_eq!(week[6], vec![(480, 1020)]);
    assert!(week[1..6].iter().all(Vec::is_empty));
}

#[test]
fn imports_old_and_new_holidays() {
    let periods = working_time("08:00:00", "17:00:00");
    let xml = format!(
        "<Project><Calendars><Calendar><UID>1</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         <WeekDay><DayType>0</DayType><DayWorking>0</DayWorking><TimePeriod>\
         <FromDate>2026-01-05T00:00:00</FromDate><ToDate>2026-01-05T23:59:00</ToDate>\
         </TimePeriod></WeekDay></WeekDays><Exceptions><Exception><DayWorking>0</DayWorking>\
         <TimePeriod><FromDate>2026-01-06</FromDate><ToDate>2026-01-06</ToDate></TimePeriod>\
         </Exception></Exceptions></Calendar></Calendars></Project>"
    );
    let (schedule, _, _) = import_calendars(&xml);

    assert_eq!(schedule.calendars()[0].holidays(), &[0, 1]);
}

#[test]
fn inherits_weekday_and_holiday_from_base() {
    let monday = working_time("08:00:00", "17:00:00");
    let tuesday = working_time("09:00:00", "12:00:00");
    let xml = format!(
        "<Project><CalendarUID>2</CalendarUID><Calendars>\
         <Calendar><UID>1</UID><IsBaseCalendar>1</IsBaseCalendar><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{monday}</WeekDay>\
         <WeekDay><DayType>0</DayType><DayWorking>0</DayWorking><TimePeriod>\
         <FromDate>2026-01-07</FromDate><ToDate>2026-01-07</ToDate>\
         </TimePeriod></WeekDay></WeekDays></Calendar>\
         <Calendar><UID>2</UID><BaseCalendarUID>1</BaseCalendarUID><WeekDays>\
         <WeekDay><DayType>3</DayType><DayWorking>1</DayWorking>{tuesday}</WeekDay>\
         </WeekDays></Calendar></Calendars></Project>"
    );
    let (schedule, calendars, _) = import_calendars(&xml);
    let child = &schedule.calendars()[1];

    assert_eq!(child.week()[0], vec![(480, 1020)]);
    assert_eq!(child.week()[1], vec![(540, 720)]);
    assert_eq!(child.holidays(), &[2]);
    assert_eq!(calendars.default, CalendarId(1));
}

#[test]
fn reports_missing_base_calendar() {
    let periods = working_time("08:00:00", "17:00:00");
    let xml = format!(
        "<Project><Calendars><Calendar><UID>4</UID><BaseCalendarUID>99</BaseCalendarUID>\
         <WeekDays><WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         </WeekDays></Calendar></Calendars></Project>"
    );
    let (_, _, unsupported) = import_calendars(&xml);

    assert_eq!(
        unsupported,
        ["Calendar 4: its base calendar 99 is not in the file, so the base's working time is not used; its working days can differ from the file's.".to_owned()]
    );
}

#[test]
fn applies_working_exceptions_in_both_forms() {
    let periods = working_time("08:00:00", "17:00:00");
    let short = working_time("09:00:00", "12:00:00");
    let xml = format!(
        "<Project><Calendars><Calendar><UID>1</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         <WeekDay><DayType>0</DayType><DayWorking>1</DayWorking>{short}<TimePeriod>\
         <FromDate>2026-01-07</FromDate><ToDate>2026-01-07</ToDate></TimePeriod></WeekDay>\
         </WeekDays><Exceptions><Exception><DayWorking>1</DayWorking><TimePeriod>\
         <FromDate>2026-01-10</FromDate><ToDate>2026-01-10</ToDate></TimePeriod>\
         {periods}</Exception></Exceptions></Calendar></Calendars></Project>"
    );
    let (schedule, _, unsupported) = import_calendars(&xml);
    let calendar = &schedule.calendars()[0];

    assert_eq!(
        calendar.exceptions(),
        [(2, vec![(540, 720)]), (5, vec![(480, 1020)])]
    );
    assert_eq!(calendar.holidays(), []);
    assert!(unsupported.is_empty(), "{unsupported:?}");
}

#[test]
fn a_derived_calendar_exception_replaces_its_base_on_the_same_date() {
    let periods = working_time("08:00:00", "17:00:00");
    let exception = |uid: u32, working: u8, times: &str| {
        format!(
            "<Calendar><UID>{uid}</UID>{base}<WeekDays><WeekDay><DayType>2</DayType>\
             <DayWorking>1</DayWorking>{periods}</WeekDay></WeekDays><Exceptions><Exception>\
             <DayWorking>{working}</DayWorking><TimePeriod><FromDate>2026-01-06</FromDate>\
             <ToDate>2026-01-06</ToDate></TimePeriod>{times}</Exception></Exceptions></Calendar>",
            base = if uid == 2 {
                "<BaseCalendarUID>1</BaseCalendarUID>"
            } else {
                ""
            },
        )
    };
    let xml = format!(
        "<Project><Calendars>{}{}</Calendars></Project>",
        exception(1, 0, ""),
        exception(2, 1, &working_time("09:00:00", "10:00:00")),
    );
    let (schedule, _, unsupported) = import_calendars(&xml);

    assert_eq!(schedule.calendars()[0].holidays(), &[1]);
    assert_eq!(
        schedule.calendars()[1].exceptions(),
        [(1, vec![(540, 600)])]
    );
    assert!(unsupported.is_empty(), "{unsupported:?}");
}

#[test]
fn overlapping_exception_periods_are_dropped_with_a_note() {
    let periods = working_time("08:00:00", "17:00:00");
    let overlap = "<WorkingTimes>\
         <WorkingTime><FromTime>08:00:00</FromTime><ToTime>12:00:00</ToTime></WorkingTime>\
         <WorkingTime><FromTime>11:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime>\
         </WorkingTimes>";
    let xml = format!(
        "<Project><Calendars><Calendar><UID>5</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         </WeekDays><Exceptions><Exception><DayWorking>1</DayWorking><TimePeriod>\
         <FromDate>2026-01-10</FromDate><ToDate>2026-01-10</ToDate></TimePeriod>\
         {overlap}</Exception></Exceptions></Calendar></Calendars></Project>"
    );
    let (schedule, _, unsupported) = import_calendars(&xml);

    assert_eq!(
        schedule.calendars()[0].exceptions(),
        [(5, vec![(480, 720)])]
    );
    assert_eq!(
        unsupported,
        [
            "Calendar 5: a working period on 2026-01-10 was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's."
        ]
    );
}

#[test]
fn skips_holiday_before_origin() {
    let periods = working_time("08:00:00", "17:00:00");
    let xml = format!(
        "<Project><Calendars><Calendar><UID>1</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         <WeekDay><DayType>0</DayType><DayWorking>0</DayWorking><TimePeriod>\
         <FromDate>2026-01-01</FromDate><ToDate>2026-01-01</ToDate>\
         </TimePeriod></WeekDay></WeekDays></Calendar></Calendars></Project>"
    );
    let (schedule, _, _) = import_calendars(&xml);

    assert_eq!(schedule.calendars()[0].holidays(), []);
}

#[test]
fn creates_default_when_project_has_no_calendars() {
    let (schedule, calendars, unsupported) = import_calendars("<Project />");
    let week = schedule.calendars()[0].week();

    assert_eq!(calendars.default, CalendarId(0));
    assert!(calendars.by_uid.is_empty());
    assert!(week[..5].iter().all(|day| day == &vec![(480, 1020)]));
    assert!(week[5..].iter().all(Vec::is_empty));
    assert_eq!(
        unsupported,
        ["Project: the file has no calendars, so Monday to Friday 08:00 to 17:00 is used and dates can differ from the file's.".to_owned()]
    );
}

#[test]
fn selects_the_required_project_fallbacks() {
    let periods = working_time("08:00:00", "17:00:00");
    let calendars = format!(
        "<Calendars><Calendar><UID>10</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         </WeekDays></Calendar><Calendar><UID>20</UID><IsBaseCalendar>1</IsBaseCalendar>\
         <WeekDays><WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         </WeekDays></Calendar></Calendars>"
    );
    let (_, absent, unsupported) = import_calendars(&format!("<Project>{calendars}</Project>"));

    assert_eq!(absent.default, CalendarId(1));
    assert_eq!(unsupported, Vec::<String>::new());

    let (_, missing, unsupported) = import_calendars(&format!(
        "<Project><CalendarUID>99</CalendarUID>{calendars}</Project>"
    ));

    assert_eq!(missing.default, CalendarId(0));
    assert_eq!(
        unsupported,
        ["Project: calendar 99 is not in the file, so calendar 10 is used for the project and dates can differ from the file's.".to_owned()]
    );
}

#[test]
fn midnight_to_midnight_is_a_24_hour_day() {
    let periods = working_time("00:00:00", "00:00:00");
    let xml = format!(
        "<Project><Calendars><Calendar><UID>1</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         </WeekDays></Calendar></Calendars></Project>"
    );
    let (schedule, _, _) = import_calendars(&xml);

    assert_eq!(schedule.calendars()[0].week()[0], vec![(0, 1440)]);
}

#[test]
fn a_negative_base_calendar_uid_means_no_base() {
    let xml = "<Project><Calendars><Calendar><UID>1</UID><BaseCalendarUID>-65533</BaseCalendarUID>\
         <WeekDays><WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>\
         <WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes>\
         </WeekDay></WeekDays></Calendar></Calendars></Project>";
    let (schedule, _, unsupported) = import_calendars(xml);

    assert_eq!(schedule.calendars()[0].week()[0], vec![(480, 960)]);
    assert!(unsupported.is_empty(), "{unsupported:?}");
}

#[test]
fn treats_midnight_end_as_end_of_day() {
    let periods = working_time("08:00:00", "00:00:00");
    let xml = format!(
        "<Project><Calendars><Calendar><UID>1</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         </WeekDays></Calendar></Calendars></Project>"
    );
    let (schedule, _, _) = import_calendars(&xml);

    assert_eq!(schedule.calendars()[0].week()[0], vec![(480, 1440)]);
}

#[test]
fn rejects_duplicate_uid() {
    let mut schedule = Schedule::new();
    let mut notes = Notes::new();
    let result = build_from(
        "<Project><Calendars><Calendar><UID>1</UID></Calendar>\
         <Calendar><UID>1</UID></Calendar></Calendars></Project>",
        &mut schedule,
        &mut notes,
    );

    assert_eq!(
        result.err(),
        Some(ImportError::Invalid {
            element: "UID".to_owned(),
            value: "1".to_owned(),
        })
    );
}

#[test]
fn skips_a_calendar_with_no_working_time() {
    let mut schedule = Schedule::new();
    let mut notes = Notes::new();

    let calendars = build_from(
        "<Project><Calendars><Calendar><UID>7</UID></Calendar></Calendars></Project>",
        &mut schedule,
        &mut notes,
    )
    .expect("an empty calendar is not an error");
    let unsupported = notes.finish();

    assert!(!calendars.by_uid.contains_key(&7));
    assert_eq!(schedule.calendars().len(), 1);
    assert!(
        unsupported
            .iter()
            .any(|line| line == "Calendar 7: it has no working time and was not imported; tasks that use it get the project calendar, so their dates can differ from the file's."),
        "{unsupported:?}"
    );
}

#[test]
fn overlapping_weekly_periods_are_dropped_with_a_note() {
    let periods = "<WorkingTimes>\
         <WorkingTime><FromTime>08:00:00</FromTime><ToTime>12:00:00</ToTime></WorkingTime>\
         <WorkingTime><FromTime>11:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime>\
         </WorkingTimes>";
    let xml = format!(
        "<Project><Calendars><Calendar><UID>7</UID><WeekDays>\
         <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{periods}</WeekDay>\
         </WeekDays></Calendar></Calendars></Project>"
    );
    let (schedule, _, unsupported) = import_calendars(&xml);

    assert_eq!(schedule.calendars()[0].week()[0], vec![(480, 720)]);
    assert_eq!(
        unsupported,
        [
            "Calendar 7: a working period on Monday was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's."
        ]
    );
}
