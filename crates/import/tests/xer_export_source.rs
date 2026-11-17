//! gs#703: the XER writer given the file the schedule was opened from. The written file keeps the
//! source's ids, `CALENDAR` rows, header, encoding and line ending, and drops (and lists) the
//! source rows the schedule no longer has.

mod common;

use common::{WEEK, regular_task, table};
use engine::Task;
use import::{Imported, ProjectSelection, XerLayout, XerOptions, parse_xer};

const EXPORTED_AT: i64 = 19_787 * 1_440;

/// What a source file holds: (id, short name) projects, (id, project, parent, name) WBS nodes,
/// (id, code, project, WBS node) tasks and (id, task, predecessor) links.
struct Spec<'a> {
    projects: &'a [(u32, &'a str)],
    wbs: &'a [(u32, u32, Option<u32>, &'a str)],
    tasks: &'a [(u32, &'a str, u32, u32)],
    links: &'a [(u32, u32, u32)],
}

/// The ids are none of the 1, 2, 3 the writer would number: calendars 7 and 9, a header of version
/// 8.4 and, by default, one project 3 with one WBS node 40.
fn build(spec: &Spec) -> String {
    let mut text =
        String::from("ERMHDR\t8.4\t2023-05-01\tProject\tpm\tpm\tprod\tProject Management\tEUR\n");
    let rows: Vec<String> = spec
        .projects
        .iter()
        .map(|(id, name)| format!("{id}\t{name}\tY\t2024-03-04 08:00\t7"))
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let fields = "proj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id";
    table(&mut text, "PROJECT", fields, &rows);
    let rows: Vec<String> = spec
        .wbs
        .iter()
        .map(|(id, project, parent, name)| {
            let parent = parent.map_or_default(|p| p.to_string());
            let flag = if parent.is_empty() { "Y" } else { "N" };
            format!("{id}\t{project}\t{parent}\t1\t{flag}\t{name}")
        })
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let fields = "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name";
    table(&mut text, "PROJWBS", fields, &rows);
    let rows: Vec<String> = spec
        .tasks
        .iter()
        .map(|&(id, code, project, node)| {
            regular_task(id, code, "8").replacen(
                "\t1\t10\t1\t",
                &format!("\t{project}\t{node}\t7\t"),
                1,
            )
        })
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let fields = "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2";
    table(&mut text, "TASK", fields, &rows);
    let rows: Vec<String> = spec
        .links
        .iter()
        .map(|(id, task, pred)| format!("{id}\t{task}\t{pred}\tPR_FS\t0"))
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let fields = "task_pred_id\ttask_id\tpred_task_id\tpred_type\tlag_hr_cnt";
    table(&mut text, "TASKPRED", fields, &rows);
    let rows: Vec<String> = spec
        .projects
        .iter()
        .map(|(id, _)| format!("{id}\trcal_Successor"))
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let fields = "proj_id\tsched_calendar_on_relationship_lag";
    table(&mut text, "SCHEDOPTIONS", fields, &rows);
    let calendars = [
        format!("7\tCA_Project\tCalendar Seven\t{WEEK}"),
        format!("9\tCA_Base\tCalendar Nine\t{WEEK}"),
    ];
    let calendars: Vec<&str> = calendars.iter().map(String::as_str).collect();
    let fields = "clndr_id\tclndr_type\tclndr_name\tclndr_data";
    table(&mut text, "CALENDAR", fields, &calendars);
    text.push_str("%E\n");
    text
}

const ONE: &[(u32, &str)] = &[(3, "HAND")];
const ROOT: &[(u32, u32, Option<u32>, &str)] = &[(40, 3, None, "Root")];
const THREE: &[(u32, &str, u32, u32)] =
    &[(101, "A1", 3, 40), (102, "A2", 3, 40), (105, "A3", 3, 40)];
const LINK: &[(u32, u32, u32)] = &[(900, 102, 101)];

fn three() -> Vec<u8> {
    let spec = Spec {
        projects: ONE,
        wbs: ROOT,
        tasks: THREE,
        links: LINK,
    };
    build(&spec).into_bytes()
}

fn options() -> XerOptions {
    XerOptions {
        exported_at: EXPORTED_AT,
        name: "HAND".to_owned(),
        layout: XerLayout::FromXer,
    }
}

fn open(bytes: &[u8]) -> Imported {
    parse_xer(bytes, &ProjectSelection::All).expect("the source should import")
}

/// The rows of `name` as lists of cells, with its fields.
fn rows_of(bytes: &[u8], name: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let parsed = xer::parse(bytes).expect("the file should parse");
    let found = parsed.table(name).expect("the table should be written");
    let fields = found.fields().map(std::borrow::Cow::into_owned).collect();
    let rows = found
        .rows()
        .iter()
        .map(|row| {
            (0..found.field_count())
                .map(|i| row.cell(i).unwrap_or_default().into_owned())
                .collect()
        })
        .collect();
    (fields, rows)
}

fn column(bytes: &[u8], table: &str, field: &str) -> Vec<String> {
    let (fields, rows) = rows_of(bytes, table);
    let at = fields.iter().position(|f| f == field).expect("the field");
    rows.into_iter().map(|row| row[at].clone()).collect()
}

/// A leaf with no activity ID, after the last task and under the same node.
fn insert(imported: &mut Imported) {
    let mut inserted = imported.tasks.last().expect("a task").clone();
    inserted.code = None;
    "Inserted".clone_into(&mut inserted.name);
    inserted.uid = 99;
    let like: Task = imported.schedule.tasks().last().expect("a task").clone();
    imported.tasks.push(inserted);
    imported.schedule.add_task(like).expect("task should add");
}

#[test]
fn an_unedited_file_keeps_its_ids_and_calendar_rows() {
    let bytes = three();
    let exported = options()
        .export_with_source(&open(&bytes), &bytes)
        .expect("export");
    let out = &exported.bytes;

    assert_eq!(column(out, "TASK", "task_id"), ["101", "102", "105"]);
    assert_eq!(column(out, "TASK", "proj_id"), ["3", "3", "3"]);
    assert_eq!(column(out, "TASK", "wbs_id"), ["40", "40", "40"]);
    assert_eq!(column(out, "TASK", "clndr_id"), ["7", "7", "7"]);
    // The project is the source's by its root WBS node: the writer names it "Root", not "HAND".
    assert_eq!(column(out, "PROJECT", "proj_id"), ["3"]);
    assert_eq!(column(out, "PROJWBS", "wbs_id"), ["40"]);
    assert_eq!(column(out, "SCHEDOPTIONS", "proj_id"), ["3"]);
    assert_eq!(column(out, "TASKPRED", "task_pred_id"), ["900"]);
    assert_eq!(column(out, "TASKPRED", "task_id"), ["102"]);
    assert_eq!(column(out, "TASKPRED", "pred_task_id"), ["101"]);
    assert_eq!(rows_of(out, "CALENDAR"), rows_of(&bytes, "CALENDAR"));
    assert!(exported.left_out.is_empty(), "{:?}", exported.left_out);

    // The header is the source's, with the export time in place of its date.
    let text = String::from_utf8(out.clone()).expect("UTF-8");
    let first = text.lines().next().unwrap_or_default();
    assert!(first.starts_with("ERMHDR\t8.4\t2024-03-"), "{first}");
    assert!(
        first.ends_with("\tProject\tpm\tpm\tprod\tProject Management\tEUR"),
        "{first}"
    );
}

#[test]
fn an_inserted_row_takes_an_id_above_the_largest() {
    // The source also holds a row, 109, that the schedule no longer has.
    let mut tasks = THREE.to_vec();
    tasks.push((109, "A9", 3, 40));
    let spec = Spec {
        projects: ONE,
        wbs: ROOT,
        tasks: &tasks,
        links: LINK,
    };
    let mut imported = open(&three());
    insert(&mut imported);

    let exported = options()
        .export_with_source(&imported, build(&spec).as_bytes())
        .expect("export");
    let out = &exported.bytes;

    assert_eq!(column(out, "TASK", "task_id"), ["101", "102", "105", "110"]);
    assert_eq!(column(out, "TASK", "clndr_id"), ["7", "7", "7", "7"]);
    assert!(
        exported.left_out.iter().any(|line| line.contains("A9")),
        "{:?}",
        exported.left_out
    );
}

#[test]
fn an_inserted_row_does_not_take_a_dropped_rows_activity_id_or_id() {
    // The source's A4 (109) is gone and a new row was inserted: the writer's own next ID would be
    // A4, which must not give the new row A4's id and hide the drop.
    let mut tasks = THREE.to_vec();
    tasks.push((109, "A4", 3, 40));
    let spec = Spec {
        projects: ONE,
        wbs: ROOT,
        tasks: &tasks,
        links: LINK,
    };
    let mut imported = open(&three());
    insert(&mut imported);

    let exported = options()
        .export_with_source(&imported, build(&spec).as_bytes())
        .expect("export");
    let out = &exported.bytes;

    assert_eq!(column(out, "TASK", "task_id"), ["101", "102", "105", "110"]);
    assert_eq!(column(out, "TASK", "task_code"), ["A1", "A2", "A3", "A5"]);
    assert!(
        exported.left_out.iter().any(|line| line.contains("A4")),
        "{:?}",
        exported.left_out
    );
}

#[test]
fn a_shared_activity_id_matches_inside_its_own_project() {
    let projects: &[(u32, &str)] = &[(3, "One"), (4, "Two")];
    let wbs: &[(u32, u32, Option<u32>, &str)] = &[(40, 3, None, "One"), (50, 4, None, "Two")];
    let tasks: &[(u32, &str, u32, u32)] =
        &[(201, "A1", 4, 50), (202, "A2", 4, 50), (101, "A1", 3, 40)];
    let bytes = build(&Spec {
        projects,
        wbs,
        tasks,
        links: &[],
    })
    .into_bytes();
    let imported = open(&bytes);

    let out = options()
        .export_with_source(&imported, &bytes)
        .expect("export")
        .bytes;

    assert_eq!(column(&out, "TASK", "task_id"), ["101", "201", "202"]);
    assert_eq!(column(&out, "TASK", "proj_id"), ["3", "4", "4"]);
    assert_eq!(column(&out, "PROJECT", "proj_id"), ["3", "4"]);
}

#[test]
fn a_nested_node_matches_by_parent_and_name_and_unmatched_rows_are_listed() {
    let wbs: &[(u32, u32, Option<u32>, &str)] =
        &[(40, 3, None, "Root"), (41, 3, Some(40), "Phase")];
    let tasks: &[(u32, &str, u32, u32)] = &[(101, "A1", 3, 41), (102, "A2", 3, 41)];
    let imported = open(
        &build(&Spec {
            projects: ONE,
            wbs,
            tasks,
            links: &[],
        })
        .into_bytes(),
    );
    // The source also holds a node (42) and a whole project (8) the schedule does not have.
    let projects: &[(u32, &str)] = &[(3, "HAND"), (8, "OTHER")];
    let wbs: &[(u32, u32, Option<u32>, &str)] = &[
        (40, 3, None, "Root"),
        (41, 3, Some(40), "Phase"),
        (42, 3, Some(40), "Gone"),
        (50, 8, None, "Other"),
    ];
    let source = build(&Spec {
        projects,
        wbs,
        tasks,
        links: &[],
    });

    let exported = options()
        .export_with_source(&imported, source.as_bytes())
        .expect("export");
    let out = &exported.bytes;

    assert_eq!(column(out, "PROJWBS", "wbs_id"), ["40", "41"]);
    assert_eq!(column(out, "PROJWBS", "parent_wbs_id"), ["", "40"]);
    assert_eq!(column(out, "TASK", "wbs_id"), ["41", "41"]);
    let listed = |word: &str| exported.left_out.iter().any(|line| line.contains(word));
    assert!(
        listed("OTHER") && listed("Gone") && listed("Other"),
        "{:?}",
        exported.left_out
    );
    assert!(
        !listed("Phase") && !listed("HAND"),
        "{:?}",
        exported.left_out
    );
}

#[test]
fn the_sources_line_ending_and_byte_order_mark_are_kept() {
    let source = format!(
        "\u{feff}{}",
        String::from_utf8(three())
            .expect("UTF-8")
            .replace('\n', "\r\n")
    );
    let imported = open(source.as_bytes());

    let out = options()
        .export_with_source(&imported, source.as_bytes())
        .expect("export")
        .bytes;

    assert!(out.starts_with(&[0xef, 0xbb, 0xbf, b'E']));
    let crlf = out.windows(2).filter(|pair| pair == b"\r\n").count();
    assert_eq!(crlf, String::from_utf8_lossy(&out).matches('\n').count());
    assert!(crlf > 10);
}

#[test]
fn a_windows_1252_source_is_written_in_windows_1252() {
    // "Name A3" becomes "Name A3 café" with é as the single byte 0xE9, which is not UTF-8.
    let mut source = three();
    let at = source
        .windows(7)
        .position(|w| w == b"Name A3")
        .expect("the name");
    source.splice(at + 7..at + 7, b" caf\xe9".iter().copied());
    let imported = open(&source);
    assert!(
        imported
            .tasks
            .iter()
            .any(|task| task.name == "Name A3 café")
    );

    let out = options()
        .export_with_source(&imported, &source)
        .expect("export")
        .bytes;

    assert!(String::from_utf8(out.clone()).is_err());
    assert!(
        out.windows(5).any(|w| w == b"caf\xe9\t"),
        "the name is written as 1252"
    );
}

#[test]
fn a_source_that_is_not_an_xer_file_is_an_error() {
    let imported = open(&three());
    assert!(
        options()
            .export_with_source(&imported, b"not a file")
            .is_err()
    );
}
