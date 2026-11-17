use crate::{CalendarData, CalendarType, ConstraintType, PredType, StatusCode, TaskType};

/// Wall-clock minutes since 1970-01-01 00:00, without a time zone.
pub type Minutes = i64;

/// The typed values retained from the XER `ERMHDR` line; the cell positions match the public samples of P6 8.2, 8.4 and 20.12.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The XER version from cell position 0.
    pub version: Option<String>,
    /// The export date from cell position 1.
    pub export_date: Option<Minutes>,
    /// The currency from cell position 7.
    pub currency: Option<String>,
}

/// A typed row from the XER `PROJECT` table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRow {
    /// The project identifier.
    pub proj_id: u32,
    /// The project's short name.
    pub proj_short_name: Option<String>,
    /// The project's calendar identifier.
    pub clndr_id: Option<u32>,
    /// The project's planned start date.
    pub plan_start_date: Option<Minutes>,
    /// The project's planned end date.
    pub plan_end_date: Option<Minutes>,
    /// The project's last recalculation date.
    pub last_recalc_date: Option<Minutes>,
    /// Whether the project was exported rather than only referenced.
    pub export_flag: Option<bool>,
    /// The project's default percent complete type, raw (`CP_Drtn`, `CP_Phys`, `CP_Units`), from
    /// `def_complete_pct_type` or else `complete_pct_type`.
    pub complete_pct_type: Option<String>,
}

/// A typed row from the XER `PROJWBS` table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WbsRow {
    /// The work breakdown structure identifier.
    pub wbs_id: u32,
    /// The owning project identifier.
    pub proj_id: u32,
    /// The parent work breakdown structure identifier.
    pub parent_wbs_id: Option<u32>,
    /// The sibling sequence number.
    pub seq_num: Option<u32>,
    /// Whether this row is the project's root node.
    pub proj_node_flag: Option<bool>,
    /// The work breakdown structure short name.
    pub wbs_short_name: Option<String>,
    /// The work breakdown structure name.
    pub wbs_name: Option<String>,
}

/// A typed row from the XER `TASK` table.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRow {
    /// The activity identifier.
    pub task_id: u32,
    /// The owning project identifier.
    pub proj_id: u32,
    /// The owning work breakdown structure identifier.
    pub wbs_id: u32,
    /// The activity's calendar identifier.
    pub clndr_id: Option<u32>,
    /// The activity code.
    pub task_code: Option<String>,
    /// The activity name.
    pub task_name: Option<String>,
    /// The activity type.
    pub task_type: Option<TaskType>,
    /// The activity status.
    pub status_code: Option<StatusCode>,
    /// The target duration in hours.
    pub target_drtn_hr_cnt: Option<f64>,
    /// The remaining duration in hours.
    pub remain_drtn_hr_cnt: Option<f64>,
    /// The primary constraint type.
    pub cstr_type: Option<ConstraintType>,
    /// The primary constraint date.
    pub cstr_date: Option<Minutes>,
    /// The secondary constraint type.
    pub cstr_type2: Option<ConstraintType>,
    /// The secondary constraint date.
    pub cstr_date2: Option<Minutes>,
    /// The early start date.
    pub early_start_date: Option<Minutes>,
    /// The early end date.
    pub early_end_date: Option<Minutes>,
    /// The late start date.
    pub late_start_date: Option<Minutes>,
    /// The late end date.
    pub late_end_date: Option<Minutes>,
    /// The actual start date.
    pub act_start_date: Option<Minutes>,
    /// The actual end date.
    pub act_end_date: Option<Minutes>,
    /// The target start date.
    pub target_start_date: Option<Minutes>,
    /// The target end date.
    pub target_end_date: Option<Minutes>,
    /// The total float in hours.
    pub total_float_hr_cnt: Option<f64>,
    /// The free float in hours.
    pub free_float_hr_cnt: Option<f64>,
    /// The activity's own percent complete type, raw (`CP_Drtn`, `CP_Phys`, `CP_Units`).
    pub complete_pct_type: Option<String>,
    /// The physical percent complete, as the file states it.
    pub phys_complete_pct: Option<f64>,
    /// The duration or units percent complete (`complete_pct`), as the file states it.
    pub complete_pct: Option<f64>,
}

/// A typed row from the XER `TASKPRED` table.
#[derive(Clone, Debug, PartialEq)]
pub struct PredRow {
    /// The successor activity identifier.
    pub task_id: u32,
    /// The predecessor activity identifier.
    pub pred_task_id: u32,
    /// The successor project identifier.
    pub proj_id: Option<u32>,
    /// The predecessor project identifier.
    pub pred_proj_id: Option<u32>,
    /// The relationship type.
    pub pred_type: Option<PredType>,
    /// The relationship lag in hours.
    pub lag_hr_cnt: Option<f64>,
}

/// A typed row from the XER `SCHEDOPTIONS` table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedOptionsRow {
    /// The project identifier, when present and valid.
    pub proj_id: Option<u32>,
    /// The `sched_calendar_on_relationship_lag` field's raw text (the name is in public samples; the default when it is absent is unverified).
    pub sched_calendar_on_relationship_lag: Option<String>,
}

/// A typed row from the XER `CALENDAR` table.
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarRow {
    /// The calendar identifier.
    pub clndr_id: u32,
    /// The calendar name.
    pub clndr_name: Option<String>,
    /// The owning project identifier.
    pub proj_id: Option<u32>,
    /// The base calendar identifier.
    pub base_clndr_id: Option<u32>,
    /// The calendar type.
    pub clndr_type: Option<CalendarType>,
    /// The nominal working hours per day.
    pub day_hr_cnt: Option<f64>,
    /// The parsed weekly schedule and exceptions.
    pub clndr_data: Option<CalendarData>,
}

/// A typed row from the XER `ACTVTYPE` table: one activity code type (a column of codes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActvTypeRow {
    /// The activity code type identifier.
    pub actv_code_type_id: u32,
    /// The activity code type name.
    pub actv_code_type: Option<String>,
    /// The scope the type belongs to, raw (`AS_Global`, `AS_EPS`, `AS_Project`).
    pub actv_code_type_scope: Option<String>,
    /// The owning project identifier, set for a project-scope type.
    pub proj_id: Option<u32>,
    /// The sibling sequence number.
    pub seq_num: Option<u32>,
}

/// A typed row from the XER `ACTVCODE` table: one value of an activity code type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActvCodeRow {
    /// The activity code identifier.
    pub actv_code_id: u32,
    /// The activity code type this value belongs to.
    pub actv_code_type_id: u32,
    /// The parent activity code identifier, when the values form a tree.
    pub parent_actv_code_id: Option<u32>,
    /// The activity code's full name.
    pub actv_code_name: Option<String>,
    /// The activity code's short name.
    pub short_name: Option<String>,
    /// The sibling sequence number.
    pub seq_num: Option<u32>,
}

/// A typed row from the XER `TASKACTV` table: one activity code assigned to one activity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskActvRow {
    /// The activity identifier.
    pub task_id: u32,
    /// The activity code type of the assigned code.
    pub actv_code_type_id: u32,
    /// The assigned activity code identifier.
    pub actv_code_id: u32,
    /// The project identifier of the activity.
    pub proj_id: Option<u32>,
}
