use super::duration::duration;
use crate::notes::Notes;
use crate::xer_import::ProjectSelection;
use crate::{ImportError, parse_xer};
use xer::{StatusCode, TaskRow};

#[test]
fn status_selects_duration_and_special_types_become_zero_duration() {
    let imported = import(&[
        "1\t1\t10\tA1\tNot started\tTT_Task\tTK_NotStart\t2\t1\t\t\t",
        "2\t1\t10\tA2\tActive\tTT_Task\tTK_Active\t2\t1\t\t\t",
        "3\t1\t10\tA3\tComplete\tTT_Task\tTK_Complete\t3\t0\t\t\t",
        "4\t1\t10\tA4\tMilestone\tTT_Mile\tTK_NotStart\t4\t4\t\t\t",
        "5\t1\t10\tA5\tLOE\tTT_LOE\tTK_NotStart\t5\t5\t\t\t",
        "6\t1\t10\tA6\tNinety minutes\tTT_Task\tTK_NotStart\t1.5\t1.5\t\t\t",
        "7\t1\t10\tA7\tRounded\tTT_Task\tTK_NotStart\t0.0083333\t0.0083333\t\t\t",
        "8\t1\t10\tA8\tWbs summary\tTT_WBS\tTK_NotStart\t6\t6\t\t\t",
        "9\t1\t10\tA9\tResource\tTT_Rsrc\tTK_NotStart\t7\t7\t\t\t",
        "10\t1\t10\tA10\tUnknown type\tTT_Foo\tTK_NotStart\t8\t8\t\t\t",
    ])
    .expect("activities import");
    // Siblings come back in code order ("A10" before "A2"), so look durations up by code.
    let duration_of = |code: &str| {
        let index = imported
            .tasks
            .iter()
            .position(|task| task.code.as_deref() == Some(code))
            .expect("activity present");
        imported.schedule.tasks()[index].duration
    };
    let durations: Vec<i64> = ["A1", "A2", "A3", "A4", "A5", "A6", "A7", "A8", "A9", "A10"]
        .into_iter()
        .map(duration_of)
        .collect();
    assert_eq!(durations, [60, 120, 180, 0, 0, 90, 0, 0, 420, 480]);
    assert!(
        imported
            .unsupported
            .iter()
            .any(|line| line.starts_with("Milestones that had a duration"))
    );
    assert!(imported.unsupported.iter().any(|line| {
        line.starts_with("Level of effort activities were imported as zero-day tasks")
    }));
    assert!(
        imported
            .unsupported
            .iter()
            .any(|line| line.contains("durations were rounded to the whole minute"))
    );
    assert!(
        !imported
            .unsupported
            .iter()
            .any(|line| line.contains("actual dates ignored"))
    );
    for expected in [
        "WBS summary activities were imported as zero-day tasks and do not stretch over their span, so their dates can differ from the file's",
        "Resource dependent activities are scheduled on their own calendar; the resource calendar was not read, so their dates can differ from the file's",
        "Activities of type TT_Foo, which this import does not know, are treated as ordinary tasks, so their dates can differ from the file's",
    ] {
        assert!(
            imported
                .unsupported
                .iter()
                .any(|line| line.contains(expected) && line.contains("(first: A")),
            "missing note {expected}: {:?}",
            imported.unsupported
        );
    }
}

#[test]
fn duration_fallback_to_the_other_field_is_counted() {
    let imported = import(&[
        "1\t1\t10\tA1\tNot started\tTT_Task\tTK_NotStart\t2\t\t\t\t",
        "2\t1\t10\tA2\tActive\tTT_Task\tTK_Active\t\t3\t\t\t",
        "3\t1\t10\tA3\tComplete\tTT_Task\tTK_Complete\t\t4\t\t\t",
        "4\t1\t10\tA4\tOther\tTT_Task\tTK_Future\t5\t\t\t\t",
        "5\t1\t10\tA5\tNo status\tTT_Task\t\t6\t\t\t\t",
    ])
    .expect("duration fallbacks import");
    let durations: Vec<_> = imported.schedule.tasks()[1..]
        .iter()
        .map(|task| task.duration)
        .collect();
    assert_eq!(durations, [120, 180, 240, 300, 360]);
    assert!(
        imported.unsupported.iter().any(
            |line| line == "Activities with no duration in the usual field take it from the other one (remaining or original), so their dates can differ from the file's: 5 (first: A1, A2, A3, A4, A5)"
        )
    );
}

#[test]
fn constraints_are_applied_or_counted_exactly() {
    let imported = import(&[
        "1\t1\t10\tA1\tMSOA\tTT_Task\tTK_NotStart\t1\t1\tCS_MSOA\t2024-01-05 08:00\t",
        "2\t1\t10\tA2\tFinish on\tTT_Task\tTK_NotStart\t1\t1\tCS_MEO\t2024-01-05 08:00\tCS_MSOB",
    ])
    .expect("constraints import");
    // 2024-01-05 08:00 is after the project start, so the activity's own CS_MSOA wins.
    assert_eq!(imported.schedule.tasks()[1].snet, Some(6_240));
    assert_eq!(imported.schedule.tasks()[2].snet, Some(3_360));
    // A must-finish-on date is applied now, as a pin; only the secondary one with no date is listed.
    assert_eq!(
        imported.schedule.tasks()[2].limit.pin,
        Some(engine::Pin::Finish(6_240))
    );
    assert!(
        !imported
            .unsupported
            .iter()
            .any(|line| line.contains("must finish on"))
    );
    assert!(
        imported
            .unsupported
            .iter()
            .any(|line| line
                .contains("with the secondary start on or before constraint but no date"))
    );
}

#[test]
fn project_start_floors_an_unlinked_activity() {
    let imported = import(&["1\t1\t10\tA1\tOnly milestone\tTT_FinMile\tTK_NotStart\t0\t0\t\t\t"])
        .expect("milestone-only project imports");
    assert_eq!(imported.schedule.tasks().len(), 2);
    assert_eq!(imported.schedule.tasks()[1].duration, 0);
    assert_eq!(imported.schedule.tasks()[1].snet, Some(3_360));
    assert_eq!(imported.tasks[0].floor, None, "a summary is not floored");
    assert_eq!(imported.tasks[1].floor, Some(3_360));
}

#[test]
fn project_start_on_the_first_working_minute_still_floors_every_activity() {
    // plan_start_date is the origin Monday 08:00, which is the first working minute (480).
    let text = file_with_plan_start(
        "2024-01-01 08:00",
        &[
            "1\t1\t10\tA1\tUnlinked\tTT_Task\tTK_NotStart\t1\t1\t\t\t",
            "2\t1\t10\tA2\tEarlier MSOA\tTT_Task\tTK_NotStart\t1\t1\tCS_MSOA\t2023-12-01 08:00\t",
            "3\t1\t10\tA3\tLater MSOA\tTT_Task\tTK_NotStart\t1\t1\tCS_MSOA\t2024-01-05 08:00\t",
        ],
    );
    let imported = parse_xer(text.as_bytes(), &ProjectSelection::Unspecified)
        .expect("project at the first working minute imports");
    let snets: Vec<_> = imported.schedule.tasks()[1..]
        .iter()
        .map(|task| task.snet)
        .collect();
    assert_eq!(snets, [Some(480), Some(480), Some(6_240)]);
}

#[test]
fn negative_and_non_finite_durations_are_invalid() {
    let error = import(&["1\t1\t10\tA1\tNegative\tTT_Task\tTK_NotStart\t-1\t-1\t\t\t"])
        .expect_err("negative duration should fail");
    assert!(matches!(error, ImportError::Invalid { .. }));

    let mut row = task_row();
    row.remain_drtn_hr_cnt = Some(f64::INFINITY);
    assert!(matches!(
        duration(&row, "A1", &mut Notes::new()),
        Err(ImportError::Invalid { .. })
    ));

    let error = import(&["1\t1\t10\tA1\tInfinite\tTT_Task\tTK_NotStart\tinf\tinf\t\t\t"])
        .expect_err("non-finite XER duration should fail");
    assert!(matches!(error, ImportError::Invalid { .. }));
}

fn import(rows: &[&str]) -> Result<crate::Imported, ImportError> {
    parse_xer(file(rows).as_bytes(), &ProjectSelection::Unspecified)
}

fn file(rows: &[&str]) -> String {
    file_with_plan_start("2024-01-03 08:00", rows)
}

fn file_with_plan_start(plan_start: &str, rows: &[&str]) -> String {
    let mut text = format!(
        "ERMHDR\t19.12\t2024-01-01 00:00\n\
         %T\tPROJECT\n\
         %F\tproj_id\tproj_short_name\texport_flag\tplan_start_date\n\
         %R\t1\tSHORT\tY\t{plan_start}\n\
         %T\tPROJWBS\n\
         %F\twbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_short_name\twbs_name\n\
         %R\t10\t1\t\t1\tY\tROOT\tRoot\n\
         %T\tTASK\n\
         %F\ttask_id\tproj_id\twbs_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2\n",
    );
    for row in rows {
        text.push_str("%R\t");
        text.push_str(row);
        text.push('\n');
    }
    text.push_str("%E\n");
    text
}

fn task_row() -> TaskRow {
    TaskRow {
        task_id: 1,
        proj_id: 1,
        wbs_id: 10,
        clndr_id: None,
        task_code: Some("A1".to_owned()),
        task_name: None,
        task_type: None,
        status_code: Some(StatusCode::NotStarted),
        target_drtn_hr_cnt: None,
        remain_drtn_hr_cnt: None,
        cstr_type: None,
        cstr_date: None,
        cstr_type2: None,
        cstr_date2: None,
        early_start_date: None,
        early_end_date: None,
        late_start_date: None,
        late_end_date: None,
        act_start_date: None,
        act_end_date: None,
        target_start_date: None,
        target_end_date: None,
        total_float_hr_cnt: None,
        free_float_hr_cnt: None,
        complete_pct_type: None,
        phys_complete_pct: None,
        complete_pct: None,
    }
}
