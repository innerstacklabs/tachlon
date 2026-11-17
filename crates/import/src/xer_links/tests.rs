use engine::{LinkType, TaskId};

use crate::xer_import::ProjectSelection;
use crate::{ImportError, Imported, parse_xer};

mod lag_calendar;

#[test]
fn maps_all_types_and_lags_in_file_order() {
    let imported = import(
        &["1\tONE\tY\t2024-01-01 08:00\t"],
        &["10\t1\t\tY\tRoot"],
        &[
            task(1, 1, 10, "", "A1"),
            task(2, 1, 10, "", "A2"),
            task(3, 1, 10, "", "A3"),
            task(4, 1, 10, "", "A4"),
            task(5, 1, 10, "", "A5"),
        ],
        &[
            "2\t1\tPR_FS\t2.5",
            "3\t1\tPR_SS\t-8",
            "4\t1\tPR_FF\t",
            "5\t1\tPR_SF\t0",
        ],
        &[],
        &[],
        &ProjectSelection::Unspecified,
    )
    .expect("relationships import");
    let actual: Vec<_> = imported
        .schedule
        .dependencies()
        .iter()
        .map(|dependency| {
            (
                dependency.pred,
                dependency.succ,
                dependency.kind,
                dependency.lag,
            )
        })
        .collect();
    assert_eq!(
        actual,
        [
            (TaskId(1), TaskId(2), LinkType::FS, 150),
            (TaskId(1), TaskId(3), LinkType::SS, -480),
            (TaskId(1), TaskId(4), LinkType::FF, 0),
            (TaskId(1), TaskId(5), LinkType::SF, 0),
        ]
    );
}

#[test]
fn drops_and_counts_invalid_or_external_relationships() {
    let imported = import(
        &[
            "1\tONE\tY\t2024-01-01 08:00\t",
            "2\tTWO\tN\t2024-01-01 08:00\t",
        ],
        &["10\t1\t\tY\tOne", "20\t2\t\tY\tTwo"],
        &[
            task(1, 1, 10, "", "A1"),
            task(2, 1, 10, "", "A2"),
            task(3, 2, 20, "", "B1"),
        ],
        &[
            "1\t1\tPR_FS\t0",
            "2\t99\tPR_FS\t0",
            "2\t3\tPR_FS\t0",
            "2\t1\tPR_XX\t0",
            "2\t1\t\t0",
        ],
        &[],
        &[],
        &ProjectSelection::Only(vec![1]),
    )
    .expect("invalid links are nonfatal");
    assert_eq!(imported.schedule.dependencies(), []);
    assert!(has_note(
        &imported,
        "Links from an activity to itself were not read; no dates are affected: 1 (first: A1)"
    ));
    assert!(has_note(
        &imported,
        "Links to an activity that is not in the file were not read; dates can differ from the file's: 1 (first: A2)"
    ));
    assert!(has_note(
        &imported,
        "Links to activities in another project were not read; dates can differ from the file's: 1"
    ));
    assert!(has_note(
        &imported,
        "Links of type PR_XX, which this import does not know, were not read; dates can differ from the file's: 1 (first: A2)"
    ));
    assert!(has_note(
        &imported,
        "Links with no type were not read; dates can differ from the file's: 1 (first: A2)"
    ));
}

#[test]
fn duplicate_links_are_kept_and_linked_tasks_receive_project_start() {
    let imported = import(
        &["1\tONE\tY\t2024-01-03 08:00\t"],
        &["10\t1\t\tY\tRoot"],
        &[
            task(1, 1, 10, "", "A1"),
            task(2, 1, 10, "", "A2"),
            task(3, 1, 10, "", "A3"),
        ],
        &["2\t1\tPR_FS\t0", "2\t1\tPR_FS\t0"],
        &[],
        &[],
        &ProjectSelection::Unspecified,
    )
    .expect("duplicates import");
    assert_eq!(imported.schedule.dependencies().len(), 2);
    assert_eq!(imported.schedule.tasks()[1].snet, Some(3_360));
    assert_eq!(imported.schedule.tasks()[2].snet, Some(3_360));
    assert_eq!(imported.schedule.tasks()[3].snet, Some(3_360));
    assert!(has_note(
        &imported,
        "The same link appears more than once; every copy was read and no dates are affected: 1 (first: A2)"
    ));
}

#[test]
fn floor_ff_link_successor_at_project_start() {
    let imported = floor_link_import("PR_FF", "0");
    assert_eq!(activity_snet(&imported, 2), Some(3_360));
}

#[test]
fn floor_ss_link_successor_at_project_start() {
    let imported = floor_link_import("PR_SS", "0");
    assert_eq!(activity_snet(&imported, 2), Some(3_360));
}

#[test]
fn floor_sf_link_successor_at_project_start() {
    let imported = floor_link_import("PR_SF", "0");
    assert_eq!(activity_snet(&imported, 2), Some(3_360));
}

#[test]
fn floor_lead_link_successor_at_project_start() {
    let mut imported = floor_link_import("PR_FS", "-8");
    let successor = activity_task_id(&imported, 2);
    assert_eq!(
        imported.schedule.tasks()[successor.0 as usize].snet,
        Some(3_360)
    );
    let solution = imported
        .schedule
        .compute()
        .expect("floor schedule computes");
    assert_eq!(solution.get(successor).es, 3_360);
}

#[test]
fn floor_cross_project_link_at_successor_project_start() {
    let imported = import(
        &[
            "1\tONE\tY\t2024-01-01 08:00\t",
            "2\tTWO\tY\t2024-01-03 08:00\t",
        ],
        &["10\t1\t\tY\tOne", "20\t2\t\tY\tTwo"],
        &[task(1, 1, 10, "", "A1"), task(2, 2, 20, "", "B1")],
        &["2\t1\tPR_FS\t0"],
        &[],
        &[],
        &ProjectSelection::All,
    )
    .expect("cross-project link imports");
    assert_eq!(activity_snet(&imported, 2), Some(3_360));
}

#[test]
fn a_selected_project_without_plan_start_date_is_noted_and_not_floored() {
    let imported = import(
        &["1\tONE\tY\t2024-01-03 08:00\t", "2\tTWO\tY\t\t"],
        &["10\t1\t\tY\tOne", "20\t2\t\tY\tTwo"],
        &[
            task(1, 1, 10, "", "A1"),
            task(2, 2, 20, "", "B1"),
            task_with_msoa_in(3, 2, 20, "B2", "2024-01-04 08:00"),
        ],
        &[],
        &[],
        &[],
        &ProjectSelection::All,
    )
    .expect("a project without plan_start_date imports");
    assert_eq!(activity_snet(&imported, 1), Some(3_360));
    assert_eq!(activity_snet(&imported, 2), None);
    // A constraint of its own still applies; there is only no start to floor it at.
    assert_eq!(activity_snet(&imported, 3), Some(4_800));
    assert!(has_note(
        &imported,
        "The project has no start date, so its activities are not held back to a project start and their dates can differ from the file's: 1 (first: 2)"
    ));
}

#[test]
fn floor_early_msoa_at_project_start_and_keep_later_msoa() {
    let imported = import(
        &["1\tONE\tY\t2024-01-03 08:00\t"],
        &["10\t1\t\tY\tRoot"],
        &[
            task_with_msoa(1, "EARLY", "2024-01-02 08:00"),
            task_with_msoa(2, "LATE", "2024-01-04 08:00"),
        ],
        &[],
        &[],
        &[],
        &ProjectSelection::Unspecified,
    )
    .expect("MSOA activities import");
    assert_eq!(activity_snet(&imported, 1), Some(3_360));
    assert_eq!(activity_snet(&imported, 2), Some(4_800));
}

#[test]
fn rounded_and_non_finite_lags_are_handled_explicitly() {
    let rounded = import(
        &["1\tONE\tY\t2024-01-01 08:00\t"],
        &["10\t1\t\tY\tRoot"],
        &[task(1, 1, 10, "", "A1"), task(2, 1, 10, "", "A2")],
        &["2\t1\tPR_FS\t0.01"],
        &[],
        &[],
        &ProjectSelection::Unspecified,
    )
    .expect("fractional-minute lag imports");
    assert_eq!(rounded.schedule.dependencies()[0].lag, 1);
    assert!(has_note(
        &rounded,
        "A link's lag was rounded to the whole minute; dates can differ by under a minute: 1 (first: A2)"
    ));

    let error = import(
        &["1\tONE\tY\t2024-01-01 08:00\t"],
        &["10\t1\t\tY\tRoot"],
        &[task(1, 1, 10, "", "A1"), task(2, 1, 10, "", "A2")],
        &["2\t1\tPR_FS\tinf"],
        &[],
        &[],
        &ProjectSelection::Unspecified,
    )
    .expect_err("non-finite lag is invalid");
    assert!(matches!(error, ImportError::Invalid { .. }));
}

fn import(
    projects: &[&str],
    wbs: &[&str],
    tasks: &[String],
    predecessors: &[&str],
    options: &[&str],
    calendars: &[&str],
    selection: &ProjectSelection,
) -> Result<Imported, ImportError> {
    let mut text = String::from("ERMHDR\t19.12\t2024-01-01 00:00\n");
    table(
        &mut text,
        "PROJECT",
        "proj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id",
        projects,
    );
    table(
        &mut text,
        "PROJWBS",
        "wbs_id\tproj_id\tparent_wbs_id\tproj_node_flag\twbs_name",
        wbs,
    );
    let task_rows: Vec<_> = tasks.iter().map(String::as_str).collect();
    table(
        &mut text,
        "TASK",
        "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date",
        &task_rows,
    );
    table(
        &mut text,
        "TASKPRED",
        "task_id\tpred_task_id\tpred_type\tlag_hr_cnt",
        predecessors,
    );
    if !options.is_empty() {
        table(
            &mut text,
            "SCHEDOPTIONS",
            "proj_id\tsched_calendar_on_relationship_lag",
            options,
        );
    }
    if !calendars.is_empty() {
        table(
            &mut text,
            "CALENDAR",
            "clndr_id\tclndr_type\tclndr_data",
            calendars,
        );
    }
    text.push_str("%E\n");
    parse_xer(text.as_bytes(), selection)
}

fn table(text: &mut String, name: &str, fields: &str, rows: &[&str]) {
    text.push_str("%T\t");
    text.push_str(name);
    text.push('\n');
    text.push_str("%F\t");
    text.push_str(fields);
    text.push('\n');
    for row in rows {
        text.push_str("%R\t");
        text.push_str(row);
        text.push('\n');
    }
}

fn task(id: u32, project: u32, wbs: u32, calendar: &str, code: &str) -> String {
    format!("{id}\t{project}\t{wbs}\t{calendar}\t{code}\t{code}\tTT_Task\tTK_NotStart\t1\t1\t\t")
}

fn task_with_msoa(id: u32, code: &str, date: &str) -> String {
    format!("{id}\t1\t10\t\t{code}\t{code}\tTT_Task\tTK_NotStart\t1\t1\tCS_MSOA\t{date}")
}

fn task_with_msoa_in(id: u32, project: u32, wbs: u32, code: &str, date: &str) -> String {
    format!("{id}\t{project}\t{wbs}\t\t{code}\t{code}\tTT_Task\tTK_NotStart\t1\t1\tCS_MSOA\t{date}")
}

fn floor_link_import(kind: &str, lag: &str) -> Imported {
    let link = format!("2\t1\t{kind}\t{lag}");
    import(
        &["1\tONE\tY\t2024-01-03 08:00\t"],
        &["10\t1\t\tY\tRoot"],
        &[task(1, 1, 10, "", "A1"), task(2, 1, 10, "", "A2")],
        &[&link],
        &[],
        &[],
        &ProjectSelection::Unspecified,
    )
    .expect("linked activities import")
}

fn activity_task_id(imported: &Imported, uid: u32) -> TaskId {
    let index = imported
        .tasks
        .iter()
        .position(|task| task.uid == uid && task.code.is_some())
        .expect("activity should be imported");
    TaskId(u32::try_from(index).expect("task index fits in u32"))
}

fn activity_snet(imported: &Imported, uid: u32) -> Option<i64> {
    imported.schedule.tasks()[activity_task_id(imported, uid).0 as usize].snet
}

fn has_note(imported: &Imported, expected: &str) -> bool {
    imported.unsupported.iter().any(|line| line == expected)
}
