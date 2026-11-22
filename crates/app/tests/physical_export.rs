//! A task's physical percent is written back to XER and MS Project files (tl#2080): a file that
//! holds 50 with eight of ten days left reopens with 50 and the same remaining work.

use crate::common::xer_text::{loaded, rewrite, text};
use crate::document_round_trip::session_of;
use app::document::Extras;
use app::export::{export_mspdi, export_xer};
use import::{Exported, Imported, ProjectSelection, parse_mspdi, parse_xer};
use xer::synth::Spec;

/// Seven activities in which `Pour` has ten days of work with eight left and `cell` as its
/// physical percent.
fn file(cell: &str) -> String {
    let mut file = text(&Spec::with_activities(3, 7));
    for (column, value) in [
        ("task_name", "Pour"),
        ("target_drtn_hr_cnt", "80"),
        ("remain_drtn_hr_cnt", "64"),
        ("phys_complete_pct", cell),
    ] {
        file = rewrite(&file, "TASK", column, |row, _| {
            (row == 5).then(|| value.to_owned())
        });
    }
    file
}

fn exports(bytes: &[u8], mspdi: bool) -> Exported {
    let session = session_of(loaded(bytes, &ProjectSelection::Unspecified));
    let extras = Extras {
        name: "physical",
        notes: &[],
        source: None,
    };
    let out = if mspdi {
        export_mspdi(&session, &extras)
    } else {
        export_xer(&session, &extras)
    };
    out.expect("exports")
}

fn xml(out: Exported) -> String {
    String::from_utf8(out.bytes).expect("utf-8")
}

/// The physical percent and remaining minutes of `Pour`.
fn pour(imported: &Imported) -> (Option<u16>, Option<i64>) {
    let task = imported.tasks.iter().find(|task| task.name == "Pour");
    let task = task.expect("Pour is a task");
    (task.physical, task.remaining)
}

#[test]
fn an_xer_file_with_50_physical_and_eight_of_ten_days_left_opens_and_saves_back_50() {
    let source = file("50");
    let before = parse_xer(source.as_bytes(), &ProjectSelection::Unspecified).expect("reads");
    assert_eq!(pour(&before), (Some(5000), Some(3840)));
    let out = exports(source.as_bytes(), false);
    let after = parse_xer(&out.bytes, &ProjectSelection::Unspecified).expect("reads back");
    assert_eq!(pour(&after), (Some(5000), Some(3840)));
}

#[test]
fn an_ms_project_file_with_40_physical_opens_and_saves_back_40() {
    let first = xml(exports(file("40").as_bytes(), true));
    assert!(first.contains("<PhysicalPercentComplete>40</PhysicalPercentComplete>"));
    let again = xml(exports(first.as_bytes(), true));
    let after = parse_mspdi(&again).expect("reads back");
    assert_eq!(pour(&after), (Some(4000), Some(3840)));
}

#[test]
fn a_fraction_is_written_in_an_xer_file_and_left_out_of_an_ms_project_file() {
    let xer = exports(file("12.5").as_bytes(), false);
    let read = parse_xer(&xer.bytes, &ProjectSelection::Unspecified).expect("reads back");
    assert_eq!(pour(&read).0, Some(1250));
    let mspdi = exports(file("12.5").as_bytes(), true);
    let line = "has a fraction, so it is written as a whole number";
    assert_eq!(
        mspdi.left_out.iter().filter(|l| l.contains(line)).count(),
        1
    );
    assert_eq!(
        pour(&parse_mspdi(&xml(mspdi)).expect("reads back")).0,
        Some(1300)
    );
}

#[test]
fn a_physical_percent_that_rounds_to_zero_is_left_out_of_an_ms_project_file() {
    let count =
        |out: &Exported, needle: &str| out.left_out.iter().filter(|l| l.contains(needle)).count();
    for cell in ["0.4", "0"] {
        let out = exports(file(cell).as_bytes(), true);
        assert_eq!(count(&out, "reads back as none"), 1, "{cell}");
        assert_eq!(count(&out, "has a fraction"), 0, "{cell}");
        let text = xml(out);
        assert!(text.contains("<PhysicalPercentComplete>0</PhysicalPercentComplete>"));
        assert_eq!(pour(&parse_mspdi(&text).expect("reads back")).0, None);
    }
    let out = exports(file("0.5").as_bytes(), true);
    assert_eq!(count(&out, "has a fraction"), 1);
    assert_eq!(count(&out, "reads back as none"), 0);
    let read = parse_mspdi(&xml(out)).expect("reads back");
    assert_eq!(pour(&read).0, Some(100));
}

#[test]
fn a_task_with_no_physical_percent_writes_what_it_wrote_before() {
    let xer = exports(file("").as_bytes(), false);
    let read = parse_xer(&xer.bytes, &ProjectSelection::Unspecified).expect("reads back");
    assert_eq!(pour(&read).0, Some(2000));
    let mspdi = exports(file("").as_bytes(), true);
    assert!(mspdi.left_out.iter().all(|line| !line.contains("physical")));
    assert!(!xml(mspdi).contains("PhysicalPercentComplete"));
}
