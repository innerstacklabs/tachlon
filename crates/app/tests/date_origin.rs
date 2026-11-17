//! Every date the app writes reads the calendar date in the file (gs#150).
//!
//! The engine counts minutes from an origin Monday. The importer takes that Monday from the
//! file's plan start (`Imported::origin_day`), `Loaded::origin` carries it, and the session writes
//! and reads every date through it. A 2024 file shows 2024 dates: the data date, a task's start,
//! the lookahead's default start and what the lookahead box takes back.

use crate::common;

use app::{Loaded, Session, dates::Origin, load_bytes, meta::TaskMeta, outline::demo_layout};
use common::clock;
use engine::DAY;
use import::ProjectSelection;
use timeline::Timeline;
use xer::synth::{Spec, write};

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");
const LINKS_XML: &str = include_str!("../../import/tests/fixtures/links.xml");

fn load(bytes: &[u8]) -> Loaded {
    load_bytes(bytes, &ProjectSelection::Unspecified, &clock).expect("the file loads")
}

/// A session over `loaded` as `App::create` builds it, and the task names it was built with.
fn open_session(loaded: Loaded) -> (Session, Vec<TaskMeta>) {
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        data_date,
        origin,
        meta,
        ..
    } = loaded;
    let timeline = Timeline::new(order, 1200.0, 600.0, 1.0);
    let session = Session::new(schedule, solution, timeline, meta.clone(), clock)
        .with_outline(outline)
        .expect("a valid outline")
        .with_data_date(data_date)
        .with_origin(origin);
    (session, meta)
}

fn task_by_code(meta: &[TaskMeta], code: &str) -> u32 {
    meta.iter()
        .position(|task| task.code_text() == code)
        .unwrap_or_else(|| panic!("no task {code}")) as u32
}

/// `text` with `from` replaced by `to`, which must be there.
fn replace(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from} is not in the text");
    text.replacen(from, to, 1)
}

/// The synthetic XER with the project's `plan_start_date` emptied.
fn without_plan_start(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).expect("UTF-8");
    let mut in_project = false;
    let mut column = None;
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let mut cells: Vec<&str> = line.trim_end_matches('\n').split('\t').collect();
        match cells[0] {
            "%T" => in_project = cells.get(1) == Some(&"PROJECT"),
            "%F" if in_project => {
                column = cells.iter().position(|name| *name == "plan_start_date");
            }
            "%R" if in_project => {
                cells[column.expect("PROJECT has plan_start_date")] = "";
                out.push_str(&cells.join("\t"));
                out.push('\n');
                continue;
            }
            _ => {}
        }
        out.push_str(line);
    }
    out.into_bytes()
}

#[test]
fn a_2024_xer_file_shows_2024_dates() {
    let loaded = load(LEDGER_XER);
    assert_eq!(
        loaded.origin.day_text(0),
        "2024-01-01",
        "the plan start's Monday"
    );
    let (mut session, meta) = open_session(loaded);

    // The data date, as the toolbar writes it (last_recalc_date 2024-01-10 08:00).
    assert_eq!(session.data_date(), Some(9 * DAY + 8 * 60));
    assert_eq!(
        session.data_date_text().as_deref(),
        Some("2024-01-10 08:00")
    );

    // A task's start and finish, as the table and the hover card write them. A1-1 is complete and
    // fixed at its actuals, so A1-2, which has not started, is the one the file's data date floors:
    // it starts at 2024-01-10 08:00 (gs#736).
    let a1 = task_by_code(&meta, "A1-2");
    let dates = session.solution().dates[a1 as usize];
    assert_eq!(session.date_text(dates.es), "2024-01-10 08:00");
    let details = session.task_details(a1).expect("details");
    assert_eq!(details.start, "2024-01-10 08:00");
    assert_eq!(details.finish, session.date_text(dates.ef));
    assert!(
        details.finish.starts_with("2024-01-1"),
        "{}",
        details.finish
    );
    for task in 0..meta.len() as u32 {
        let details = session.task_details(task).expect("details");
        for text in [
            &details.start,
            &details.finish,
            &details.late_start,
            &details.late_finish,
        ] {
            assert!(text.starts_with("2024-"), "task {task}: {text}");
        }
    }

    // The same file with its data date stripped keeps the check that the file's own calendar
    // closes Monday 2024-01-01: with nothing started, the engine starts the first activity on the 2nd.
    let text = String::from_utf8(LEDGER_XER.to_vec()).expect("UTF-8");
    let stripped = replace(&text, "2024-01-10 08:00", "");
    let stripped = replace(
        &replace(&stripped, "TK_Complete", "TK_NotStart"),
        "TK_Active",
        "TK_NotStart",
    );
    let open = load(stripped.as_bytes());
    assert_eq!(open.data_date, None, "the data date is stripped");
    let (open, open_meta) = open_session(open);
    let first = task_by_code(&open_meta, "A1-1");
    assert_eq!(
        open.date_text(open.solution().dates[first as usize].es),
        "2024-01-02 09:00"
    );

    // The lookahead: the default start is the data date's day, the box takes that text back and
    // shows the same text again.
    assert_eq!(session.lookahead_start_text(), "2024-01-10");
    let start = session
        .parse_lookahead_day("2024-01-10")
        .expect("the default start parses");
    assert_eq!(start, 9 * DAY);
    assert_eq!(
        start,
        session.default_lookahead_start().div_euclid(DAY) * DAY
    );
    assert!(session.set_lookahead(start, 3));
    assert_eq!(session.lookahead_start_text(), "2024-01-10");
    // The old offset is gone: the date the app used to show for this instant is 2 years later.
    assert_ne!(session.parse_lookahead_day("2026-01-14"), Some(start));
}

#[test]
fn a_2024_mspdi_file_shows_2024_dates() {
    // links.xml starts on Monday 2026-01-05; move the project start to Wednesday 2024-01-03.
    let xml = replace(
        LINKS_XML,
        "<StartDate>2026-01-05T08:00:00",
        "<StartDate>2024-01-03T08:00:00",
    );
    let loaded = load(xml.as_bytes());
    assert_eq!(
        loaded.origin.day_text(0),
        "2024-01-01",
        "the Monday on or before the start"
    );
    assert_eq!(loaded.data_date, None);
    let (session, meta) = open_session(loaded);

    assert_eq!(
        session.data_date_text(),
        None,
        "the fixture has no StatusDate"
    );
    let first = (0..meta.len() as u32)
        .map(|task| session.solution().dates[task as usize].es)
        .min()
        .expect("tasks");
    assert_eq!(session.date_text(first), "2024-01-03 08:00");
    for task in 0..meta.len() as u32 {
        let details = session.task_details(task).expect("details");
        assert!(details.start.starts_with("2024-"), "{}", details.start);
        assert!(details.finish.starts_with("2024-"), "{}", details.finish);
    }
    // No data date: the lookahead starts on the origin Monday.
    assert_eq!(session.lookahead_start_text(), "2024-01-01");
    assert_eq!(session.parse_lookahead_day("2024-01-01"), Some(0));
    assert_eq!(session.parse_lookahead_day("2024-01-15"), Some(14 * DAY));
}

#[test]
fn the_demo_keeps_its_stated_origin() {
    let layout = demo_layout(50);
    let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
    let session = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    );
    assert_eq!(session.origin(), Origin::DEMO);
    assert_eq!(session.date_text(0), "2026-01-05 00:00");
    assert_eq!(session.lookahead_start_text(), "2026-01-05");
    assert_eq!(session.data_date_text(), None);
}

#[test]
fn opening_a_file_in_place_moves_the_origin_with_it() {
    let layout = demo_layout(50);
    let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
    let demo = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    );
    assert_eq!(demo.lookahead_start_text(), "2026-01-05");

    let next = load(LEDGER_XER);
    let opened = demo
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(next.outline),
            next.data_date,
            next.origin,
        )
        .expect("reopens");
    assert_eq!(opened.data_date_text().as_deref(), Some("2024-01-10 08:00"));
    assert_eq!(opened.lookahead_start_text(), "2024-01-10");
    assert_eq!(
        demo.lookahead_start_text(),
        "2026-01-05",
        "the old session is untouched"
    );
}

#[test]
fn a_file_with_no_plan_start_gets_its_stated_origin_and_no_panic() {
    // XER: the importer takes Monday 2000-01-03 and says so.
    let loaded = load(&without_plan_start(&write(&Spec::small(3))));
    assert!(
        loaded
            .unsupported
            .iter()
            .any(|line| line.contains("2000-01-03")),
        "{:?}",
        loaded.unsupported
    );
    assert_eq!(loaded.origin.day_text(0), "2000-01-03");
    let (session, meta) = open_session(loaded);
    assert_eq!(session.lookahead_start_text().len(), 10);
    assert!(session.date_text(0).starts_with("2000-01-03"));
    for task in 0..meta.len() as u32 {
        let details = session.task_details(task).expect("details");
        assert!(details.start.len() == 16, "{}", details.start);
    }
    let text = session.lookahead_start_text();
    let at = session
        .parse_lookahead_day(&text)
        .expect("the shown start parses");
    assert_eq!(session.origin().day_text(at), text, "the start round-trips");

    // MSPDI: no StartDate at all.
    let xml = replace(LINKS_XML, "<StartDate>2026-01-05T08:00:00</StartDate>", "");
    let loaded = load(xml.as_bytes());
    assert_eq!(loaded.origin.day_text(0), "2000-01-03");
    let (session, _) = open_session(loaded);
    assert_eq!(session.lookahead_start_text(), "2000-01-03");
    assert_eq!(session.parse_lookahead_day("2000-01-03"), Some(0));
}

/// The text of every axis label in the session's scene, with the view fitted to `days` days from
/// the origin.
fn axis_labels(session: &mut Session, days: i64) -> Vec<String> {
    session.fit(0.0, (days * DAY) as f64);
    session.scene().labels.into_iter().map(|l| l.text).collect()
}

#[test]
fn a_2024_file_axis_reads_2024() {
    let (mut session, _) = open_session(load(LEDGER_XER));
    // Weeks over days: the major tier names the month and the year.
    let weeks = axis_labels(&mut session, 40);
    assert!(
        weeks.iter().any(|t| t.ends_with(" 2024")),
        "a major label names 2024: {weeks:?}"
    );
    assert!(weeks.iter().all(|t| !t.contains("2026")), "{weeks:?}");
    // Months over weeks: "Jan 2024".
    let months = axis_labels(&mut session, 100);
    assert!(months.iter().any(|t| t == "Jan 2024"), "{months:?}");
    assert!(months.iter().all(|t| !t.contains("2026")), "{months:?}");
    // Years: the 2025 tick follows the file (the 2024 label at the origin sits under the data-date
    // tag and is left out), and nothing reads 2026 within two years of the origin.
    let years = axis_labels(&mut session, 700);
    assert!(years.iter().any(|t| t == "2025"), "{years:?}");
    assert!(years.iter().all(|t| t != "2026"), "{years:?}");
}

#[test]
fn the_demo_axis_keeps_2026_and_reopening_moves_it() {
    let layout = demo_layout(50);
    let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
    let mut demo = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    );
    let before = axis_labels(&mut demo, 100);
    assert!(before.iter().any(|t| t == "Jan 2026"), "{before:?}");

    let next = load(LEDGER_XER);
    let mut opened = demo
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(next.outline),
            next.data_date,
            next.origin,
        )
        .expect("reopens");
    let after = axis_labels(&mut opened, 100);
    assert!(after.iter().any(|t| t == "Jan 2024"), "{after:?}");
    assert!(after.iter().all(|t| !t.contains("2026")), "{after:?}");
}
