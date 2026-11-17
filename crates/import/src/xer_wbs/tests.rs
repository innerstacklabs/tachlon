use super::{PlanKind, plan};
use crate::ImportError;
use crate::notes::Notes;

#[test]
fn preorder_sorts_activities_and_child_wbs_and_sets_levels() {
    let data = read(&file(
        &[
            "10\t1\t\t9\tY\tROOT\tRoot",
            "20\t1\t10\t2\tN\tB\tSecond",
            "30\t1\t10\t1\tN\tA\tFirst",
            "40\t1\t30\t1\tN\tAA\tGrandchild",
            "50\t1\t10\t3\tN\tEMPTY\tEmpty",
        ],
        &[
            "100\t1\t10\tZ\tRoot activity",
            "101\t1\t30\tB\tLater",
            "102\t1\t30\tA\tEarlier",
            "103\t1\t40\tG\tGrandchild activity",
            "104\t1\t20\tC\tSecond child activity",
        ],
    ));
    let mut notes = Notes::new();
    let entries = plan(&data, &data.project_rows[0], &mut notes).expect("plan");
    let labels: Vec<String> = entries
        .iter()
        .map(|entry| match &entry.kind {
            PlanKind::Summary { name, .. } => name.clone(),
            PlanKind::Activity(row) => row.task_code.clone().unwrap_or_default(),
        })
        .collect();
    assert_eq!(
        labels,
        [
            "SHORT",
            "Z",
            "First",
            "A",
            "B",
            "Grandchild",
            "G",
            "Second",
            "C"
        ]
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.outline_level)
            .collect::<Vec<_>>(),
        [1, 2, 2, 3, 3, 3, 4, 2, 3]
    );
    assert_eq!(entries[0].parent, None);
    assert_eq!(entries[2].parent, Some(0));
    assert_eq!(entries[3].parent, Some(2));
    assert!(
        notes
            .finish()
            .iter()
            .any(|line| line.contains("WBS elements with no activity"))
    );
}

#[test]
fn unknown_parents_cycles_and_orphan_activities_are_kept_at_root() {
    let data = read(&file(
        &[
            "10\t1\t\t1\tY\tROOT\tRoot",
            "20\t1\t999\t1\tN\tLOST\tUnknown",
            "30\t1\t40\t2\tN\tC1\tCycle one",
            "40\t1\t30\t3\tN\tC2\tCycle two",
        ],
        &[
            "100\t1\t20\tU\tUnknown parent activity",
            "101\t1\t30\tC\tCycle activity",
            "102\t1\t999\tO\tOrphan activity",
        ],
    ));
    let mut notes = Notes::new();
    let entries = plan(&data, &data.project_rows[0], &mut notes).expect("plan");
    let activity_ids: Vec<u32> = entries
        .iter()
        .filter_map(|entry| match entry.kind {
            PlanKind::Activity(row) => Some(row.task_id),
            PlanKind::Summary { .. } => None,
        })
        .collect();
    assert_eq!(activity_ids.len(), 3);
    assert!(activity_ids.contains(&100));
    assert!(activity_ids.contains(&101));
    assert!(activity_ids.contains(&102));
    let notes = notes.finish();
    assert!(notes.iter().any(|line| line.contains("parent is missing")));
    assert!(
        notes
            .iter()
            .any(|line| line.contains("WBS element is not in the file"))
    );
}

#[test]
fn missing_root_is_synthetic_and_empty_project_has_no_plan() {
    let data = read(&file(
        &["20\t1\t\t1\tN\tA\tChild"],
        &["100\t1\t20\tA\tActivity"],
    ));
    let mut notes = Notes::new();
    let entries = plan(&data, &data.project_rows[0], &mut notes).expect("plan");
    match &entries[0].kind {
        PlanKind::Summary { uid, name } => {
            assert_eq!(*uid, 1);
            assert_eq!(name, "SHORT");
        }
        PlanKind::Activity(_) => panic!("root should be a summary"),
    }
    assert!(
        notes
            .finish()
            .iter()
            .any(|line| line.contains("no top WBS element"))
    );

    let empty = read(&file(&["10\t1\t\t1\tY\tROOT\tRoot"], &[]));
    assert!(
        plan(&empty, &empty.project_rows[0], &mut Notes::new())
            .expect("empty plan")
            .is_empty()
    );
}

#[test]
fn duplicate_wbs_id_is_rejected() {
    let data = read(&file(
        &["10\t1\t\t1\tY\tROOT\tRoot", "10\t1\t\t2\tN\tDUP\tDuplicate"],
        &["100\t1\t10\tA\tActivity"],
    ));
    let error = plan(&data, &data.project_rows[0], &mut Notes::new())
        .err()
        .expect("duplicate should fail");
    assert_eq!(
        error,
        ImportError::DuplicateId {
            table: "PROJWBS".to_owned(),
            id: 10,
        }
    );
}

fn read(text: &str) -> xer::Data {
    let parsed = xer::parse(text.as_bytes()).expect("parse test XER");
    xer::read(&parsed).expect("read test XER")
}

fn file(wbs: &[&str], tasks: &[&str]) -> String {
    let mut text = String::from(
        "ERMHDR\t19.12\t2024-01-01 00:00\n\
         %T\tPROJECT\n\
         %F\tproj_id\tproj_short_name\texport_flag\tplan_start_date\n\
         %R\t1\tSHORT\tY\t2024-01-03 08:00\n\
         %T\tPROJWBS\n\
         %F\twbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_short_name\twbs_name\n",
    );
    for row in wbs {
        text.push_str("%R\t");
        text.push_str(row);
        text.push('\n');
    }
    text.push_str("%T\tTASK\n%F\ttask_id\tproj_id\twbs_id\ttask_code\ttask_name\n");
    for row in tasks {
        text.push_str("%R\t");
        text.push_str(row);
        text.push('\n');
    }
    text.push_str("%E\n");
    text
}
