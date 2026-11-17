use engine::{Calendar, Schedule};
use import::{Imported, XerLayout, XerOptions, export_xer};
use xer::{CalendarData, CalendarException, CalendarType, Period};

/// Monday 2024-01-01 as days since 1970-01-01.
const ORIGIN_DAY: i64 = 19_723;

fn day(from: u16, to: u16) -> Vec<(u16, u16)> {
    vec![(from, to)]
}

fn period(start: u16, finish: u16) -> Period {
    Period { start, finish }
}

#[test]
fn a_working_saturday_and_a_holiday_read_back_equal() {
    // Monday to Friday 08:00-16:00 (engine week: Monday first). Tuesday 2024-01-02 is a
    // holiday and Saturday 2024-01-06 is worked 09:00-13:00.
    let week = [
        day(480, 960),
        day(480, 960),
        day(480, 960),
        day(480, 960),
        day(480, 960),
        Vec::new(),
        Vec::new(),
    ];
    let calendar = Calendar::with_exceptions(week, &[(1, Vec::new()), (5, day(540, 780))])
        .expect("calendar should build");
    let mut schedule = Schedule::new();
    schedule.add_calendar(calendar);

    let imported = Imported {
        schedule,
        tasks: Vec::new(),
        unsupported: Vec::new(),
        data_date: None,
        origin_day: ORIGIN_DAY,
        project_start: None,
    };
    let options = XerOptions {
        exported_at: 0,
        name: "Calendars".to_owned(),
        layout: XerLayout::One,
    };
    let exported = export_xer(&imported, &options).expect("calendars should export");
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    let row = data.calendar(1).expect("calendar 1 should be written");
    assert_eq!(row.clndr_name.as_deref(), Some("Calendar 1"));
    assert_eq!(row.clndr_type, Some(CalendarType::Base));
    assert_eq!(row.day_hr_cnt, Some(8.0));

    // Sunday first; 45293 is 2024-01-02 and 45297 is 2024-01-06.
    let workday = vec![period(480, 960)];
    let expected = CalendarData {
        week: Some([
            Vec::new(),
            workday.clone(),
            workday.clone(),
            workday.clone(),
            workday.clone(),
            workday,
            Vec::new(),
        ]),
        exceptions: vec![
            CalendarException {
                serial: 45_293,
                periods: Vec::new(),
                unparsed_periods: 0,
            },
            CalendarException {
                serial: 45_297,
                periods: vec![period(540, 780)],
                unparsed_periods: 0,
            },
        ],
    };
    assert_eq!(row.clndr_data.as_ref(), Some(&expected));
}
