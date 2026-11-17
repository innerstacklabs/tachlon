//! A working period whose `FromTime` or `ToTime` is missing or unreadable is dropped with a
//! note; the import itself does not fail (gs#240).

use import::parse_mspdi;

const UNREADABLE: &str = include_str!("fixtures/unreadable-times.xml");

fn imported() -> import::Imported {
    parse_mspdi(UNREADABLE).expect("an unreadable working time should not fail the import")
}

#[test]
fn an_unreadable_weekly_period_is_dropped_and_its_siblings_stay() {
    let imported = imported();
    let week = imported.schedule.calendars()[0].week();

    // Monday: ToTime 25:00:00 dropped, 08:00 to 12:00 stays.
    assert_eq!(week[0], vec![(480, 720)]);
    // Tuesday: the missing ToTime and the FromTime "noon" dropped, 09:00 to 17:00 stays.
    assert_eq!(week[1], vec![(540, 1_020)]);
    // Wednesday: untouched.
    assert_eq!(week[2], vec![(480, 960)]);
    // Thursday: the only period is unreadable.
    assert_eq!(week[3], Vec::<(u16, u16)>::new());
}

#[test]
fn an_unreadable_exception_period_keeps_its_valid_sibling() {
    let imported = imported();
    let exceptions = imported.schedule.calendars()[0].exceptions();

    // 2026-01-12 is day 7 from the 2026-01-05 origin. The 2026-01-19 to 2026-01-20 range has
    // only an unreadable period, so it is ignored rather than made a holiday.
    assert_eq!(exceptions, [(7, vec![(540, 720)])]);
}

#[test]
fn a_second_valid_calendar_is_kept() {
    let imported = imported();
    let calendars = imported.schedule.calendars();

    assert_eq!(calendars.len(), 2);
    assert_eq!(calendars[1].week()[0], vec![(420, 900)]);
}

#[test]
fn each_drop_is_one_note_naming_the_calendar_and_day() {
    let imported = imported();
    let notes: Vec<&str> = imported
        .unsupported
        .iter()
        .map(String::as_str)
        .filter(|note| note.contains("a working period on"))
        .collect();

    assert_eq!(
        notes,
        [
            "Calendar 1: a working period on Monday was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's.",
            "Calendar 1: a working period on Tuesday was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's.",
            "Calendar 1: a working period on Thursday was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's.",
            "Calendar 1: a working period on 2026-01-12 was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's.",
            "Calendar 1: a working period on 2026-01-19 to 2026-01-20 was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's.",
        ]
    );
    assert!(
        imported.unsupported.contains(
            &"Calendar 1: a working exception whose working times are all invalid was not read; those days keep the calendar's usual week.".to_owned()
        ),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn a_working_time_without_either_time_does_not_panic() {
    let xml = "<Project><Calendars><Calendar><UID>1</UID><WeekDays>\
<WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes>\
<WorkingTime/><WorkingTime><FromTime></FromTime><ToTime></ToTime></WorkingTime>\
<WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime>\
</WorkingTimes></WeekDay></WeekDays></Calendar></Calendars></Project>";
    let imported = parse_mspdi(xml).expect("empty working times should not fail the import");

    assert_eq!(imported.schedule.calendars()[0].week()[0], vec![(480, 960)]);
}
