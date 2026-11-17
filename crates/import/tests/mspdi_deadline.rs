//! The MS Project reader keeps each task's `Deadline` as an instant and moves no date for it.

use import::{Imported, parse_mspdi};

/// The project starts Wednesday 2026-01-07, so the origin is Monday 2026-01-05 at 00:00.
fn project(deadline: &str) -> String {
    let task = |uid: u32, extra: &str| {
        format!(
            "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>{extra}</Task>"
        )
    };
    format!(
        "<Project xmlns=\"http://schemas.microsoft.com/project\"><StartDate>2026-01-07T08:00:00</StartDate>\
<Tasks>{}{}</Tasks></Project>",
        task(1, deadline),
        task(2, "")
    )
}

fn deadlines(imported: &Imported) -> Vec<Option<i64>> {
    imported.tasks.iter().map(|task| task.deadline).collect()
}

fn dates(imported: &mut Imported) -> Vec<(i64, i64)> {
    let solution = imported.schedule.compute().expect("project computes");
    solution.dates.iter().map(|d| (d.es, d.ef)).collect()
}

#[test]
fn a_task_keeps_its_deadline_and_no_date_moves() {
    let mut with =
        parse_mspdi(&project("<Deadline>2026-01-12T16:00:00</Deadline>")).expect("imports");
    let mut without = parse_mspdi(&project("")).expect("imports");
    // Monday 2026-01-12 16:00 is 7 days and 16 hours after the origin.
    assert_eq!(deadlines(&with), [Some(7 * 1440 + 16 * 60), None]);
    assert_eq!(deadlines(&without), [None, None]);
    assert_eq!(dates(&mut with), dates(&mut without));
    assert_eq!(
        with.unsupported, without.unsupported,
        "a read deadline is no note"
    );
}

#[test]
fn a_deadline_before_the_origin_is_negative_and_an_unreadable_one_is_none() {
    let early = parse_mspdi(&project("<Deadline>2026-01-02T16:00:00</Deadline>")).expect("imports");
    // Friday 2026-01-02 16:00 is 3 days before the origin, plus 16 hours.
    assert_eq!(deadlines(&early), [Some(-3 * 1440 + 16 * 60), None]);
    let unreadable = parse_mspdi(&project("<Deadline>soon</Deadline>")).expect("imports");
    assert_eq!(deadlines(&unreadable), [None, None]);
}
