use super::{ProjectSelection, origin_day};
use crate::notes::Notes;
use crate::units::days_from_civil;
use crate::{ImportError, parse_xer};
use xer::ProjectRow;
use xer::synth::{Spec, write};

#[test]
fn one_project_is_selected_implicitly() {
    let imported = parse_xer(
        &write(&Spec::with_activities(1, 2)),
        &ProjectSelection::Unspecified,
    );
    assert!(imported.is_ok());
}

#[test]
fn several_projects_require_a_choice() {
    let mut spec = Spec::with_activities(1, 3);
    spec.projects = 2;
    let error = parse_xer(&write(&spec), &ProjectSelection::Unspecified)
        .expect_err("several projects must require a choice");
    let ImportError::ProjectChoice { projects } = error else {
        panic!("expected project choice");
    };
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].proj_id, 1_000);
    assert_eq!(projects[0].activities, 3);
    assert!(projects[0].short_name.is_some());
}

#[test]
fn all_selects_every_exported_project() {
    let mut spec = Spec::with_activities(2, 1);
    spec.projects = 2;
    let imported = parse_xer(&write(&spec), &ProjectSelection::All)
        .expect("all exported projects should import");
    let codes: Vec<_> = imported
        .tasks
        .iter()
        .filter_map(|task| task.code.as_deref())
        .collect();
    assert!(codes.contains(&"A1-1"));
    assert!(codes.contains(&"A2-1"));
}

#[test]
fn only_rejects_an_unknown_id() {
    let error = parse_xer(
        &write(&Spec::with_activities(3, 1)),
        &ProjectSelection::Only(vec![99]),
    )
    .expect_err("unknown project should fail");
    assert_eq!(error, ImportError::UnknownProject { proj_id: 99 });
}

#[test]
fn duplicate_task_id_is_rejected_across_the_whole_table() {
    let text = "ERMHDR\t19.12\t2024-01-01 00:00\n\
        %T\tPROJECT\n\
        %F\tproj_id\tproj_short_name\texport_flag\n\
        %R\t1\tONE\tY\n\
        %T\tTASK\n\
        %F\ttask_id\tproj_id\twbs_id\n\
        %R\t7\t1\t10\n\
        %R\t7\t2\t20\n\
        %E\n";
    let error = parse_xer(text.as_bytes(), &ProjectSelection::All)
        .expect_err("duplicate TASK id should fail");
    assert_eq!(
        error,
        ImportError::DuplicateId {
            table: "TASK".to_owned(),
            id: 7,
        }
    );
}

#[test]
fn duplicate_project_id_is_rejected_for_every_selection_mode() {
    let text = "ERMHDR\t19.12\t2024-01-01 00:00\n\
        %T\tPROJECT\n\
        %F\tproj_id\tproj_short_name\texport_flag\n\
        %R\t1\tONE\tY\n\
        %R\t1\tONE\tY\n\
        %E\n";
    for selection in [ProjectSelection::All, ProjectSelection::Unspecified] {
        let error =
            parse_xer(text.as_bytes(), &selection).expect_err("duplicate PROJECT id should fail");
        assert_eq!(
            error,
            ImportError::DuplicateId {
                table: "PROJECT".to_owned(),
                id: 1,
            }
        );
    }
}

#[test]
fn activity_in_a_project_not_in_the_file_is_counted_and_dropped() {
    let text = "ERMHDR\t19.12\t2024-01-01 00:00\n\
        %T\tPROJECT\n\
        %F\tproj_id\tproj_short_name\texport_flag\tplan_start_date\n\
        %R\t1\tONE\tY\t2024-01-01 08:00\n\
        %T\tPROJWBS\n\
        %F\twbs_id\tproj_id\tproj_node_flag\twbs_name\n\
        %R\t10\t1\tY\tRoot\n\
        %T\tTASK\n\
        %F\ttask_id\tproj_id\twbs_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\n\
        %R\t1\t1\t10\tKEPT\tKept\tTT_Task\tTK_NotStart\t1\t1\n\
        %R\t2\t99\t20\tDROPPED\tDropped\tTT_Task\tTK_NotStart\t1\t1\n\
        %E\n";
    let imported = parse_xer(text.as_bytes(), &ProjectSelection::Unspecified)
        .expect("known-project activity should still import");
    assert!(
        imported
            .tasks
            .iter()
            .any(|task| task.code.as_deref() == Some("KEPT"))
    );
    assert!(
        !imported
            .tasks
            .iter()
            .any(|task| task.code.as_deref() == Some("DROPPED"))
    );
    assert!(imported.unsupported.iter().any(
        |line| line == "Activities in a project that is not in the file were not read, and links to them were dropped, so dates can differ from the file's: 1 (first: DROPPED)"
    ));
}

#[test]
fn a_non_exported_project_is_not_a_candidate() {
    let bytes = file(&["1\tONE\tY\t2024-01-03 08:00", "2\tTWO\tN\t2024-01-04 08:00"]);
    assert!(parse_xer(bytes.as_bytes(), &ProjectSelection::Unspecified).is_ok());
}

#[test]
fn origin_uses_monday_before_a_wednesday() {
    let project = project_with_start(Some(days_from_civil(2024, 1, 3) * 1_440 + 8 * 60));
    assert_eq!(
        origin_day(&[&project], &mut Notes::new()).expect("origin"),
        days_from_civil(2024, 1, 1)
    );
}

#[test]
fn missing_start_uses_2000_origin_and_is_noted() {
    let project = project_with_start(None);
    let mut notes = Notes::new();
    assert_eq!(
        origin_day(&[&project], &mut notes).expect("origin"),
        days_from_civil(2000, 1, 3)
    );
    assert!(
        notes
            .finish()
            .iter()
            .any(|line| line.contains("2000-01-03"))
    );
}

#[test]
fn earliest_selected_project_controls_origin() {
    let later = project_with_start(Some(days_from_civil(2024, 2, 7) * 1_440));
    let earlier = project_with_start(Some(days_from_civil(2024, 1, 3) * 1_440));
    assert_eq!(
        origin_day(&[&later, &earlier], &mut Notes::new()).expect("origin"),
        days_from_civil(2024, 1, 1)
    );
}

fn project_with_start(plan_start_date: Option<i64>) -> ProjectRow {
    ProjectRow {
        proj_id: 1,
        proj_short_name: Some("ONE".to_owned()),
        clndr_id: None,
        plan_start_date,
        plan_end_date: None,
        last_recalc_date: None,
        export_flag: Some(true),
        complete_pct_type: None,
    }
}

fn file(projects: &[&str]) -> String {
    let mut text = String::from(
        "ERMHDR\t19.12\t2024-01-01 00:00\n%T\tPROJECT\n%F\tproj_id\tproj_short_name\texport_flag\tplan_start_date\n",
    );
    for project in projects {
        text.push_str("%R\t");
        text.push_str(project);
        text.push('\n');
    }
    text.push_str("%E\n");
    text
}
