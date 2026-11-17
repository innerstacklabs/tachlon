//! Import time on a large MSPDI file. Run it by hand, in release mode:
//! `cargo test -p import --release --test mspdi_scale -- --ignored --nocapture`.

use std::fmt::Write as _;
use std::time::Instant;

use import::parse_mspdi;

const TASKS: u32 = 50_000;

/// A project of `TASKS` independent tasks that each carry a deadline that is not a date, so all of them
/// count in one `unsupported` line.
fn generated_project() -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <Project xmlns=\"http://schemas.microsoft.com/project\">\
         <StartDate>2026-01-05T08:00:00</StartDate><Tasks>",
    );
    for uid in 1..=TASKS {
        write!(
            xml,
            "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel>\
             <Summary>0</Summary><Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>\
             <Deadline>soon</Deadline></Task>"
        )
        .expect("writing to a String cannot fail");
    }
    xml.push_str("</Tasks></Project>");
    xml
}

#[test]
#[ignore = "timing run for 50,000 tasks; run by hand"]
fn imports_fifty_thousand_tasks_with_one_counted_note() {
    let xml = generated_project();
    let started = Instant::now();
    let imported = parse_mspdi(&xml).expect("generated project should import");
    let elapsed = started.elapsed();
    eprintln!(
        "parse_mspdi: {TASKS} tasks, {} bytes, {} notes, {elapsed:?}",
        xml.len(),
        imported.unsupported.len()
    );

    // The counted deadline note, then the one line for the missing calendars.
    assert_eq!(imported.tasks.len(), TASKS as usize);
    assert_eq!(
        imported.unsupported,
        [
            format!("Tasks with a deadline that is not a date: the deadline was not read; no dates are affected: {TASKS} (first: 1, 2, 3, 4, 5)"),
            "Project: the file has no calendars, so Monday to Friday 08:00 to 17:00 is used and dates can differ from the file's.".to_owned(),
        ]
    );
}
