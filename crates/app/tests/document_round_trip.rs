//! The schedule document round trip (gs#739, decision 0010 item 6): save, open, save again gives
//! the same text, and the reopened session is the first one under the renumbering.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use app::{
    Loaded, Place, Session, Siblings,
    document::{Document, DocumentError, Extras, FORMAT, Source, SourceFormat, read, write},
    edit::Edit,
    fitted_timeline, load_bytes,
};
use engine::{LinkType, TaskId};
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

pub(super) fn session_of(loaded: Loaded) -> Session {
    let timeline = fitted_timeline(loaded.order, &loaded.solution, 1000.0, 400.0, 1.0);
    Session::new(
        loaded.schedule,
        loaded.solution,
        timeline,
        loaded.meta,
        || 0.0,
    )
    .with_outline(loaded.outline)
    .expect("the outline matches")
    .with_origin(loaded.origin)
    .with_data_date(loaded.data_date)
    .with_progress(loaded.progress)
}

fn reopened(document: Document) -> Session {
    let mut session = session_of(document.loaded);
    if let Some(baseline) = document.baseline {
        session = session.with_baseline(baseline);
    }
    session.with_collapsed(&document.collapsed)
}

/// A file opened with one task in progress: an XER carries no actuals, so the task is started
/// here, a day in with half its work left, and the schedule solved again.
fn open_in_progress(xer: &[u8]) -> (Session, TaskId) {
    let mut loaded =
        load_bytes(xer, &ProjectSelection::Unspecified, &|| 0.0).expect("the fixture loads");
    let started = TaskId(
        (0..loaded.meta.len() as u32)
            .find(|&i| loaded.schedule.tasks()[i as usize].duration >= 960)
            .expect("a task of two days or more"),
    );
    let before = loaded.solution.get(started).es;
    let half = loaded.schedule.tasks()[started.0 as usize].duration / 2;
    loaded
        .schedule
        .set_actual_start(started, Some(before))
        .unwrap();
    loaded.schedule.set_remaining(started, Some(half)).unwrap();
    loaded.solution = loaded.schedule.compute().expect("solves");
    loaded.order = timeline::order_by_start(&loaded.solution.dates);
    (session_of(loaded), started)
}

fn extras<'a>(notes: &'a [String], xer: &'a [u8]) -> Extras<'a> {
    Extras {
        name: "round trip \"quoted\"",
        notes,
        source: Some(Source {
            format: SourceFormat::Xer,
            bytes: xer,
        }),
    }
}

/// One edit of each kind, one undone, then a baseline and a collapsed summary.
fn edited(session: &mut Session) {
    let outline = session.outline().unwrap();
    let preorder = outline.preorder().to_vec();
    let leaves: Vec<TaskId> = preorder
        .iter()
        .copied()
        .filter(|&t| !outline.is_summary(t))
        .collect();
    let summaries: Vec<TaskId> = preorder
        .iter()
        .copied()
        .filter(|&t| outline.is_summary(t) && outline.parent(t).is_some())
        .collect();
    assert!(summaries.len() >= 2, "two work packages");
    let gone = leaves[2];
    let was_in = outline.parent(gone);
    let moved = leaves[3];
    let last = *leaves.last().unwrap();

    let mut edit = |e: Edit| session.edit(e).expect("the edit applies");
    edit(Edit::Rename {
        task: leaves[0],
        name: "Renamed \u{e9}\u{1F600}".into(),
    });
    edit(Edit::SetDuration {
        task: leaves[1],
        minutes: 1920,
    });
    edit(Edit::Delete { task: gone });
    edit(Edit::Restore {
        task: gone,
        place: Place {
            parent: was_in,
            after: None,
        },
    });
    edit(Edit::Reparent {
        task: moved,
        place: Place {
            parent: Some(summaries[1]),
            after: None,
        },
        siblings: Siblings::Keep,
    });
    edit(Edit::AddLink {
        pred: leaves[0],
        succ: last,
        kind: LinkType::SS,
        lag: 480,
    });
    let dep = session
        .schedule()
        .live_dependencies()
        .find(|(_, d)| d.succ != last && d.pred != moved && d.succ != moved)
        .map(|(dep, _)| dep)
        .expect("a link to remove");
    session.edit(Edit::RemoveLink { dep }).expect("removes");
    session.insert_below_selection().expect("inserts");
    assert!(session.undo().expect("undoes"), "the insert is undone");

    session
        .edit(Edit::SetBaseline { baseline: None })
        .expect("baseline");
    // After the baseline, so it differs from the dates now.
    session
        .edit(Edit::SetDuration {
            task: leaves[4],
            minutes: 4800,
        })
        .expect("later edit");
    assert!(session.toggle(summaries[0]), "a summary collapses");
}

#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "a and b are the two outlines"
)]
#[expect(
    clippy::too_many_lines,
    reason = "one test follows a document through save, open and save"
)]
fn save_open_save_gives_the_same_text() {
    let xer = synth_xer(&Spec::with_activities(7, 60));
    let (mut first, started) = open_in_progress(&xer);
    edited(&mut first);
    let notes = vec!["Unsupported: \"resource\" tables\n".to_string(), "é".into()];
    let extras = extras(&notes, &xer);

    let text = write(&first, &extras);
    let document = read(&text, &|| 0.0).expect("reads");
    assert_eq!(document.name, "round trip \"quoted\"");
    assert_eq!(document.notes, notes);
    assert_eq!(document.loaded.unsupported, notes);
    assert_eq!(document.source, Some((SourceFormat::Xer, xer.clone())));
    let second = reopened(document);
    assert_eq!(write(&second, &extras), text, "the second write differs");

    // The renumbering: the live tasks in outline order.
    let order = first.outline().unwrap().preorder().to_vec();
    let new = |old: TaskId| TaskId(order.iter().position(|&t| t == old).unwrap() as u32);
    let (a, b) = (first.outline().unwrap(), second.outline().unwrap());
    assert_eq!(second.schedule().live_task_count(), order.len());
    assert_eq!(first.data_date(), second.data_date());
    assert_eq!(first.date_text(10_000), second.date_text(10_000));
    assert_eq!(
        first.solution().project_finish,
        second.solution().project_finish
    );
    assert!(
        second.schedule().tasks()[new(started).0 as usize]
            .remaining
            .is_some(),
        "the task is still in progress"
    );
    for &old in &order {
        let n = new(old);
        let (x, y) = (first.solution().get(old), second.solution().get(n));
        assert_eq!(
            (x.es, x.ef, x.ls, x.lf, x.float),
            (y.es, y.ef, y.ls, y.lf, y.float)
        );
        let (mx, my) = (&first.meta()[old.0 as usize], &second.meta()[n.0 as usize]);
        assert_eq!(mx.name, my.name);
        assert_eq!((&mx.code, &mx.wbs_code), (&my.code, &my.wbs_code));
        assert_eq!(mx.outline_level, my.outline_level);
        assert_eq!(
            a.parent(old).map(new),
            b.parent(n),
            "outline parent of {old}"
        );
        assert_eq!(a.depth(old), b.depth(n));
        assert_eq!(first.is_collapsed(old), second.is_collapsed(n));
        assert_eq!(
            first.baseline().unwrap().task(old),
            second.baseline().unwrap().task(n),
            "baseline of {old}"
        );
        assert_eq!(
            first.timeline().progress().get(old.0 as usize),
            second.timeline().progress().get(n.0 as usize)
        );
        let (tx, ty) = (
            &first.schedule().tasks()[old.0 as usize],
            &second.schedule().tasks()[n.0 as usize],
        );
        assert_eq!(
            (
                tx.duration,
                tx.snet,
                tx.actual_start,
                tx.actual_finish,
                tx.remaining
            ),
            (
                ty.duration,
                ty.snet,
                ty.actual_start,
                ty.actual_finish,
                ty.remaining
            )
        );
    }
    assert_eq!(
        first.baseline().unwrap().project_finish(),
        second.baseline().unwrap().project_finish()
    );
    assert!(
        order.iter().any(|&t| first.is_collapsed(t)),
        "one collapsed"
    );
    assert_eq!(
        first.schedule().live_dependencies().count(),
        second.schedule().live_dependencies().count()
    );
    assert_eq!(
        second.history_len(),
        (0, 0),
        "an opened document has no history"
    );
    assert!(!second.can_undo() && !second.can_redo());
    // The collapsed rows are the rows the table shows.
    let rows = |s: &Session, map: &dyn Fn(u32) -> u32| -> Vec<u32> {
        s.timeline().order().iter().map(|&t| map(t)).collect()
    };
    assert_eq!(rows(&first, &|t| new(TaskId(t)).0), rows(&second, &|t| t));
}

pub(super) fn document() -> (String, Vec<u8>) {
    let xer = synth_xer(&Spec::with_activities(7, 60));
    let (mut session, _) = open_in_progress(&xer);
    edited(&mut session);
    (write(&session, &extras(&[], &xer)), xer)
}

pub(super) fn refusal(text: &str) -> DocumentError {
    match read(text, &|| 0.0) {
        Err(error) => error,
        Ok(_) => panic!("the reader accepted {text:.80}"),
    }
}

#[test]
fn another_format_and_a_newer_version_are_refused_by_name() {
    let (text, _) = document();
    let other = text.replacen("\"format\":\"gantt\"", "\"format\":\"other\"", 1);
    assert_eq!(
        refusal(&other).to_string(),
        "not a schedule document: format is \"other\""
    );
    let newer = text.replacen("\"version\":1", "\"version\":2", 1);
    assert_eq!(refusal(&newer), DocumentError::Newer(2));
    assert_eq!(
        refusal(&newer).to_string(),
        "this document is version 2; this app reads version 1"
    );
    // Not a schedule document at all: other JSON, and not JSON.
    assert!(
        refusal("[1,2]")
            .to_string()
            .contains("not a schedule document")
    );
    assert!(
        refusal("{\"tasks\":[]}")
            .to_string()
            .contains("no \"format\" key")
    );
    assert!(matches!(refusal("<xml/>"), DocumentError::Syntax(_)));
    assert!(
        refusal("{\"format\":7}")
            .to_string()
            .contains("format is 7")
    );
}

/// A document whose `format` is the retired project name is refused, not read (gs#1120). The name
/// is built at run time, so this file does not hold it.
#[test]
fn a_document_with_the_retired_format_is_refused_as_not_a_document() {
    let (text, _) = document();
    let marker = format!("{{\"format\":\"{FORMAT}\",");
    assert!(text.starts_with(&marker), "{text:.40}");
    let retired = ["man", "tis"].concat();
    let old = text.replacen(&marker, &format!("{{\"format\":\"{retired}\","), 1);
    assert_ne!(old, text);
    assert!(matches!(refusal(&old), DocumentError::NotDocument(_)));
}

#[test]
fn a_document_cut_at_every_997th_byte_is_an_error_not_a_panic() {
    let (text, _) = document();
    let bytes = text.as_bytes();
    assert!(bytes.len() > 20_000, "the cuts reach well into the text");
    for end in (0..bytes.len()).step_by(997) {
        let cut = String::from_utf8_lossy(&bytes[..end]);
        assert!(read(&*cut, &|| 0.0).is_err(), "cut at {end} was accepted");
    }
}

#[test]
fn a_field_that_cannot_be_read_is_named_by_its_path() {
    let (text, _) = document();
    let change = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        edit(&mut value);
        refusal(&value.to_string()).to_string()
    };
    let message = change(&|v| {
        v["tasks"][12].as_object_mut().unwrap().remove("remaining");
    });
    assert!(
        message.contains("tasks[12].remaining: missing key"),
        "{message}"
    );
    let message = change(&|v| v["tasks"][3]["duration"] = "long".into());
    assert!(
        message.contains("tasks[3].duration: expected an integer"),
        "{message}"
    );
    let message = change(&|v| v["extra"] = 1.into());
    assert!(message.contains("extra: unknown key"), "{message}");
    let message = change(&|v| v["links"][0]["kind"] = "XX".into());
    assert!(message.contains("links[0].kind"), "{message}");
    let message = change(&|v| v["tasks"][3]["progress"] = 101.into());
    assert!(message.contains("tasks[3].progress"), "{message}");
}

#[test]
fn what_the_engine_or_the_outline_refuses_is_a_message_not_a_panic() {
    let (text, _) = document();
    let broken = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        edit(&mut value);
        let error = refusal(&value.to_string());
        assert!(matches!(error, DocumentError::Invalid(_)), "{error}");
        error.to_string()
    };
    assert!(broken(&|v| v["tasks"][2]["calendar"] = 9.into()).contains("tasks[2]"));
    assert!(broken(&|v| v["tasks"][1]["parent"] = 1.into()).contains("outline"));
    assert!(broken(&|v| v["tasks"][1]["parent"] = 99_999.into()).contains("tasks[1].parent"));
    assert!(
        broken(&|v| v["links"][0]["succ"] = v["links"][0]["pred"].clone()).contains("links[0]")
    );
    assert!(broken(&|v| v["links"][0]["pred"] = 99_999.into()).contains("links[0].pred"));
    assert!(
        broken(&|v| v["view"]["collapsed"] = serde_json::json!([5_000]))
            .contains("view.collapsed[0]: no task 5000")
    );
    assert!(broken(&|v| v["baseline"]["tasks"] = serde_json::json!([])).contains("baseline.tasks"));
    assert!(broken(&|v| v["source"]["base64"] = "Zm9".into()).contains("source.base64"));
}

pub(super) fn changed(edit: &dyn Fn(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(&document().0).unwrap();
    edit(&mut value);
    value.to_string()
}

#[test]
fn a_document_with_no_links_or_no_tasks_reads() {
    let linkless = changed(&|v| v["links"] = serde_json::json!([]));
    let read_back = read(&linkless, &|| 0.0).expect("no links reads");
    assert_eq!(read_back.loaded.schedule.dependencies(), []);
    let empty = changed(&|v| {
        v["tasks"] = serde_json::json!([]);
        v["links"] = serde_json::json!([]);
        v["baseline"] = serde_json::Value::Null;
        v["view"]["collapsed"] = serde_json::json!([]);
    });
    let none = read(&empty, &|| 0.0).expect("no tasks reads");
    assert_eq!(none.loaded.meta, []);
}

/// A parent or an end of a link names a task, so it is checked once the tasks are read: a field
/// the reader cannot read, in a later task or a later field of the link, is named first.
#[test]
fn a_parent_or_link_end_with_no_task_is_named_after_the_fields_read_before_the_count() {
    let named = |edit: &dyn Fn(&mut serde_json::Value)| refusal(&changed(edit)).to_string();
    let message = named(&|v| {
        v["tasks"][1]["parent"] = 99_999.into();
        v["tasks"][5]["duration"] = "long".into();
    });
    assert!(message.contains("tasks[5].duration"), "{message}");
    let message = named(&|v| {
        v["tasks"][4]["parent"] = 88_888.into();
        v["tasks"][2]["parent"] = 99_999.into();
    });
    assert!(
        message.contains("tasks[2].parent: no task 99999"),
        "{message}"
    );
    let message = named(&|v| {
        v["links"][0]["pred"] = 99_999.into();
        v["links"][0]["succ"] = 88_888.into();
    });
    assert!(
        message.contains("links[0].pred: no task 99999"),
        "{message}"
    );
    let message = named(&|v| {
        v["links"][0]["succ"] = 88_888.into();
        v["links"][0]["lag"] = "long".into();
    });
    assert!(message.contains("links[0].lag"), "{message}");
}

/// Write, read and write again; the texts must match and the history must be empty.
fn reopen_identically(session: &Session, xer: &[u8]) -> Session {
    let extras = extras(&[], xer);
    let text = write(session, &extras);
    let again = reopened(read(&text, &|| 0.0).expect("a saved document reads"));
    assert_eq!(write(&again, &extras), text, "the second write differs");
    assert_eq!(again.history_len(), (0, 0));
    again
}

#[test]
fn a_schedule_with_every_task_deleted_saves_and_opens() {
    let xer = synth_xer(&Spec::with_activities(7, 4));
    let (mut session, _) = open_in_progress(&xer);
    let preorder = session.outline().unwrap().preorder().to_vec();
    for &task in preorder.iter().rev() {
        session.edit(Edit::Delete { task }).expect("deletes");
    }
    assert_eq!(session.schedule().live_task_count(), 0);
    let again = reopen_identically(&session, &xer);
    assert_eq!(again.schedule().live_task_count(), 0);
}

#[test]
fn a_collapse_flag_left_on_an_emptied_summary_survives_save_and_open() {
    let xer = synth_xer(&Spec::with_activities(7, 4));
    let (mut session, _) = open_in_progress(&xer);
    let outline = session.outline().unwrap();
    let package = outline.preorder()[1];
    let children = outline.children(package).to_vec();
    assert!(session.toggle(package), "the work package collapses");
    for task in children {
        let place = Place {
            parent: None,
            after: None,
        };
        let siblings = Siblings::Keep;
        session
            .edit(Edit::Reparent {
                task,
                place,
                siblings,
            })
            .expect("moves out");
    }
    assert!(!session.outline().unwrap().is_summary(package));
    assert!(session.is_collapsed(package), "the flag stays on the task");
    let again = reopen_identically(&session, &xer);
    let moved: Vec<TaskId> = again
        .outline()
        .unwrap()
        .preorder()
        .iter()
        .copied()
        .filter(|&t| again.is_collapsed(t))
        .collect();
    assert_eq!(moved.len(), 1, "and comes back on one task");
    assert_eq!(again.meta()[moved[0].0 as usize].name, "Work package 1");
}

#[test]
fn a_baseline_without_float_round_trips() {
    let file = include_str!("fixtures/baselined.xml");
    let mut loaded = load_bytes(file.as_bytes(), &ProjectSelection::Unspecified, &|| 0.0)
        .expect("the fixture loads");
    let baseline = loaded.baseline.take().expect("the file's baseline");
    let first = session_of(loaded).with_baseline(baseline);
    let extras = Extras {
        name: "baselined",
        notes: &[],
        source: Some(Source {
            format: SourceFormat::Mspdi,
            bytes: file.as_bytes(),
        }),
    };

    let text = write(&first, &extras);
    assert!(text.contains(",null]"), "the missing float is a null");
    let second = reopened(read(&text, &|| 0.0).expect("a null float reads"));
    let held = |s: &Session| *s.baseline().unwrap().task(TaskId(0)).unwrap();
    assert_eq!(held(&second), held(&first));
    assert_eq!(held(&second).float, None);
    assert_eq!(write(&second, &extras), text, "the second write differs");
}

#[test]
fn the_baseline_data_date_round_trips() {
    let file = include_str!("fixtures/baselined.xml");
    let mut loaded = load_bytes(file.as_bytes(), &ProjectSelection::Unspecified, &|| 0.0)
        .expect("the fixture loads");
    loaded.baseline = None;
    let mut first = session_of(loaded);
    first.set_data_date_text("2026-10-14").expect("sets");
    first
        .edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let at = first.data_date().expect("a data date");
    let extras = Extras {
        name: "dated",
        notes: &[],
        source: None,
    };

    let text = write(&first, &extras);
    let second = reopened(read(&text, &|| 0.0).expect("reads"));
    assert_eq!(second.baseline().unwrap().data_date(), Some(at));
    assert_eq!(write(&second, &extras), text, "the second write differs");

    // A baseline set with no data date says so, and a file that has no such key still reads.
    let mut plain = reopened(read(&text, &|| 0.0).expect("reads"));
    plain.set_data_date_text("").expect("clears");
    plain
        .edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let undated = write(&plain, &extras);
    assert!(undated.contains("\"data_date\":null,\"project_finish\""));
    let old = undated.replacen(
        "\"data_date\":null,\"project_finish\"",
        "\"project_finish\"",
        1,
    );
    let third = reopened(read(&old, &|| 0.0).expect("an older file reads"));
    assert_eq!(third.baseline().unwrap().data_date(), None);
}

/// The stress fixture has no outline, so its document carries no parents: with them `read` refuses
/// it, because the stress links touch tasks that other tasks name as parents (tl1344s).
#[test]
fn a_session_with_no_outline_writes_a_document_that_reads() {
    let (mut schedule, _head) = engine::stress(app::SEED, 200);
    let solution = schedule.compute().expect("generated schedules are valid");
    let order = timeline::order_by_start(&solution.dates);
    let timeline = fitted_timeline(order, &solution, 1000.0, 400.0, 1.0);
    let meta = app::names::generate(app::SEED, schedule.tasks().len());
    let session = Session::new(schedule, solution, timeline, meta, || 0.0);
    assert!(session.outline().is_none());
    let extras = Extras {
        name: "stress",
        notes: &[],
        source: None,
    };
    let text = write(&session, &extras);
    let again = reopened(read(&text, &|| 0.0).expect("the document reads"));
    assert_eq!(
        again.schedule().tasks().len(),
        session.schedule().tasks().len()
    );
}
