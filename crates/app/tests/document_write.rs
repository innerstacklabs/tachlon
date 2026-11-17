//! The schedule document writer (gs#738): a schedule, its edits and its baseline as one JSON text.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{
    Session,
    document::{Extras, Source, SourceFormat, write},
    edit::Edit,
    fitted_timeline, load_bytes,
};
use common::xer_text::rewrite;
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

fn open(bytes: &[u8]) -> Session {
    let loaded = load_bytes(bytes, &ProjectSelection::Unspecified, &|| 0.0).expect("loads");
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

/// The fixture with no actual dates: an activity with actuals is fixed at them, and these tests
/// measure what an edit moves.
fn not_started(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).expect("UTF-8");
    let text = rewrite(&text, "TASK", "act_start_date", |_, _| Some(String::new()));
    rewrite(&text, "TASK", "act_end_date", |_, _| Some(String::new())).into_bytes()
}

fn extras<'a>(notes: &'a [String], source: Option<Source<'a>>) -> Extras<'a> {
    Extras {
        name: "four",
        notes,
        source,
    }
}

#[test]
fn a_small_schedule_writes_the_expected_text() {
    let mut session = open(&not_started(&synth_xer(&Spec::with_activities(7, 4))));
    // Preorder is project, work package, then the four activities: the second activity goes, so
    // the ones after it move down a place and their links follow.
    let second = session.outline().unwrap().preorder()[3];
    session
        .edit(Edit::Delete { task: second })
        .expect("deletes");
    session.insert_below_selection().expect("inserts");
    let text = write(&session, &extras(&[], None));
    assert_eq!(text, EXPECTED);
}

/// The text of the document, key by key, as a reader would see it.
const EXPECTED: &str = concat!(
    "{\"format\":\"gantt\",",
    "\"version\":1,",
    "\"name\":\"four\",",
    "\"origin\":19723,",
    "\"data_date\":480,",
    "\"lag_calendar\":{\"calendar\":0},",
    "\"calendars\":[{\"week\":[[[480,720],[780,1020]],[[480,720],[780,1020]],[[480,720],[780,1020]],[[480,720],[780,1020]],[[480,720],[780,1020]],[],[]],\"exceptions\":[[0,[]],[1,[[540,720]]],[2,[[480,720],[780,900]]]]}],",
    "\"tasks\":[{\"duration\":0,\"calendar\":0,\"snet\":null,\"actual_start\":null,\"actual_finish\":null,\"remaining\":null,",
    "\"name\":\"Café Project 1\",\"parent\":null,\"outline_level\":1,\"code\":null,\"wbs_code\":\"WBS-1-0\",\"activity_codes\":[],\"progress\":0},{\"duration\":0,\"calendar\":0,\"snet\":null,\"actual_start\":null,\"actual_finish\":null,\"remaining\":null,",
    "\"name\":\"Work package 1\",\"parent\":0,\"outline_level\":2,\"code\":null,\"wbs_code\":\"WBS-1-0.WBS-1-1\",\"activity_codes\":[],\"progress\":0},{\"duration\":3840,\"calendar\":0,\"snet\":480,\"actual_start\":null,\"actual_finish\":null,\"remaining\":null,",
    "\"name\":\"Activity 1\",\"parent\":1,\"outline_level\":3,\"code\":\"A1-1\",\"wbs_code\":\"WBS-1-0.WBS-1-1\",\"activity_codes\":[[\"Discipline\",\"D0\"]],\"progress\":0},{\"duration\":3360,\"calendar\":0,\"snet\":480,\"actual_start\":null,\"actual_finish\":null,\"remaining\":null,",
    "\"name\":\"Activity 3\",\"parent\":1,\"outline_level\":3,\"code\":\"A1-3\",\"wbs_code\":\"WBS-1-0.WBS-1-1\",\"activity_codes\":[[\"Discipline\",\"D2\"]],\"progress\":0},{\"duration\":1920,\"calendar\":0,\"snet\":480,\"actual_start\":null,\"actual_finish\":null,\"remaining\":null,",
    "\"name\":\"Activity 4\",\"parent\":1,\"outline_level\":3,\"code\":\"A1-4\",\"wbs_code\":\"WBS-1-0.WBS-1-1\",\"activity_codes\":[[\"Discipline\",\"D3\"]],\"progress\":0},{\"duration\":480,\"calendar\":0,\"snet\":null,\"actual_start\":null,\"actual_finish\":null,\"remaining\":null,",
    "\"name\":\"New task\",\"parent\":null,\"outline_level\":1,\"code\":\"A1-5\",\"wbs_code\":null,\"activity_codes\":[],\"progress\":0}],",
    "\"links\":[{\"pred\":2,\"succ\":3,\"kind\":\"FS\",\"lag\":0,",
    "\"lag_calendar\":null}],",
    "\"baseline\":null,",
    "\"view\":{\"collapsed\":[]},",
    "\"notes\":[],",
    "\"source\":null}",
);

fn four_task_edited() -> Session {
    let mut session = open(&not_started(&synth_xer(&Spec::with_activities(7, 4))));
    let second = session.outline().unwrap().preorder()[3];
    session
        .edit(Edit::Delete { task: second })
        .expect("deletes");
    session.insert_below_selection().expect("inserts");
    session
}

#[test]
fn writing_twice_gives_the_same_text_and_it_is_valid_json() {
    let session = four_task_edited();
    let notes = vec!["Unsupported: \"resource\" tables\n".to_string()];
    let extras = extras(&notes, None);
    let text = write(&session, &extras);
    assert_eq!(text, write(&session, &extras));
    let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(value["format"], "gantt");
    assert_eq!(value["version"], 1);
    assert_eq!(value["notes"][0], notes[0].as_str());
    assert_eq!(value["tasks"].as_array().unwrap().len(), 6);
}

#[test]
fn baseline_collapsed_rows_and_source_are_written_by_new_id() {
    let mut session = open(&not_started(&synth_xer(&Spec::with_activities(7, 4))));
    session
        .edit(Edit::SetBaseline { baseline: None })
        .expect("baseline");
    let preorder = session.outline().unwrap().preorder().to_vec();
    session
        .edit(Edit::Delete { task: preorder[3] })
        .expect("deletes");
    assert!(session.toggle(preorder[1]), "the work package collapses");
    let source = Source {
        format: SourceFormat::Xer,
        bytes: b"fo",
    };
    let text = write(&session, &extras(&[], Some(source)));
    let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(value["view"]["collapsed"], serde_json::json!([1]));
    assert_eq!(
        value["source"],
        serde_json::json!({"format": "xer", "base64": "Zm8="})
    );
    // The baseline was set before the delete, so it is indexed by the old ids. The text holds one
    // entry per live task, at the task's new id: entry k is what the baseline holds for the task
    // at outline position k, and the deleted task has no entry.
    let baseline = &value["baseline"]["tasks"];
    let snapshot = session.baseline().expect("a baseline is set");
    assert_eq!(
        value["baseline"]["project_finish"],
        snapshot.project_finish()
    );
    let live = session.outline().unwrap().preorder();
    assert_eq!(baseline.as_array().unwrap().len(), live.len());
    assert!(!live.contains(&preorder[3]));
    for (new, &old) in live.iter().enumerate() {
        let held = snapshot.task(old).expect("a task live at the baseline");
        assert_eq!(
            baseline[new],
            serde_json::json!([held.es, held.ef, held.float]),
            "entry {new} is the baseline of old task {}",
            old.0
        );
    }
}

/// Bytes of `bytes` after `gzip -6 -n`, the setting the plan names; `None` without a `gzip`.
fn gzip_len(bytes: &[u8]) -> Option<usize> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("gzip")
        .args(["-6", "-n", "-c"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let input = bytes.to_vec();
    let feeder = std::thread::spawn(move || stdin.write_all(&input));
    let out = child.wait_with_output().ok()?;
    feeder.join().ok()?.ok()?;
    Some(out.stdout.len())
}

/// The size report of decision 0010 item 7 and ruling 6. Run it with
/// `cargo test -p app --test it document_write::document_size_50k -- --ignored --nocapture`; it writes
/// `document-50k-<seed>.json` and `document-50k-<seed>-nosource.json` under the target directory.
/// The seeds are the demo's (the XER generator at the stress seed) and 1 (the hosted `large-50k`
/// sample).
#[test]
#[ignore = "writes a 50,000-activity document; run by hand"]
fn document_size_50k() {
    let dir = std::path::PathBuf::from(
        std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".into()),
    );
    for seed in [app::SEED, 1] {
        let xer = synth_xer(&Spec::with_activities(seed, 50_000));
        let session = open(&xer);
        let source = Source {
            format: SourceFormat::Xer,
            bytes: &xer,
        };
        let whole = write(&session, &extras(&[], Some(source)));
        let snapshot = write(&session, &extras(&[], None));
        let _ = std::fs::write(dir.join(format!("document-50k-{seed}.json")), &whole);
        let _ = std::fs::write(
            dir.join(format!("document-50k-{seed}-nosource.json")),
            &snapshot,
        );
        let source_part = whole.len() - snapshot.len();
        let zipped = |text: &str| gzip_len(text.as_bytes()).map_or("n/a".into(), |n| n.to_string());
        println!(
            "seed {seed}: tasks {}",
            session.schedule().live_task_count()
        );
        println!(
            "  whole     {:>12} raw {:>12} gzip -6",
            whole.len(),
            zipped(&whole)
        );
        println!(
            "  snapshot  {:>12} raw {:>12} gzip -6",
            snapshot.len(),
            zipped(&snapshot)
        );
        println!(
            "  source    {:>12} raw (xer {} bytes as base64)",
            source_part,
            xer.len()
        );
    }
}
