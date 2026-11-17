//! A local answer-key check of the lag calendar (gs#148). It needs a P6 XER file that carries
//! P6's own computed dates and is not in the repository:
//!
//! `XER_ANSWER_KEY=<file.xer> cargo test -p import --test answer_key -- --ignored --nocapture`
//!
//! For each link with a non-zero lag between tasks on different calendars it takes P6's dates for
//! the predecessor (`early_start_date`, `early_end_date`), predicts the successor's start under
//! each lag calendar with the engine's calendar arithmetic, and compares it with P6's start for
//! the successor. Only the counts are printed. FS and SS links are compared on the successor's
//! start. FF and SF links are checked one way: P6's successor finish may not be earlier than the
//! instant the lag reaches. The file and anything derived from it must not be committed.

use std::fs;

use engine::{Calendar, LinkType};
use import::{ProjectSelection, parse_xer};

#[derive(Default)]
struct Tally {
    exact: usize,
    /// P6's successor start is earlier than the link allows under this choice.
    violated: usize,
    /// P6's successor start is later (the link did not drive it, or something else did).
    later: usize,
}

#[test]
#[ignore = "needs XER_ANSWER_KEY=<a P6 XER file with computed dates>"]
#[expect(
    clippy::too_many_lines,
    reason = "one pass tallies every cross-calendar link, then the report prints the tallies"
)]
fn lag_calendar_against_p6_computed_dates() {
    let Some(path) = std::env::var_os("XER_ANSWER_KEY") else {
        eprintln!("XER_ANSWER_KEY unset; skipped");
        return;
    };
    let bytes = fs::read(&path).expect("the answer-key file should be readable");
    let imported = parse_xer(&bytes, &ProjectSelection::All).expect("the file should import");
    let calendars = imported.schedule.calendars();
    let tasks = imported.schedule.tasks();

    let mut cross = 0;
    let mut scored = 0;
    let mut other_kinds = 0;
    let mut no_dates = 0;
    let mut differ = 0;
    let mut predecessor = Tally::default();
    let mut successor = Tally::default();
    let mut finish_violated_pred = 0;
    let mut finish_violated_succ = 0;
    let mut discriminating_pred = 0;
    let mut discriminating_succ = 0;
    for dependency in imported.schedule.dependencies() {
        let (p, s) = (dependency.pred.0 as usize, dependency.succ.0 as usize);
        if dependency.lag == 0 || tasks[p].calendar == tasks[s].calendar {
            continue;
        }
        cross += 1;
        let start_driven = matches!(dependency.kind, LinkType::FS | LinkType::SS);
        if !start_driven {
            let anchor = match dependency.kind {
                LinkType::FF => imported.tasks[p].file_finish,
                _ => imported.tasks[p].file_start,
            };
            let Some((anchor, finish)) = anchor.zip(imported.tasks[s].file_finish) else {
                no_dates += 1;
                continue;
            };
            other_kinds += 1;
            for (count, owner) in [
                (&mut finish_violated_pred, p),
                (&mut finish_violated_succ, s),
            ] {
                if finish < calendars[tasks[owner].calendar.0 as usize].add(anchor, dependency.lag)
                {
                    *count += 1;
                }
            }
            continue;
        }
        let (anchor, actual) = match (
            dependency.kind,
            imported.tasks[p].file_start,
            imported.tasks[p].file_finish,
            imported.tasks[s].file_start,
        ) {
            (LinkType::FS, _, Some(finish), Some(actual)) => (finish, actual),
            (LinkType::SS, Some(start), _, Some(actual)) => (start, actual),
            _ => {
                no_dates += 1;
                continue;
            }
        };
        if anchor < 0 || actual < 0 {
            no_dates += 1;
            continue;
        }
        scored += 1;
        let predict = |lag_calendar: &Calendar| {
            let reached = lag_calendar.add(anchor, dependency.lag);
            calendars[tasks[s].calendar.0 as usize].next_working(reached)
        };
        let by_predecessor = predict(&calendars[tasks[p].calendar.0 as usize]);
        let by_successor = predict(&calendars[tasks[s].calendar.0 as usize]);
        if by_predecessor != by_successor {
            differ += 1;
        }
        for (tally, predicted) in [
            (&mut predecessor, by_predecessor),
            (&mut successor, by_successor),
        ] {
            match actual.cmp(&predicted) {
                std::cmp::Ordering::Equal => tally.exact += 1,
                std::cmp::Ordering::Less => tally.violated += 1,
                std::cmp::Ordering::Greater => tally.later += 1,
            }
        }
        if by_predecessor != by_successor && actual == by_predecessor {
            discriminating_pred += 1;
        }
        if by_predecessor != by_successor && actual == by_successor {
            discriminating_succ += 1;
        }
    }
    eprintln!(
        "file lag calendar: {:?}; cross-calendar lag links: {cross} (FS/SS scored {scored}, FF/SF checked one way {other_kinds}, no P6 dates {no_dates})",
        imported.schedule.lag_calendar()
    );
    eprintln!("links where the two choices predict different starts: {differ}");
    for (name, tally, discriminating) in [
        ("Predecessor", &predecessor, discriminating_pred),
        ("Successor", &successor, discriminating_succ),
    ] {
        eprintln!(
            "{name}: P6 start equals prediction {} / P6 earlier than the link allows {} / P6 later {}; equal where the choices differ {}",
            tally.exact, tally.violated, tally.later, discriminating
        );
    }
    eprintln!(
        "FF/SF links where P6's finish is earlier than the link allows: Predecessor {finish_violated_pred} / Successor {finish_violated_succ}"
    );
}
