//! A file with many manual tasks, leveling delays or odd deadlines lists one
//! counted note per kind, not one sentence per task (gantt#703).

use std::fmt::Write as _;

use import::parse_mspdi;

const TASKS: u32 = 1_000;

const CALENDARS: &str = "<Calendars><Calendar><UID>1</UID><IsBaseCalendar>1</IsBaseCalendar>\
    <WeekDays><WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime>\
    <FromTime>08:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime></WorkingTimes>\
    </WeekDay></WeekDays></Calendar></Calendars>";

/// `TASKS` independent tasks, each manual, with a leveling delay and a deadline that
/// is not a date.
fn generated_project() -> String {
    let mut xml = format!("<Project><StartDate>2026-01-05T08:00:00</StartDate>{CALENDARS}<Tasks>");
    for uid in 1..=TASKS {
        write!(
            xml,
            "<Task><UID>{uid}</UID><Duration>PT8H0M0S</Duration>\
             <Manual>1</Manual><LevelingDelay>4800</LevelingDelay><Deadline>soon</Deadline></Task>"
        )
        .expect("writing to a String cannot fail");
    }
    xml.push_str("</Tasks></Project>");
    xml
}

#[test]
fn a_thousand_odd_tasks_give_one_counted_note_per_kind() {
    let imported = parse_mspdi(&generated_project()).expect("the file should import");

    assert_eq!(imported.tasks.len(), TASKS as usize);
    assert_eq!(imported.unsupported.len(), 3, "{:?}", imported.unsupported);
    for (note, kind) in imported.unsupported.iter().zip([
        "Tasks with a deadline that is not a date",
        "Tasks scheduled manually in the file",
        "Tasks with a leveling delay",
    ]) {
        assert!(note.starts_with(kind), "{note}");
        assert!(note.ends_with(": 1000 (first: 1, 2, 3, 4, 5)"), "{note}");
    }
}
