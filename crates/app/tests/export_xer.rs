//! Writing the session as a Primavera XER file (gs#703): the file carries the edits.

use app::baseline::Baseline;
use app::document::{Extras, Source, SourceFormat};
use app::export::{export_xer, xer_left_out};
use app::{Loaded, Session, edit::Edit, fitted_timeline, load_bytes};
use engine::TaskId;
use import::{ProjectSelection, parse_xer};

const XER: &[u8] = include_bytes!("../../../packages/gantt/tests/fixtures/one-project.xer");
const MSPDI: &str = include_str!("../../import/tests/fixtures/summary.xml");

fn open(bytes: &[u8]) -> Session {
    let Loaded {
        schedule,
        solution,
        order,
        meta,
        outline,
        origin,
        data_date,
        progress,
        ..
    } = load_bytes(bytes, &ProjectSelection::All, &|| 0.0).expect("loads");
    let timeline = fitted_timeline(order, &solution, 1000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, || 0.0)
        .with_outline(outline)
        .expect("the outline matches")
        .with_origin(origin)
        .with_data_date(data_date)
        .with_progress(progress)
}

fn task(session: &Session, name: &str) -> TaskId {
    let at = session.meta().iter().position(|m| m.name == name);
    TaskId(at.expect(name) as u32)
}

fn from_xer(bytes: &[u8]) -> Extras<'_> {
    Extras {
        name: "one-project",
        notes: &[],
        source: Some(Source {
            format: SourceFormat::Xer,
            bytes,
        }),
    }
}

/// The name and duration of every task the file holds, leaves and summaries, sorted by name.
fn read_back(bytes: &[u8]) -> Vec<(String, i64)> {
    let read = parse_xer(bytes, &ProjectSelection::All).expect("reads back");
    let mut tasks: Vec<_> = read
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name.clone(), read.schedule.tasks()[i].duration))
        .collect();
    tasks.sort();
    tasks
}

#[test]
fn an_edited_session_exports_its_edits_to_xer() {
    let mut session = open(XER);
    let (second, third) = (task(&session, "Activity 2"), task(&session, "Activity 3"));
    session
        .edit(Edit::Rename {
            task: second,
            name: "Renamed activity".into(),
        })
        .expect("renames");
    session
        .edit(Edit::SetDuration {
            task: third,
            minutes: 7 * 480,
        })
        .expect("changes the duration");

    let exported = export_xer(&session, &from_xer(XER)).expect("exports");
    let tasks = read_back(&exported.bytes);
    assert!(tasks.iter().any(|t| t.0 == "Renamed activity"), "{tasks:?}");
    assert!(tasks.iter().all(|t| t.0 != "Activity 2"));
    assert!(tasks.contains(&("Activity 3".into(), 7 * 480)), "{tasks:?}");
}

#[test]
fn a_deleted_task_is_not_written() {
    let mut session = open(XER);
    let before = read_back(&export_xer(&session, &from_xer(XER)).unwrap().bytes);
    assert!(before.iter().any(|t| t.0 == "Activity 4"));

    let fourth = task(&session, "Activity 4");
    session
        .edit(Edit::Delete { task: fourth })
        .expect("deletes");
    let exported = export_xer(&session, &from_xer(XER)).expect("exports");
    let after = read_back(&exported.bytes);
    assert!(after.iter().all(|t| t.0 != "Activity 4"), "{after:?}");
    assert_eq!(after.len(), before.len() - 1);
}

#[test]
fn the_same_session_exports_the_same_bytes() {
    let session = open(XER);
    let first = export_xer(&session, &from_xer(XER)).expect("exports");
    let second = export_xer(&session, &from_xer(XER)).expect("exports");
    assert_eq!(first.bytes, second.bytes);
    assert!(first.bytes.starts_with(b"ERMHDR"));
}

#[test]
fn an_xer_source_gives_its_projects_and_any_other_gives_one() {
    let projects = |bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let table = text.split("%T\tPROJECT\n").nth(1).expect("a PROJECT table");
        table
            .lines()
            .skip(1)
            .take_while(|l| l.starts_with("%R"))
            .count()
    };
    let two = include_bytes!("../../../packages/gantt/tests/fixtures/two-projects.xer");
    let session = open(two);
    let kept = export_xer(&session, &from_xer(two)).expect("exports");
    assert_eq!(projects(&kept.bytes), 2);

    let session = open(MSPDI.as_bytes());
    let extras = Extras {
        name: "summary",
        notes: &[],
        source: None,
    };
    let one = export_xer(&session, &extras).expect("exports");
    assert_eq!(projects(&one.bytes), 1);
    assert!(String::from_utf8_lossy(&one.bytes).contains("summary"));
}

/// A task's actual start, actual finish and remaining work, the dates as minutes from 1970.
fn actuals(imported: &import::Imported, name: &str) -> (Option<i64>, Option<i64>, Option<i64>) {
    let task = imported.tasks.iter().find(|t| t.name == name).expect(name);
    let day = imported.origin_day * 1_440;
    (
        task.actual_start.map(|at| at + day),
        task.actual_finish.map(|at| at + day),
        task.remaining,
    )
}

#[test]
fn a_started_task_reads_back_started_with_its_actuals() {
    let session = open(XER);
    let exported = export_xer(&session, &from_xer(XER)).expect("exports");
    let opened = parse_xer(XER, &ProjectSelection::All).expect("reads");
    let again = parse_xer(&exported.bytes, &ProjectSelection::All).expect("reads back");

    // Activity 6 is under way and Activity 1 is complete in the source.
    for name in ["Activity 6", "Activity 1"] {
        let before = actuals(&opened, name);
        assert!(
            before.0.is_some(),
            "{name} has an actual start in the source"
        );
        assert_eq!(actuals(&again, name).0, before.0, "{name}: actual start");
        assert_eq!(actuals(&again, name).1, before.1, "{name}: actual finish");
    }
    let under_way = "Activity 6";
    assert_eq!(actuals(&again, under_way).2, actuals(&opened, under_way).2);
    assert!(
        exported
            .left_out
            .iter()
            .all(|line| !line.contains("actual dates"))
    );
}

/// A re-opened export lists each WBS node's rows by the Start column (gs#865), not by activity
/// code: the started Activity 6 (A1-6) comes before the not-started Activity 4 (A1-4), and the
/// complete, started Activity 1 leads. The starts are the engine's, so they ascend by `es`.
#[test]
fn a_re_opened_export_lists_a_nodes_rows_by_the_start_column() {
    let session = open(XER);
    let exported = export_xer(&session, &from_xer(XER)).expect("exports");
    let again = load_bytes(&exported.bytes, &ProjectSelection::All, &|| 0.0).expect("reads back");
    let node = again
        .meta
        .iter()
        .position(|m| m.name == "Work package 1")
        .expect("the WBS node");
    let rows = again.outline.children(TaskId(node as u32));
    let names: Vec<&str> = rows
        .iter()
        .map(|t| again.meta[t.0 as usize].name.as_str())
        .collect();
    let at = |name: &str| names.iter().position(|n| *n == name).expect(name);
    let starts: Vec<i64> = rows
        .iter()
        .map(|t| again.solution.dates[t.0 as usize].es)
        .collect();
    assert!(starts.is_sorted(), "{names:?} start at {starts:?}");
    assert_eq!(names[0], "Activity 1", "the started task leads: {names:?}");
    assert!(at("Activity 6") < at("Activity 4"), "{names:?}");
}

/// B and C have a start-no-earlier-than and a start-no-later-than a week after A, which starts the project.
#[test]
fn a_one_project_export_keeps_the_project_start() {
    let text = r#"{"format":"gantt","version":1,"name":"d","origin":19723,"data_date":null,"lag_calendar":{"calendar":0},"calendars":[{"week":[[[480,1020]],[[480,1020]],[[480,1020]],[[480,1020]],[[480,1020]],[],[]],"exceptions":[]}],"tasks":[{"duration":480,"calendar":0,"snet":null,"actual_start":null,"actual_finish":null,"remaining":null,"name":"A","parent":null,"outline_level":1,"code":null,"wbs_code":null,"activity_codes":[],"progress":0},{"duration":480,"calendar":0,"snet":10560,"actual_start":null,"actual_finish":null,"remaining":null,"limit":{"snlt":10560},"name":"B","parent":null,"outline_level":1,"code":null,"wbs_code":null,"activity_codes":[],"progress":0},{"duration":480,"calendar":0,"snet":10560,"actual_start":null,"actual_finish":null,"remaining":null,"limit":{"snlt":10560},"name":"C","parent":null,"outline_level":1,"code":null,"wbs_code":null,"activity_codes":[],"progress":0},{"duration":480,"calendar":0,"snet":null,"actual_start":null,"actual_finish":null,"remaining":null,"name":"D","parent":null,"outline_level":1,"code":null,"wbs_code":null,"activity_codes":[],"progress":0}],"links":[{"pred":0,"succ":3,"kind":"FS","lag":0,"lag_calendar":null}],"baseline":null,"view":{"collapsed":[]},"notes":[],"source":null}"#;
    let session = super::document_round_trip::session_of(
        app::document::read(text, &|| 0.0).expect("reads").loaded,
    );
    let extras = Extras {
        name: "d",
        notes: &[],
        source: None,
    };
    let exported = export_xer(&session, &extras).expect("exports");
    let again = load_bytes(&exported.bytes, &ProjectSelection::All, &|| 0.0).expect("reads back");
    assert_eq!(again.origin.day(), session.origin().day());
    for name in ["A", "B", "C", "D"] {
        let at = again.meta.iter().position(|m| m.name == name).expect(name);
        let start = session.solution().get(task(&session, name)).es;
        assert_eq!(again.solution.dates[at].es, start, "{name}");
    }
    assert_eq!(exported.left_out, Vec::<String>::new());
    for name in ["B", "C"] {
        let at = again.meta.iter().position(|m| m.name == name).expect(name);
        let read = &again.schedule.tasks()[at];
        assert_eq!((read.snet, read.limit.snlt), (Some(10560), Some(10560)));
    }
    let saturday = text.replace(r#""data_date":null"#, r#""data_date":7800"#);
    let loaded = app::document::read(&saturday, &|| 0.0)
        .expect("reads")
        .loaded;
    let dated = super::document_round_trip::session_of(loaded);
    let exported = export_xer(&dated, &extras).expect("exports");
    let again = load_bytes(&exported.bytes, &ProjectSelection::All, &|| 0.0).expect("reads back");
    assert_eq!(again.data_date, dated.data_date());
}

/// The generated XER's activity codes, a baseline of every task and two deadlines.
fn lossy() -> Session {
    let session = open(&xer::synth::write(&xer::synth::Spec::with_activities(7, 6)));
    let baseline = Baseline::snapshot(session.schedule(), session.solution(), None);
    let mut deadlines = vec![None; session.meta().len()];
    deadlines[0] = Some(60);
    deadlines[1] = Some(120);
    session.with_baseline(baseline).with_deadlines(deadlines)
}

#[test]
fn a_save_as_xer_lists_what_this_schedule_loses() {
    let session = lossy();
    let coded = (session.meta().iter())
        .filter(|m| !m.activity_codes.is_empty())
        .count();
    let tasks = session.meta().len();
    assert!(
        coded > 0 && coded < tasks,
        "{coded} of {tasks} tasks have codes"
    );
    let lines = xer_left_out(&session);
    assert_eq!(
        lines,
        [
            format!("the activity codes on {coded} tasks are not written"),
            format!("the baselines on {tasks} tasks are not written"),
            "the deadlines on 2 tasks are not written".to_string(),
        ]
    );
    let extras = Extras {
        name: "lossy",
        notes: &[],
        source: None,
    };
    let written = export_xer(&session, &extras).expect("exports");
    let all_listed = lines.iter().all(|line| written.left_out.contains(line));
    assert!(all_listed, "{:?}", written.left_out);
}

#[test]
fn a_save_as_xer_lists_only_what_the_schedule_has_and_nothing_when_it_has_none() {
    assert_eq!(xer_left_out(&open(MSPDI.as_bytes())), Vec::<String>::new());
    let mut deadlines = vec![None; open(MSPDI.as_bytes()).meta().len()];
    deadlines[0] = Some(60);
    assert_eq!(
        xer_left_out(&open(MSPDI.as_bytes()).with_deadlines(deadlines)),
        ["the deadlines on 1 task are not written"]
    );
}
