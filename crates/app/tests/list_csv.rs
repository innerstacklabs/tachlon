//! `Session::list_csv` (gs#703): the visible list as CSV, in the columns Ctrl+C copies.

use crate::common;

use app::{Session, meta::TaskMeta};
use common::clock;
use common::collapse::{N, session, standard_calendar, summaries};
use engine::{DAY, Schedule, Task};
use timeline::Timeline;

/// A session with no outline whose tasks are called `names`, one day each.
fn named(names: &[&str]) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for _ in names {
        let task = Task {
            duration: DAY,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("calendar is the schedule's");
    }
    let solution = schedule.clone().compute().expect("computes");
    let meta = names
        .iter()
        .map(|name| TaskMeta {
            name: name.to_string(),
            ..TaskMeta::default()
        })
        .collect();
    let timeline = Timeline::new((0..names.len() as u32).collect(), 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
}

/// `list_text` as CSV, written here by the rule alone: a field with a comma, a quote or a line
/// break is quoted and its quotes doubled.
fn expected_from_text(session: &Session) -> String {
    session
        .list_text()
        .lines()
        .map(|line| {
            line.split('\t')
                .map(|cell| {
                    if cell.contains([',', '"']) {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_csv_holds_the_visible_rows_and_quotes_commas_quotes_and_line_breaks() {
    // A comma and quotes, a line break, and a plain name.
    let session = named(&["a,\"b\"", "line\nbreak", "plain"]);
    let csv = session.list_csv();
    let header = "Task,Dur,Start,Finish,Float\n";
    assert!(csv.starts_with(header), "the header line first: {csv:?}");
    let rest = &csv[header.len()..];
    assert!(
        rest.starts_with("\"a,\"\"b\"\"\","),
        "comma and quotes: {rest:?}"
    );
    assert!(rest.contains("\n\"line\nbreak\","), "line break: {rest:?}");
    assert!(
        rest.contains("\nplain,"),
        "a plain name has no quotes: {rest:?}"
    );

    // Collapse hides a summary's children; a filter hides the rest. The text is the header and
    // the visible rows, whatever the range.
    let all = session_rows(&session_of(N));
    let mut collapsed = session_of(N);
    let first = summaries(&collapsed)[0];
    collapsed = collapsed.with_collapsed(&[first]);
    let shown = session_rows(&collapsed);
    assert!(shown < all, "collapse hides rows: {shown} of {all}");
    assert_eq!(collapsed.list_csv(), expected_from_text(&collapsed));

    let mut filtered = session_of(N);
    assert!(filtered.set_critical_only(true));
    assert!(session_rows(&filtered) < all, "the filter hides rows");
    assert_eq!(filtered.list_csv(), expected_from_text(&filtered));

    let mut ranged = session_of(N);
    let order = ranged.timeline().order().to_vec();
    ranged.select(order[3]);
    assert!(ranged.extend_selection(1).is_some());
    assert_eq!(
        ranged.list_csv().lines().count(),
        all,
        "the range does not narrow it"
    );
}

#[test]
fn a_name_that_starts_with_a_formula_character_is_written_as_text() {
    // A spreadsheet evaluates a cell that starts with = + - or @; the apostrophe makes it text.
    let names = ["=SUM(A1)", "+1", "-1", "@x", "a-b", "-2,\"q\""];
    let csv = named(&names).list_csv();
    let rows: Vec<&str> = csv.lines().skip(1).collect();
    let starts = |row: usize, text: &str| {
        assert!(rows[row].starts_with(text), "row {row}: {:?}", rows[row]);
    };
    starts(0, "'=SUM(A1),");
    starts(1, "'+1,");
    starts(2, "'-1,");
    starts(3, "'@x,");
    starts(4, "a-b,"); // a dash inside a name is not a lead character
    starts(5, "\"'-2,\"\"q\"\"\","); // the apostrophe goes inside the quotes
}

fn session_of(n: usize) -> Session {
    session(n)
}

/// The lines of `list_csv`, the header included.
fn session_rows(session: &Session) -> usize {
    session.list_csv().lines().count()
}
