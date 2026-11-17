//! The table writers: one function per table, in file order.

use super::blobs::{
    date, full_day_blob, id, project_blob, project_calendar_id, project_id, task_id, wbs_id,
    weekday_blob, wrap_u32,
};
use super::names::{task_name, wbs_name};
use super::{Counts, Names, Output, Spec, SplitMix64};

const PROJECT: &str = "%F\tproj_id\tfy_start_month_num\trsrc_self_add_flag\tallow_complete_flag\trsrc_multi_assign_flag\tcheckout_flag\tproject_flag\tstep_complete_flag\tcost_qty_recalc_flag\tbatch_sum_flag\tname_sep_char\tdef_complete_pct_type\tproj_short_name\tacct_id\torig_proj_id\tsource_proj_id\tbase_type_id\tclndr_id\tsum_base_proj_id\ttask_code_base\ttask_code_step\tpriority_num\twbs_max_sum_level\tstrgy_priority_num\tlast_checksum\tcritical_drtn_hr_cnt\tdef_cost_per_qty\tlast_recalc_date\tplan_start_date\tplan_end_date\tscd_end_date\tadd_date\tlast_tasksum_date\tfcst_start_date\tdef_duration_type\ttask_code_prefix\tguid\tdef_qty_type\tadd_by_name\tweb_local_root_path\tproj_url\tdef_rate_type\tadd_act_remain_flag\tact_this_per_link_flag\tdef_task_type\tact_pct_link_flag\tcritical_path_type\ttask_code_prefix_flag\tdef_rollup_dates_flag\tuse_project_baseline_flag\trem_target_link_flag\treset_planned_flag\tallow_neg_act_flag\tsum_assign_level\tlast_fin_dates_id\tlast_baseline_update_date\tcr_external_key\tapply_actuals_date\tlocation_id\tloaded_scope_level\texport_flag\tnew_fin_dates_id\tnext_data_date\tclose_period_flag\tsum_refresh_date\ttrsrcsum_loaded";
const CALENDAR: &str = "%F\tclndr_id\tdefault_flag\tclndr_name\tproj_id\tbase_clndr_id\tlast_chng_date\tclndr_type\tday_hr_cnt\tweek_hr_cnt\tmonth_hr_cnt\tyear_hr_cnt\trsrc_private\tclndr_data";
const SCHEDOPTIONS: &str = "%F\tschedoptions_id\tproj_id\tsched_outer_depend_type\tsched_open_critical_flag\tsched_lag_early_start_flag\tsched_retained_logic\tsched_setplantoforecast\tsched_float_type\tsched_calendar_on_relationship_lag\tsched_use_expect_end_flag\tsched_progress_override\tlevel_float_thrs_cnt\tlevel_outer_assign_flag\tlevel_outer_assign_priority\tlevel_over_alloc_pct\tlevel_within_float_flag\tlevel_keep_sched_date_flag\tlevel_all_rsrc_flag\tsched_use_project_end_date_for_float\tenable_multiple_longest_path_calc\tlimit_multiple_longest_path_calc\tmax_multiple_longest_path\tuse_total_float_multiple_longest_paths\tkey_activity_for_multiple_longest_paths\tLevelPriorityList";
const PROJWBS: &str = "%F\twbs_id\tproj_id\tobs_id\tseq_num\tproj_node_flag\tsum_data_flag\tstatus_code\twbs_short_name\twbs_name\tphase_id\tparent_wbs_id\tev_user_pct\tev_etc_user_value\torig_cost\tindep_remain_total_cost\tann_dscnt_rate_pct\tdscnt_period_type\tindep_remain_work_qty\tanticip_start_date\tanticip_end_date\tev_compute_type\tev_etc_compute_type\tguid\ttmpl_guid\tplan_open_state";
const TASK: &str = "%F\ttask_id\tproj_id\twbs_id\tclndr_id\tphys_complete_pct\trev_fdbk_flag\tlock_plan_flag\tauto_compute_act_flag\tcomplete_pct_type\ttask_type\tduration_type\tstatus_code\ttask_code\ttask_name\trsrc_id\ttotal_float_hr_cnt\tfree_float_hr_cnt\tremain_drtn_hr_cnt\tact_work_qty\tremain_work_qty\ttarget_work_qty\ttarget_drtn_hr_cnt\ttarget_equip_qty\tact_equip_qty\tremain_equip_qty\tcstr_date\tact_start_date\tact_end_date\tlate_start_date\tlate_end_date\texpect_end_date\tearly_start_date\tearly_end_date\trestart_date\treend_date\ttarget_start_date\ttarget_end_date\trem_late_start_date\trem_late_end_date\tcstr_type\tpriority_type\tsuspend_date\tresume_date\tfloat_path\tfloat_path_order\tguid\ttmpl_guid\tcstr_date2\tcstr_type2\tdriving_path_flag\tact_this_per_work_qty\tact_this_per_equip_qty\texternal_early_start_date\texternal_late_end_date\tcreate_date\tupdate_date\tcreate_user\tupdate_user\tlocation_id";
const TASKPRED: &str = "%F\ttask_pred_id\ttask_id\tpred_task_id\tproj_id\tpred_proj_id\tpred_type\tlag_hr_cnt\tfloat_path\taref\tarls";
const ACTVTYPE: &str = "%F\tactv_code_type_id\tactv_short_len\tseq_num\tactv_code_type\tproj_id\twbs_id\tactv_code_type_scope";
const ACTVCODE: &str = "%F\tactv_code_id\tparent_actv_code_id\tactv_code_type_id\tactv_code_name\tshort_name\tseq_num\tcolor\ttotal_assignments";
const TASKACTV: &str = "%F\ttask_id\tactv_code_type_id\tactv_code_id\tproj_id";
const TASKRSRC: &str = "%F\ttaskrsrc_id\ttask_id\tproj_id\tcost_qty_link_flag\trole_id\tacct_id\trsrc_id\tpobs_id\tskill_level\tremain_qty\ttarget_qty\tremain_qty_per_hr\ttarget_lag_drtn_hr_cnt\ttarget_qty_per_hr\tact_ot_qty\tact_reg_qty\trelag_drtn_hr_cnt\tot_factor\tcost_per_qty\ttarget_cost\tact_reg_cost\tact_ot_cost\tremain_cost\tact_start_date\tact_end_date\trestart_date\treend_date\ttarget_start_date\ttarget_end_date\trem_late_start_date\trem_late_end_date\trollup_dates_flag\ttarget_crv\tremain_crv\tactual_crv\tts_pend_act_end_flag\tguid\trate_type\tact_this_per_cost\tact_this_per_qty\tcurv_id\trsrc_type\tcost_per_qty_source_type\tcreate_user\tcreate_date\thas_rsrchours\ttaskrsrc_sum_id";
const STANDARD_CALENDAR_ID: u32 = 100;

pub(super) fn header(out: &mut Output, seed: u64) {
    let month = seed % 12 + 1;
    let day = seed.rotate_left(17) % 28 + 1;
    out.formatted(format_args!(
        "ERMHDR\t19.12\t2024-{month:02}-{day:02} 08:00\tProject\tadmin\tadmin\tdb\tProject Management\tUSD"
    ));
    out.end_line();
}

pub(super) fn currency(out: &mut Output) {
    out.table(
        "CURRTYPE",
        "%F\tcurr_id\tcurr_short_name\tcurr_symbol\tdecimal_digit_cnt",
    );
    out.line("%R\t1\tUS Dollar\t$\t2");
}

pub(super) fn projects(out: &mut Output, counts: &Counts) {
    out.table("PROJECT", PROJECT);
    for project in 0..counts.projects {
        let project_id = project_id(project);
        let calendar_id = project_calendar_id(project);
        let project_number = project.saturating_add(1);
        out.row(PROJECT, |field, out| match field {
            "proj_id" => out.number(project_id),
            "proj_short_name" if project == 0 => out.raw("Café Project 1"),
            "proj_short_name" => out.formatted(format_args!("Project {project_number}")),
            "clndr_id" => out.number(calendar_id),
            "last_recalc_date" | "plan_start_date" | "add_date" | "fcst_start_date" => {
                date(out, project.saturating_mul(5), 8);
            }
            "plan_end_date" | "scd_end_date" => {
                date(out, project.saturating_mul(5).saturating_add(27), 17);
            }
            "export_flag" | "project_flag" | "allow_complete_flag" => out.raw("Y"),
            "name_sep_char" => out.raw("."),
            "def_complete_pct_type" => out.raw("CP_Drtn"),
            "def_duration_type" => out.raw("DT_FixedDrtn2"),
            "def_task_type" => out.raw("TT_Task"),
            "task_code_prefix" => out.formatted(format_args!("P{project_number}")),
            "task_code_step" => out.raw("10"),
            "add_by_name" => out.raw("admin"),
            "critical_path_type" => out.raw("CT_TotFloat"),
            "loaded_scope_level" => out.raw("7"),
            "guid" => out.formatted(format_args!("SYNTH-PROJECT-{project_id}")),
            _ => {}
        });
    }
}

pub(super) fn calendars(out: &mut Output, counts: &Counts) {
    out.table("CALENDAR", CALENDAR);
    calendar_row(
        out,
        100,
        "Standard",
        None,
        None,
        "CA_Base",
        &weekday_blob(false),
    );
    calendar_row(out, 101, "24 Hour", None, None, "CA_Base", &full_day_blob());
    for project in 0..counts.projects {
        calendar_row(
            out,
            project_calendar_id(project),
            "Project Calendar",
            Some(project_id(project)),
            Some(STANDARD_CALENDAR_ID),
            "CA_Project",
            &project_blob(project),
        );
    }
}

fn calendar_row(
    out: &mut Output,
    id: u32,
    name: &str,
    project: Option<u32>,
    base: Option<u32>,
    kind: &str,
    blob: &str,
) {
    out.row(CALENDAR, |field, out| match field {
        "clndr_id" => out.number(id),
        "default_flag" if id == STANDARD_CALENDAR_ID => out.raw("Y"),
        "clndr_name" => out.raw(name),
        "proj_id" => out.optional_number(project),
        "base_clndr_id" => out.optional_number(base),
        "clndr_type" => out.raw(kind),
        "day_hr_cnt" => out.raw("8"),
        "week_hr_cnt" => out.raw(if id == 101 { "168" } else { "40" }),
        "month_hr_cnt" => out.raw("172"),
        "year_hr_cnt" => out.raw("2080"),
        "rsrc_private" => out.raw("N"),
        "clndr_data" => out.raw(blob),
        _ => {}
    });
}

pub(super) fn schedule_options(out: &mut Output, counts: &Counts) {
    out.table("SCHEDOPTIONS", SCHEDOPTIONS);
    for project in 0..counts.projects {
        out.row(SCHEDOPTIONS, |field, out| match field {
            "schedoptions_id" => out.number(id(3_000, project, 1, 0)),
            "proj_id" => out.number(project_id(project)),
            "sched_calendar_on_relationship_lag" => out.raw("rcal_ProjDefault"),
            "sched_retained_logic" | "sched_open_critical_flag" => out.raw("Y"),
            "sched_float_type" => out.raw("FT_Total"),
            _ => {}
        });
    }
}

pub(super) fn wbs(out: &mut Output, counts: &Counts, activities: usize, names: Names) {
    out.table("PROJWBS", PROJWBS);
    let children = (activities / 20).clamp(1, 200);
    for project in 0..counts.projects {
        let project_number = project.saturating_add(1);
        for child in 0..=children {
            let row_wbs_id = wbs_id(project, child);
            out.row(PROJWBS, |field, out| match field {
                "wbs_id" => out.number(row_wbs_id),
                "proj_id" => out.number(project_id(project)),
                "seq_num" => out.number(child),
                "proj_node_flag" => out.raw(if child == 0 { "Y" } else { "N" }),
                "sum_data_flag" => out.raw("N"),
                "status_code" => out.raw("WS_Open"),
                "wbs_short_name" => out.formatted(format_args!("WBS-{project_number}-{child}")),
                "wbs_name" => out.raw(&wbs_name(names, child)),
                "parent_wbs_id" if child != 0 => out.number(wbs_id(project, 0)),
                "guid" => out.formatted(format_args!("SYNTH-WBS-{row_wbs_id}")),
                _ => {}
            });
        }
    }
}

pub(super) fn tasks(
    out: &mut Output,
    counts: &Counts,
    activities: usize,
    names: Names,
    rng: &mut SplitMix64,
) {
    out.table("TASK", TASK);
    let children = (activities / 20).clamp(1, 200);
    for project in 0..counts.projects {
        for activity in 0..activities {
            let task_id = task_id(project, activity);
            let project_number = project.saturating_add(1);
            let activity_number = activity.saturating_add(1);
            let milestone = activity % 10 == 9 && activity % 25 != 24;
            let duration = if milestone {
                0
            } else {
                (rng.below(10) + 1) * 8
            };
            let complete = activity % 11 == 0;
            let active = !complete && activity % 5 == 0;
            out.row(TASK, |field, out| match field {
                "task_id" => out.number(task_id),
                "proj_id" => out.number(project_id(project)),
                "wbs_id" => out.number(wbs_id(project, 1 + activity % children)),
                "clndr_id" => out.number(project_calendar_id(project)),
                "complete_pct_type" => out.raw("CP_Drtn"),
                "task_type" if activity % 25 == 24 => out.raw("TT_LOE"),
                "task_type" if milestone => out.raw("TT_Mile"),
                "task_type" => out.raw("TT_Task"),
                "duration_type" => out.raw("DT_FixedDrtn2"),
                "status_code" if complete => out.raw("TK_Complete"),
                "status_code" if active => out.raw("TK_Active"),
                "status_code" => out.raw("TK_NotStart"),
                "task_code" => out.formatted(format_args!("A{project_number}-{activity_number}")),
                "task_name" => out.raw(&task_name(names, children, activity)),
                "target_drtn_hr_cnt" => out.number(duration),
                "remain_drtn_hr_cnt" => out.number(if complete { 0 } else { duration }),
                "total_float_hr_cnt" => out.number((activity % 15) * 8),
                "free_float_hr_cnt" => out.number((activity % 4) * 8),
                "cstr_type" if activity % 13 == 12 => out.raw("CS_MSOA"),
                "cstr_date" if activity % 13 == 12 => date(out, activity.saturating_add(2), 8),
                "act_start_date" if complete || active => date(out, activity, 8),
                "act_end_date" if complete => date(out, activity.saturating_add(1), 17),
                "early_start_date" | "target_start_date" => {
                    date(out, activity.saturating_add(2), 8);
                }
                "early_end_date" | "target_end_date" => date(out, activity.saturating_add(3), 17),
                "late_start_date" => date(out, activity.saturating_add(4), 8),
                "late_end_date" => date(out, activity.saturating_add(5), 17),
                "guid" => out.formatted(format_args!("SYNTH-TASK-{task_id}")),
                "create_user" | "update_user" => out.raw("admin"),
                _ => {}
            });
        }
    }
}

pub(super) fn predecessors(out: &mut Output, counts: &Counts, spec: &Spec, rng: &mut SplitMix64) {
    out.table("TASKPRED", TASKPRED);
    let mut relation = 0usize;
    for project in 0..counts.projects {
        for activity in 1..spec.activities {
            let width = activity.min(5);
            let mut offset = rng.below(width);
            if activity % 7 == 0 && offset == 2 {
                offset = (offset + 1) % width;
            }
            pred_row(
                out,
                relation,
                project,
                activity,
                project,
                activity - 1 - offset,
                "PR_FS",
                0,
            );
            relation = relation.saturating_add(1);
            if activity >= 3 && activity % 7 == 0 {
                let kind = match rng.below(4) {
                    0 => "PR_FS",
                    1 => "PR_SS",
                    2 => "PR_FF",
                    _ => "PR_SF",
                };
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_possible_wrap,
                    reason = "below(8) returns 0..=7, which fits an i32"
                )]
                let lag = (rng.below(8) as i32 - 2) * 8;
                pred_row(
                    out,
                    relation,
                    project,
                    activity,
                    project,
                    activity - 3,
                    kind,
                    lag,
                );
                relation = relation.saturating_add(1);
            }
        }
        if spec.cross_project_links && project != 0 && spec.activities != 0 {
            pred_row(
                out,
                relation,
                project,
                0,
                project - 1,
                spec.activities - 1,
                "PR_FS",
                0,
            );
            relation = relation.saturating_add(1);
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "one argument per TASKPRED column the writer fills; the call sites read as the row"
)]
fn pred_row(
    out: &mut Output,
    relation: usize,
    project: usize,
    task: usize,
    pred_project: usize,
    pred_task: usize,
    kind: &str,
    lag: i32,
) {
    out.row(TASKPRED, |field, out| match field {
        "task_pred_id" => out.number(id(2_000_000, 0, 1, relation)),
        "task_id" => out.number(task_id(project, task)),
        "pred_task_id" => out.number(task_id(pred_project, pred_task)),
        "proj_id" => out.number(project_id(project)),
        "pred_proj_id" => out.number(project_id(pred_project)),
        "pred_type" => out.raw(kind),
        "lag_hr_cnt" => out.number(lag),
        _ => {}
    });
}

pub(super) fn resources(out: &mut Output, counts: &Counts, activities: usize) {
    out.table("TASKRSRC", TASKRSRC);
    for project in 0..counts.projects {
        for activity in 0..activities {
            for assignment in 0..2usize {
                let ordinal = project
                    .saturating_mul(activities)
                    .saturating_add(activity)
                    .saturating_mul(2)
                    .saturating_add(assignment);
                out.row(TASKRSRC, |field, out| match field {
                    "taskrsrc_id" => out.number(id(4_000_000, 0, 1, ordinal)),
                    "task_id" => out.number(task_id(project, activity)),
                    "proj_id" => out.number(project_id(project)),
                    "cost_qty_link_flag" | "rollup_dates_flag" => out.raw("Y"),
                    "rsrc_id" => out.number(5_000u32.wrapping_add(wrap_u32(assignment))),
                    "target_qty" | "remain_qty" => out.raw("8"),
                    "skill_level" | "target_qty_per_hr" | "remain_qty_per_hr" => out.raw("1"),
                    "cost_per_qty" => out.raw("100"),
                    "target_cost" | "remain_cost" => out.raw("800"),
                    "target_start_date" => date(out, activity.saturating_add(2), 8),
                    "target_end_date" => date(out, activity.saturating_add(3), 17),
                    "guid" => out.formatted(format_args!("SYNTH-RSRC-{ordinal}")),
                    "rate_type" => out.raw("RT_PricePerUnit"),
                    "rsrc_type" => out.raw("RT_Labor"),
                    "create_user" => out.raw("admin"),
                    _ => {}
                });
            }
        }
    }
}

/// The synthetic activity code type: one global type with `ACTIVITY_CODE_VALUES` values.
const ACTIVITY_CODE_TYPE_ID: u32 = 7_000;
const ACTIVITY_CODE_VALUES: u32 = 6;

pub(super) fn activity_codes(out: &mut Output, counts: &Counts, activities: usize) {
    out.table("ACTVTYPE", ACTVTYPE);
    out.row(ACTVTYPE, |field, out| match field {
        "actv_code_type_id" => out.number(ACTIVITY_CODE_TYPE_ID),
        "actv_short_len" => out.raw("8"),
        "seq_num" => out.raw("1"),
        "actv_code_type" => out.raw("Discipline"),
        "actv_code_type_scope" => out.raw("AS_Global"),
        _ => {}
    });
    out.table("ACTVCODE", ACTVCODE);
    for value in 0..ACTIVITY_CODE_VALUES {
        out.row(ACTVCODE, |field, out| match field {
            "actv_code_id" => out.number(7_100 + value),
            // The last value sits under the first, so the values form a tree.
            "parent_actv_code_id" if value == ACTIVITY_CODE_VALUES - 1 => out.number(7_100),
            "actv_code_type_id" => out.number(ACTIVITY_CODE_TYPE_ID),
            "actv_code_name" => out.formatted(format_args!("Discipline {value}")),
            "short_name" => out.formatted(format_args!("D{value}")),
            "seq_num" => out.number(value + 1),
            _ => {}
        });
    }
    out.table("TASKACTV", TASKACTV);
    for project in 0..counts.projects {
        for activity in 0..activities {
            out.row(TASKACTV, |field, out| match field {
                "task_id" => out.number(task_id(project, activity)),
                "actv_code_type_id" => out.number(ACTIVITY_CODE_TYPE_ID),
                "actv_code_id" => out.number(
                    7_100u32.wrapping_add(wrap_u32(activity % ACTIVITY_CODE_VALUES as usize)),
                ),
                "proj_id" => out.number(project_id(project)),
                _ => {}
            });
        }
    }
}
