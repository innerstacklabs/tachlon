//! The reader builds the schedule while it reads, so what the end of the file settles must come
//! out as it did when the whole file was held: the same first error, the same notes in the same
//! order, whatever order the file lists its parts in.

use import::{ImportError, parse_mspdi};

const HEADER: &str = "<StartDate>2026-01-05T08:00:00</StartDate>";
const CALENDARS: &str = "<Calendars><Calendar><UID>1</UID><IsBaseCalendar>1</IsBaseCalendar>\
    <WeekDays><WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime>\
    <FromTime>08:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime></WorkingTimes>\
    </WeekDay></WeekDays></Calendar></Calendars>";
const TASKS: &str = "<Tasks>\
    <Task><UID>1</UID><Name>One</Name><Duration>PT8H0M0S</Duration><Active>0</Active>\
      <Manual>1</Manual></Task>\
    <Task><UID>2</UID><Name>Two</Name><Duration>PT8H0M0S</Duration><Manual>1</Manual>\
      <PredecessorLink><PredecessorUID>1</PredecessorUID><LinkLag>5</LinkLag></PredecessorLink>\
      <PredecessorLink><PredecessorUID>9</PredecessorUID></PredecessorLink></Task>\
    <Task><UID>3</UID><Name>Three</Name><Duration>PT8H0M0S</Duration><Active>0</Active>\
      <PredecessorLink><PredecessorUID>2</PredecessorUID></PredecessorLink></Task>\
    </Tasks>";
const ASSIGNMENTS: &str = "<Assignments>\
    <Assignment><TaskUID>3</TaskUID><ResourceUID>4</ResourceUID></Assignment>\
    <Assignment><TaskUID>1</TaskUID><ResourceUID>4</ResourceUID></Assignment>\
    <Assignment><TaskUID>2</TaskUID><ResourceUID>-65535</ResourceUID></Assignment>\
    </Assignments>";

fn project(parts: &[&str]) -> String {
    format!("<Project>{}</Project>", parts.concat())
}

fn read(parts: &[&str]) -> String {
    format!("{:?}", parse_mspdi(&project(parts)))
}

#[test]
fn notes_keep_document_order_with_assignments_after_the_tasks() {
    let imported =
        parse_mspdi(&project(&[HEADER, CALENDARS, TASKS, ASSIGNMENTS])).expect("the file is valid");

    let notes: Vec<&str> = imported.unsupported.iter().map(String::as_str).collect();
    let at = |needle: &str| {
        notes
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no note with {needle:?} in {notes:?}"))
    };
    // The counted notes list first, then the sentences: one resource note for tasks 1 and 3 (not 2), then the links.
    assert!(at("Tasks that are inactive in the file") < at("Tasks scheduled manually in the file"));
    assert!(at("Tasks scheduled manually in the file") < at("2 tasks have resource assignments"));
    assert!(at("2 tasks have resource assignments") < at("Task 2: a link lag has a fraction"));
    assert!(at("Task 2: a link lag has a fraction") < at("Task 2: a link from task 9"));
}

#[test]
fn a_file_with_its_header_and_calendars_last_reads_as_one_in_order() {
    let in_order = read(&[HEADER, CALENDARS, TASKS, ASSIGNMENTS]);
    let reversed = read(&[TASKS, ASSIGNMENTS, CALENDARS, HEADER]);

    assert!(in_order.starts_with("Ok("), "{in_order}");
    assert_eq!(reversed, in_order);
}

#[test]
fn the_first_error_is_the_one_a_whole_file_reader_raises() {
    let bad_task = "<Tasks><Task><UID>1</UID><Duration>soon</Duration></Task></Tasks>";
    let bad_assignment = "<Assignments><Assignment><TaskUID>x</TaskUID></Assignment></Assignments>";
    let bad_calendar = "<Calendars><Calendar><UID>x</UID></Calendar></Calendars>";

    let element = |parts: &[&str]| match parse_mspdi(&project(parts)) {
        Err(ImportError::Invalid { element, .. }) => element,
        other => panic!("expected an invalid value, got {other:?}"),
    };
    // A later part's error ranks above an earlier task's, as the parts are read in this order:
    // header, calendars, assignments, tasks.
    assert_eq!(element(&[HEADER, bad_task, bad_assignment]), "TaskUID");
    assert_eq!(
        element(&[HEADER, bad_task, bad_assignment, bad_calendar]),
        "UID"
    );
    assert_eq!(element(&[HEADER, bad_task]), "Duration");
}

#[test]
fn a_malformed_tag_after_a_bad_value_is_still_the_error() {
    let xml = "<Project><Tasks><Task><UID>x</UID></Task></Tasks><Name></Project>";

    assert!(matches!(parse_mspdi(xml), Err(ImportError::Xml { .. })));
}

#[test]
fn a_second_tasks_element_is_ignored() {
    let one = read(&[HEADER, CALENDARS, TASKS]);
    let two = read(&[
        HEADER,
        CALENDARS,
        TASKS,
        "<Tasks><Task><UID>1</UID></Task></Tasks>",
    ]);

    assert_eq!(two, one);
}

#[test]
fn many_tasks_with_the_start_and_calendars_last_read_as_in_order() {
    let tasks: String = (1..=2000)
        .map(|uid| {
            let link = if uid > 1 {
                format!(
                    "<PredecessorLink><PredecessorUID>{}</PredecessorUID></PredecessorLink>",
                    uid - 1
                )
            } else {
                String::new()
            };
            format!(
                "<Task><UID>{uid}</UID><Name>T{uid}</Name><Duration>PT8H0M0S</Duration>\
                 <CalendarUID>1</CalendarUID><Start>2026-01-12T08:00:00</Start>{link}</Task>"
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let tasks = format!("<Tasks>{tasks}</Tasks>");

    let in_order = read(&[HEADER, CALENDARS, &tasks]);
    let last = read(&[&tasks, CALENDARS, HEADER]);

    assert!(in_order.starts_with("Ok("), "{}", &in_order[..200]);
    assert!(
        last == in_order,
        "a file with its start and calendars last differs"
    );
}
