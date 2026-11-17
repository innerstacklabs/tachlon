//! gantt#2106: a value outside the engine's range does not stop an MS Project file from opening:
//! it is left out or set to the limit, with one counted note for its kind. The bounds themselves
//! are tested beside `range.rs`; here each reader is shown to call it.

use engine::{MAX_DATE, MAX_DURATION, Pin, Task};
use import::{Imported, parse_mspdi};

/// The last minute `MAX_DATE` allows from the origin Monday 2024-01-01, and a year far past it.
const AT_BOUND: &str = "3026-01-28T00:00:00";
const YEAR_9999: &str = "9999-12-31T00:00:00";
const START: &str = "<StartDate>2024-01-01T08:00:00</StartDate>";
const STARTED: &str = "<ActualStart>2024-01-01T08:00:00</ActualStart>";
const FINISH: &str =
    "<ActualStart>2024-01-01T08:00:00</ActualStart><ActualFinish>{d}</ActualFinish>";
const RESUME: &str = "<ActualStart>2024-01-01T08:00:00</ActualStart><Resume>{d}</Resume>";
const DATE: &str = "Tasks with a date more than 1,000 years from the schedule start: the date was not read, so their dates can differ from the file's";
const LAG: &str = "Links with a lag over 100 years either way: the lag was not read, so dates can differ from the file's";
const DURATION: &str = "Tasks with a duration over 100 years: the duration was set to 100 years of 366 days, so their dates can differ from the file's";
const REMAINING: &str = "Tasks with a remaining duration over 100 years: the remaining duration was not read, so their dates can differ from the file's";
const DATA_DATE: &str = "The data date is more than 1,000 years from the schedule start and was not read; progress is measured without it, so dates can differ from the file's";

fn parse(tasks: &str, project: &str) -> Imported {
    let xml = format!("<Project>{project}<Tasks>{tasks}</Tasks></Project>");
    parse_mspdi(&xml).expect("imports")
}

/// What follows `note` in the first line that starts with it: its count and first ids.
fn counted<'a>(i: &'a Imported, note: &str) -> Option<&'a str> {
    i.unsupported
        .iter()
        .find_map(|line| line.strip_prefix(note))
}

/// Where the engine reads a kind of date.
type Read = fn(&Imported, usize) -> Option<i64>;

fn task(imported: &Imported, n: usize) -> &Task {
    &imported.schedule.tasks()[n]
}

fn pin(imported: &Imported, n: usize) -> Option<i64> {
    task(imported, n).limit.pin.map(|p| match p {
        Pin::Start(at) | Pin::Finish(at) => at,
    })
}

#[test]
fn a_duration_lag_remaining_or_status_date_beyond_the_bound_gets_one_counted_note() {
    // 1,000,000 hours is 60,000,000 minutes: past `MAX_DURATION` as a duration, remaining and lag.
    let tasks = format!(
        "<Task><UID>1</UID><Duration>PT1000000H0M0S</Duration>{STARTED}\
         <RemainingDuration>PT1000000H0M0S</RemainingDuration></Task>\
         <Task><UID>2</UID><Duration>PT1H</Duration><PredecessorLink><PredecessorUID>1</PredecessorUID>\
         <LinkLag>{}</LinkLag></PredecessorLink></Task>",
        (MAX_DURATION + 1) * 10
    );
    let imported = parse(
        &tasks,
        &format!("{START}<StatusDate>{YEAR_9999}</StatusDate>"),
    );
    assert_eq!(task(&imported, 0).duration, MAX_DURATION);
    assert_eq!(imported.tasks[0].remaining, None);
    assert_eq!(imported.schedule.dependencies()[0].lag, 0);
    assert_eq!(imported.data_date, None);
    for (note, first) in [(DURATION, "1"), (REMAINING, "1"), (LAG, "2")] {
        assert_eq!(
            counted(&imported, note),
            Some(&*format!(": 1 (first: {first})"))
        );
    }
    assert!(counted(&imported, DATA_DATE).is_some());
    let kept = parse("", &format!("{START}<StatusDate>{AT_BOUND}</StatusDate>"));
    assert_eq!(kept.data_date, Some(MAX_DATE));
}

#[test]
fn a_date_outside_the_range_is_left_out_with_one_counted_note_per_kind() {
    let limit = |code: u8| {
        format!("<ConstraintType>{code}</ConstraintType><ConstraintDate>{{d}}</ConstraintDate>")
    };
    let kinds: [(String, Read); 9] = [
        (limit(4), |i, n| task(i, n).snet),
        (limit(5), |i, n| task(i, n).limit.snlt),
        (limit(6), |i, n| task(i, n).limit.fnet),
        (limit(7), |i, n| task(i, n).limit.fnlt),
        (limit(2), pin),
        (limit(3), pin),
        (FINISH.into(), |i, n| i.tasks[n].actual_finish),
        (RESUME.into(), |i, n| task(i, n).limit.resume),
        ("<ActualStart>{d}</ActualStart>".into(), |i, n| {
            i.tasks[n].actual_start
        }),
    ];
    for (elements, read) in kinds {
        let task = |uid: usize, date: &str| {
            let elements = elements.replace("{d}", date);
            format!("<Task><UID>{uid}</UID><Duration>PT1H</Duration>{elements}</Task>")
        };
        let bare = "<Task><UID>3</UID><Duration>PT1H</Duration></Task>";
        let imported = parse(&(task(1, YEAR_9999) + &task(2, AT_BOUND) + bare), START);
        let at = |n| read(&imported, n);
        assert_eq!((at(0), at(1)), (at(2), Some(MAX_DATE)), "{elements}");
        assert_eq!(
            counted(&imported, DATE),
            Some(": 1 (first: 1)"),
            "{elements}"
        );
    }
}
