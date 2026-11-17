//! Overlapping working periods in an MSPDI calendar are dropped with a note, as in XER; the
//! import itself does not fail (gs#227).

use import::parse_mspdi;

const OVERLAPPING: &str = include_str!("fixtures/overlapping-periods.xml");

fn imported() -> import::Imported {
    parse_mspdi(OVERLAPPING).expect("overlapping periods should not fail the import")
}

#[test]
fn overlapping_weekly_periods_are_dropped_and_the_rest_stay() {
    let imported = imported();
    let week = imported.schedule.calendars()[0].week();

    // Monday: 11:00 to 13:00 dropped, 08:00 to 12:00 and 13:00 to 17:00 stay.
    assert_eq!(week[0], vec![(480, 720), (780, 1_020)]);
    // Tuesday: nothing overlaps, both stay.
    assert_eq!(week[1], vec![(480, 720), (780, 1_020)]);
    // Wednesday: untouched.
    assert_eq!(week[2], vec![(480, 960)]);
    // Thursday: the period that ends before it starts is dropped, the other stays.
    assert_eq!(week[3], vec![(540, 720)]);
}

#[test]
fn an_overlapping_exception_keeps_its_other_periods() {
    let imported = imported();
    let exceptions = imported.schedule.calendars()[0].exceptions();

    // 2026-01-12 is day 7 from the 2026-01-05 origin.
    assert_eq!(exceptions, [(7, vec![(540, 720), (840, 960)])]);
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
            "Calendar 1: a working period on Thursday was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's.",
            "Calendar 1: a working period on 2026-01-12 was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's.",
            "Calendar 2: a working period on Monday was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's.",
        ]
    );
}

#[test]
fn a_calendar_with_touching_periods_gets_no_note() {
    let xml = "<Project><Calendars><Calendar><UID>1</UID><WeekDays>\
<WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes>\
<WorkingTime><FromTime>08:00:00</FromTime><ToTime>12:00:00</ToTime></WorkingTime>\
<WorkingTime><FromTime>12:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime>\
</WorkingTimes></WeekDay></WeekDays></Calendar></Calendars></Project>";
    let imported = parse_mspdi(xml).expect("touching periods should import");

    assert_eq!(
        imported.schedule.calendars()[0].week()[0],
        vec![(480, 720), (720, 1_020)]
    );
    assert!(
        !imported
            .unsupported
            .iter()
            .any(|note| note.contains("was not read")),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn an_exception_with_only_refused_periods_is_ignored_not_made_a_holiday() {
    let xml = OVERLAPPING.replace(
        "<WorkingTimes>\n            <WorkingTime><FromTime>09:00:00</FromTime><ToTime>12:00:00</ToTime></WorkingTime>\n            <WorkingTime><FromTime>10:00:00</FromTime><ToTime>13:00:00</ToTime></WorkingTime>\n            <WorkingTime><FromTime>14:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime>\n          </WorkingTimes>",
        "<WorkingTimes><WorkingTime><FromTime>16:00:00</FromTime><ToTime>09:00:00</ToTime></WorkingTime></WorkingTimes>",
    );
    assert_ne!(
        xml, OVERLAPPING,
        "the exception times should have been replaced"
    );
    let imported = parse_mspdi(&xml).expect("refused exception periods should not fail the import");

    assert_eq!(imported.schedule.calendars()[0].exceptions(), []);
    assert!(
        imported.unsupported.contains(
            &"Calendar 1: a working exception whose working times are all invalid was not read; those days keep the calendar's usual week.".to_owned()
        ),
        "{:?}",
        imported.unsupported
    );
}
