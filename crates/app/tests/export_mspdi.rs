//! Writing the session as an MS Project XML file (gs#703): the file carries the edits.

use app::document::{Source, SourceFormat};
use app::{
    Loaded, Session, document::Extras, edit::Edit, export::export_mspdi, fitted_timeline,
    load_bytes,
};
use engine::{DepId, TaskId};
use import::{ProjectSelection, parse_mspdi};

const FIXTURE: &str = include_str!("../../import/tests/fixtures/summary.xml");
const BASELINED: &str = include_str!("fixtures/baselined.xml");

fn open(bytes: &[u8]) -> Session {
    open_with(bytes, |_| {})
}

/// Open `bytes`, change what was loaded, and solve again.
fn open_with(bytes: &[u8], change: impl FnOnce(&mut Loaded)) -> Session {
    let mut loaded = load_bytes(bytes, &ProjectSelection::Unspecified, &|| 0.0).expect("loads");
    change(&mut loaded);
    loaded.solution = loaded.schedule.compute().expect("solves");
    let timeline = fitted_timeline(loaded.order, &loaded.solution, 1000.0, 400.0, 1.0);
    let session = Session::new(
        loaded.schedule,
        loaded.solution,
        timeline,
        loaded.meta,
        || 0.0,
    );
    match loaded.baseline {
        Some(baseline) => session.with_baseline(baseline),
        None => session,
    }
    .with_outline(loaded.outline)
    .expect("the outline matches")
    .with_origin(loaded.origin)
    .with_data_date(loaded.data_date)
    .with_progress(loaded.progress)
}

/// What a schedule says: each task's name, duration and outline level in outline order, then each
/// live link as predecessor name, successor name, type and lag, sorted.
type Facts = (Vec<(String, i64, u8)>, Vec<(String, String, String, i64)>);

fn of_session(session: &Session) -> Facts {
    let schedule = session.schedule();
    let name = |task: TaskId| session.meta()[task.0 as usize].name.clone();
    let order = session.outline().expect("an outline").preorder().to_vec();
    let tasks = order
        .iter()
        .map(|&t| {
            let meta = &session.meta()[t.0 as usize];
            (
                meta.name.clone(),
                schedule.tasks()[t.0 as usize].duration,
                meta.outline_level,
            )
        })
        .collect();
    let mut links: Vec<_> = schedule
        .dependencies()
        .iter()
        .enumerate()
        .filter(|&(i, d)| {
            schedule.is_dependency_live(DepId(i as u32))
                && !schedule.is_deleted(d.pred)
                && !schedule.is_deleted(d.succ)
        })
        .map(|(_, d)| (name(d.pred), name(d.succ), format!("{:?}", d.kind), d.lag))
        .collect();
    links.sort();
    (tasks, links)
}

fn of_file(bytes: &[u8]) -> Facts {
    let imported = parse_mspdi(std::str::from_utf8(bytes).expect("UTF-8")).expect("reads back");
    let schedule = &imported.schedule;
    let name = |task: TaskId| imported.tasks[task.0 as usize].name.clone();
    let tasks = imported
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            (
                t.name.clone(),
                schedule.tasks()[i].duration,
                t.outline_level,
            )
        })
        .collect();
    let mut links: Vec<_> = schedule
        .dependencies()
        .iter()
        .map(|d| (name(d.pred), name(d.succ), format!("{:?}", d.kind), d.lag))
        .collect();
    links.sort();
    (tasks, links)
}

fn task(session: &Session, name: &str) -> TaskId {
    let at = session
        .meta()
        .iter()
        .position(|m| m.name == name)
        .expect(name);
    TaskId(at as u32)
}

fn extras(notes: &[String]) -> Extras<'_> {
    Extras {
        name: "summary",
        notes,
        source: None,
    }
}

#[test]
fn an_edited_session_exports_its_edits() {
    let mut session = open(FIXTURE.as_bytes());
    let first = task(&session, "First child");
    let second = task(&session, "Second child");
    let third = task(&session, "Third child");
    session
        .edit(Edit::Rename {
            task: first,
            name: "Renamed child".into(),
        })
        .expect("renames");
    session
        .edit(Edit::SetDuration {
            task: second,
            minutes: 3 * 480,
        })
        .expect("changes the duration");
    session.edit(Edit::Delete { task: third }).expect("deletes");

    let exported = export_mspdi(&session, &extras(&[])).expect("exports");
    let (tasks, links) = of_file(&exported.bytes);
    assert_eq!(of_session(&session), (tasks.clone(), links.clone()));
    assert!(tasks.contains(&("Renamed child".into(), 480, 2)));
    assert!(tasks.contains(&("Second child".into(), 3 * 480, 2)));
    assert!(
        tasks
            .iter()
            .all(|t| t.0 != "Third child" && t.0 != "First child")
    );
    assert_eq!(tasks.len(), 4);
    assert_eq!(
        links.len(),
        1,
        "the link into the deleted task is gone: {links:?}"
    );
}

#[test]
fn an_xer_file_with_a_renamed_leaf_reopens_from_the_xml_with_the_new_name() {
    const XER: &[u8] = include_bytes!("../../../packages/gantt/tests/fixtures/one-project.xer");
    let mut session = open(XER);
    let outline = session.outline().expect("an outline");
    let leaf = session
        .timeline()
        .order()
        .iter()
        .map(|&t| TaskId(t))
        .find(|&t| !outline.is_summary(t))
        .expect("a leaf row");
    session
        .edit(Edit::Rename {
            task: leaf,
            name: "Exported and renamed".into(),
        })
        .expect("renames");
    let source = Source {
        format: SourceFormat::Xer,
        bytes: XER,
    };
    let extras = Extras {
        name: "one-project",
        notes: &[],
        source: Some(source),
    };
    let exported = export_mspdi(&session, &extras).expect("exports");
    let reopened = open(&exported.bytes);
    let names = |s: &Session| s.meta().iter().map(|m| m.name.clone()).collect::<Vec<_>>();
    assert!(names(&reopened).contains(&"Exported and renamed".to_string()));
    assert_eq!(of_session(&reopened).0, of_session(&session).0);
}

#[test]
fn an_unedited_session_exports_what_it_opened() {
    let session = open(FIXTURE.as_bytes());
    let exported = export_mspdi(&session, &extras(&[])).expect("exports");
    let written = of_file(&exported.bytes);
    assert_eq!(written, of_file(&export_of_fixture()));
    assert_eq!(written, of_session(&session));
    assert_eq!(written.0.len(), 5);
    assert_eq!(written.1.len(), 2);
}

/// The fixture as the importer reads it, written back by the importer's own writer.
fn export_of_fixture() -> Vec<u8> {
    let imported = parse_mspdi(FIXTURE).expect("reads");
    import::export_mspdi(&imported).expect("writes").bytes
}

/// The start of every task in outline order, as the written file schedules them.
fn starts_of_file(bytes: &[u8]) -> Vec<engine::Instant> {
    let mut read = parse_mspdi(std::str::from_utf8(bytes).expect("UTF-8")).expect("reads back");
    read.schedule
        .set_data_date(read.data_date)
        .expect("in range");
    let solution = read.schedule.compute().expect("solves");
    (0..read.tasks.len())
        .map(|i| solution.get(TaskId(i as u32)).es)
        .collect()
}

/// The start of every task in outline order, as the session has them.
fn starts_of_session(session: &Session) -> Vec<engine::Instant> {
    let order = session.outline().expect("an outline").preorder().to_vec();
    order
        .iter()
        .map(|&t| session.solution().get(t).es)
        .collect()
}

/// Three independent tasks, one at the project start and two moved to the same later day.
fn three_tasks(session: &mut Session) {
    let mut at = |name: &str, snet: Option<engine::Instant>| {
        let task = task(session, name);
        session.edit(Edit::Move { task, snet }).expect("moves");
    };
    at("Second", Some(480 + 1440));
    at("Third", Some(480 + 1440));
}

fn three_task_file() -> String {
    let (head, _) = FIXTURE.split_once("<Tasks>").expect("tasks");
    let task = |uid: u32, name: &str| {
        format!(
            "<Task><UID>{uid}</UID><Name>{name}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat></Task>"
        )
    };
    format!(
        "{head}<Tasks>{}{}{}</Tasks></Project>",
        task(1, "First"),
        task(2, "Second"),
        task(3, "Third")
    )
}

#[test]
fn a_task_kept_earlier_than_the_others_keeps_its_start() {
    let mut session = open(three_task_file().as_bytes());
    three_tasks(&mut session);
    let before = starts_of_session(&session);
    assert_eq!(before, [480, 1920, 1920]);

    let exported = export_mspdi(&session, &extras(&[])).expect("exports");
    assert_eq!(starts_of_file(&exported.bytes), before);
    assert!(
        exported
            .left_out
            .iter()
            .all(|l| !l.contains("different start"))
    );
}

#[test]
fn progress_follows_the_actuals_not_the_timeline() {
    let session = open_with(FIXTURE.as_bytes(), |loaded| {
        let started = loaded.solution.get(TaskId(2)).es;
        loaded
            .schedule
            .set_actual_start(TaskId(1), Some(0))
            .unwrap();
        loaded
            .schedule
            .set_actual_finish(TaskId(1), Some(480))
            .unwrap();
        loaded
            .schedule
            .set_actual_start(TaskId(2), Some(started))
            .unwrap();
        loaded.schedule.set_remaining(TaskId(2), Some(120)).unwrap();
        loaded.progress = vec![0; loaded.progress.len()];
    });
    let exported = export_mspdi(&session, &extras(&[])).expect("exports");
    let read = parse_mspdi(std::str::from_utf8(&exported.bytes).unwrap()).expect("reads back");
    let percent = |name: &str| read.tasks.iter().find(|t| t.name == name).unwrap().progress;
    assert_eq!(percent("First child"), Some(100));
    assert_eq!(percent("Second child"), Some(75));
    assert_eq!(percent("Third child"), Some(0));
    // The actuals are written, so nothing is listed and the file reads back started and finished.
    let actual = |line: &&String| line.contains("actual") || line.contains("different start");
    assert_eq!(exported.left_out.iter().filter(actual).count(), 0);
    let task = |name: &str| read.tasks.iter().find(|t| t.name == name).unwrap();
    assert!(task("First child").actual_finish.is_some());
    assert!(task("Second child").in_progress);
    assert_eq!(task("Second child").remaining, Some(120));
}

#[test]
fn activity_ids_and_codes_are_reported_as_left_out() {
    let session = open_with(FIXTURE.as_bytes(), |loaded| {
        loaded.meta[1].code = Some("A1010".into());
        loaded.meta[1].activity_codes = vec![import::ActivityCode {
            code_type: "Discipline".into(),
            value: "ELEC".into(),
        }];
    });
    let exported = export_mspdi(&session, &extras(&[])).expect("exports");
    assert!(
        exported
            .left_out
            .iter()
            .any(|l| l.contains("activity ids and activity codes on 1 task")),
        "{:?}",
        exported.left_out
    );
}

fn left_out_of(file: &str, source: bool) -> Vec<String> {
    let session = open(file.as_bytes());
    let source = source.then_some(Source {
        format: SourceFormat::Mspdi,
        bytes: file.as_bytes(),
    });
    let extras = Extras {
        name: "baselined",
        notes: &[],
        source,
    };
    export_mspdi(&session, &extras).expect("exports").left_out
}

#[test]
fn a_baseline_saved_in_the_opened_file_is_written_back() {
    let left_out = left_out_of(BASELINED, true);
    assert!(
        left_out.iter().all(|l| !l.contains("baseline")),
        "{left_out:?}"
    );
    let session = open(BASELINED.as_bytes());
    let source = Source {
        format: SourceFormat::Mspdi,
        bytes: BASELINED.as_bytes(),
    };
    let extras = Extras {
        name: "baselined",
        notes: &[],
        source: Some(source),
    };
    let written =
        String::from_utf8(export_mspdi(&session, &extras).expect("exports").bytes).expect("text");
    let (before, after) = (
        parse_mspdi(BASELINED).expect("reads"),
        parse_mspdi(&written).expect("reads back"),
    );
    let dates = |file: &import::Imported| {
        file.tasks
            .iter()
            .map(|t| (t.baseline_start, t.baseline_finish))
            .collect::<Vec<_>>()
    };
    assert!(dates(&before)[0].0.is_some(), "the fixture has a baseline");
    assert_eq!(dates(&after), dates(&before));
}

#[test]
fn a_file_without_a_baseline_exports_with_no_baseline_note() {
    assert!(
        left_out_of(FIXTURE, true)
            .iter()
            .all(|l| !l.contains("baseline"))
    );
    // No source, nothing to compare: the export is what it was.
    assert!(
        left_out_of(BASELINED, false)
            .iter()
            .all(|l| !l.contains("baseline"))
    );
}

#[test]
fn a_baseline_0_without_dates_is_reported_as_left_out() {
    let cost_only = FIXTURE.replacen(
        "<Name>First child</Name>",
        "<Name>First child</Name><Baseline><Number>0</Number><Cost>100</Cost></Baseline>",
        1,
    );
    assert_ne!(cost_only, FIXTURE);
    let left_out = left_out_of(&cost_only, true);
    assert!(
        left_out.iter().any(|l| l.contains("baseline")),
        "{left_out:?}"
    );
    // Baseline 1 is not the one the importer reads, so it is not reported either.
    let other = cost_only.replace("<Number>0</Number>", "<Number>1</Number>");
    assert!(
        left_out_of(&other, true)
            .iter()
            .all(|l| !l.contains("baseline"))
    );
}

/// `FIXTURE` with `baseline` written after its first child's name.
fn with_baseline(baseline: &str) -> String {
    let named = "<Name>First child</Name>";
    let file = FIXTURE.replacen(named, &format!("{named}{baseline}"), 1);
    assert_ne!(file, FIXTURE);
    file
}

fn says_baseline(file: &str) -> bool {
    left_out_of(file, true)
        .iter()
        .any(|l| l.contains("baseline"))
}

#[test]
fn a_baseline_0_written_another_way_in_xml_is_still_reported() {
    for baseline in [
        "<Baseline><Number>0</Number><Cost>1</Cost></Baseline>",
        "<Baseline xmlns:x=\"urn:x\"\n><Number> 0 </Number></Baseline >",
        "<x:Baseline><x:Number>0</x:Number></x:Baseline>",
        "<Baseline><Number><![CDATA[0]]></Number></Baseline>",
        "<Baseline note=\"a > b\"><Number>0</Number></Baseline>",
    ] {
        let file = with_baseline(baseline);
        assert!(says_baseline(&file), "{baseline}");
        let spaced = file.replacen("<Tasks>", "<Tasks >", 1);
        assert_ne!(spaced, file);
        assert!(says_baseline(&spaced), "{baseline} under <Tasks >");
    }
}

#[test]
fn a_baseline_that_is_not_markup_or_not_baseline_0_is_not_reported() {
    for baseline in [
        "<!-- <Baseline><Number>0</Number></Baseline> -->",
        "<Notes><![CDATA[<Baseline><Number>0</Number></Baseline>]]></Notes>",
        "<Baseline><Number>1</Number><Cost>1</Cost></Baseline>",
        "<Baseline><Cost>1</Cost></Baseline>",
        "<Baseline/>",
    ] {
        assert!(!says_baseline(&with_baseline(baseline)), "{baseline}");
    }
}
