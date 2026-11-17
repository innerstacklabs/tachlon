use super::{Cells, ValueWarnings, bad_warning, header_text, required_column};
use crate::value::{Parsed, parse_flag, parse_hours, parse_minutes, parse_u32};
use crate::{
    CalendarRow, CalendarType, ConstraintType, Error, Header, PredRow, PredType, ProjectRow,
    SchedOptionsRow, StatusCode, TaskRow, TaskType, Warning, WbsRow, Xer, parse_clndr_data,
};
use std::collections::HashSet;

pub(super) fn read_calendars(
    xer: &Xer<'_>,
    warnings: &mut ValueWarnings,
) -> Result<Vec<CalendarRow>, Error> {
    let Some(table) = xer.table("CALENDAR").filter(|table| !table.is_empty()) else {
        return Ok(Vec::new());
    };
    let id = required_column(table, "clndr_id")?;
    let (name, project, base, kind, hours, data) = (
        table.column("clndr_name"),
        table.column("proj_id"),
        table.column("base_clndr_id"),
        table.column("clndr_type"),
        table.column("day_hr_cnt"),
        table.column("clndr_data"),
    );
    let mut out = Vec::with_capacity(table.len());
    let mut seen = HashSet::new();
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "CALENDAR", row.line(), warnings);
        let clndr_id = cell.key(id, "clndr_id", parse_u32)?;
        if !seen.insert(clndr_id) {
            cell.4.push((
                row.line(),
                id,
                Warning::CalendarDuplicate {
                    clndr_id,
                    line: row.line(),
                },
            ));
        }
        let clndr_data = match cell.text(data) {
            Some(text) => {
                let parsed = parse_clndr_data(clndr_id, row.line(), &text)?;
                if let Some(column) = data {
                    cell.4.extend(
                        parsed
                            .warnings
                            .into_iter()
                            .map(|warning| (row.line(), column, warning)),
                    );
                }
                parsed.data
            }
            None => None,
        };
        out.push(CalendarRow {
            clndr_id,
            clndr_name: cell.text(name),
            proj_id: cell.optional(project, "proj_id", parse_u32),
            base_clndr_id: cell.optional(base, "base_clndr_id", parse_u32),
            clndr_type: cell.text(kind).map(|value| CalendarType::from_code(&value)),
            day_hr_cnt: cell.optional(hours, "day_hr_cnt", parse_hours),
            clndr_data,
        });
    }
    Ok(out)
}

pub(super) fn read_header(xer: &Xer<'_>, warnings: &mut ValueWarnings) -> Header {
    let row = xer.header();
    let encoding = xer.encoding();
    let export_date = row.raw_cell(1).and_then(|raw| {
        let value = encoding.decode(raw);
        match parse_minutes(&value) {
            Parsed::Empty => None,
            Parsed::Value(minutes) => Some(minutes),
            Parsed::Bad => {
                warnings.push((
                    row.line(),
                    1,
                    bad_warning("ERMHDR", "export_date", row.line(), &value),
                ));
                None
            }
        }
    });
    Header {
        version: header_text(row.raw_cell(0), encoding),
        export_date,
        currency: header_text(row.raw_cell(7), encoding),
    }
}

pub(super) fn read_projects(
    xer: &Xer<'_>,
    warnings: &mut ValueWarnings,
) -> Result<Vec<ProjectRow>, Error> {
    let Some(table) = xer.table("PROJECT").filter(|table| !table.is_empty()) else {
        return Ok(Vec::new());
    };
    let id = required_column(table, "proj_id")?;
    let complete_type = table
        .column("def_complete_pct_type")
        .or_else(|| table.column("complete_pct_type"));
    let (name, calendar, start, end, recalc, export) = (
        table.column("proj_short_name"),
        table.column("clndr_id"),
        table.column("plan_start_date"),
        table.column("plan_end_date"),
        table.column("last_recalc_date"),
        table.column("export_flag"),
    );
    let mut out = Vec::with_capacity(table.len());
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "PROJECT", row.line(), warnings);
        out.push(ProjectRow {
            proj_id: cell.key(id, "proj_id", parse_u32)?,
            proj_short_name: cell.text(name),
            clndr_id: cell.optional(calendar, "clndr_id", parse_u32),
            plan_start_date: cell.optional(start, "plan_start_date", parse_minutes),
            plan_end_date: cell.optional(end, "plan_end_date", parse_minutes),
            last_recalc_date: cell.optional(recalc, "last_recalc_date", parse_minutes),
            export_flag: cell.optional(export, "export_flag", parse_flag),
            complete_pct_type: cell.text(complete_type),
        });
    }
    Ok(out)
}

pub(super) fn read_wbs(xer: &Xer<'_>, warnings: &mut ValueWarnings) -> Result<Vec<WbsRow>, Error> {
    let Some(table) = xer.table("PROJWBS").filter(|table| !table.is_empty()) else {
        return Ok(Vec::new());
    };
    let (wbs_id, proj_id) = (
        required_column(table, "wbs_id")?,
        required_column(table, "proj_id")?,
    );
    let (parent, sequence, node, short_name, name) = (
        table.column("parent_wbs_id"),
        table.column("seq_num"),
        table.column("proj_node_flag"),
        table.column("wbs_short_name"),
        table.column("wbs_name"),
    );
    let mut out = Vec::with_capacity(table.len());
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "PROJWBS", row.line(), warnings);
        out.push(WbsRow {
            wbs_id: cell.key(wbs_id, "wbs_id", parse_u32)?,
            proj_id: cell.key(proj_id, "proj_id", parse_u32)?,
            parent_wbs_id: cell.optional(parent, "parent_wbs_id", parse_u32),
            seq_num: cell.optional(sequence, "seq_num", parse_u32),
            proj_node_flag: cell.optional(node, "proj_node_flag", parse_flag),
            wbs_short_name: cell.text(short_name),
            wbs_name: cell.text(name),
        });
    }
    Ok(out)
}

#[expect(clippy::too_many_lines, reason = "one pass over the TASK columns")]
pub(super) fn read_tasks(
    xer: &Xer<'_>,
    warnings: &mut ValueWarnings,
) -> Result<Vec<TaskRow>, Error> {
    let Some(table) = xer.table("TASK").filter(|table| !table.is_empty()) else {
        return Ok(Vec::new());
    };
    let (task_id, proj_id, wbs_id) = (
        required_column(table, "task_id")?,
        required_column(table, "proj_id")?,
        required_column(table, "wbs_id")?,
    );
    let column = |name| table.column(name);
    let (calendar, code, name, kind, status) = (
        column("clndr_id"),
        column("task_code"),
        column("task_name"),
        column("task_type"),
        column("status_code"),
    );
    let (target_duration, remain_duration) =
        (column("target_drtn_hr_cnt"), column("remain_drtn_hr_cnt"));
    let (constraint, constraint_date, constraint2, constraint_date2) = (
        column("cstr_type"),
        column("cstr_date"),
        column("cstr_type2"),
        column("cstr_date2"),
    );
    let (early_start, early_end, late_start, late_end) = (
        column("early_start_date"),
        column("early_end_date"),
        column("late_start_date"),
        column("late_end_date"),
    );
    let (actual_start, actual_end, target_start, target_end) = (
        column("act_start_date"),
        column("act_end_date"),
        column("target_start_date"),
        column("target_end_date"),
    );
    let (total_float, free_float) = (column("total_float_hr_cnt"), column("free_float_hr_cnt"));
    let (complete_type, phys_complete, complete) = (
        column("complete_pct_type"),
        column("phys_complete_pct"),
        column("complete_pct"),
    );

    let mut out = Vec::with_capacity(table.len());
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "TASK", row.line(), warnings);
        out.push(TaskRow {
            task_id: cell.key(task_id, "task_id", parse_u32)?,
            proj_id: cell.key(proj_id, "proj_id", parse_u32)?,
            wbs_id: cell.key(wbs_id, "wbs_id", parse_u32)?,
            clndr_id: cell.optional(calendar, "clndr_id", parse_u32),
            task_code: cell.text(code),
            task_name: cell.text(name),
            task_type: cell.text(kind).map(|value| TaskType::from_code(&value)),
            status_code: cell.text(status).map(|value| StatusCode::from_code(&value)),
            target_drtn_hr_cnt: cell.optional(target_duration, "target_drtn_hr_cnt", parse_hours),
            remain_drtn_hr_cnt: cell.optional(remain_duration, "remain_drtn_hr_cnt", parse_hours),
            cstr_type: cell
                .text(constraint)
                .map(|value| ConstraintType::from_code(&value)),
            cstr_date: cell.optional(constraint_date, "cstr_date", parse_minutes),
            cstr_type2: cell
                .text(constraint2)
                .map(|value| ConstraintType::from_code(&value)),
            cstr_date2: cell.optional(constraint_date2, "cstr_date2", parse_minutes),
            early_start_date: cell.optional(early_start, "early_start_date", parse_minutes),
            early_end_date: cell.optional(early_end, "early_end_date", parse_minutes),
            late_start_date: cell.optional(late_start, "late_start_date", parse_minutes),
            late_end_date: cell.optional(late_end, "late_end_date", parse_minutes),
            act_start_date: cell.optional(actual_start, "act_start_date", parse_minutes),
            act_end_date: cell.optional(actual_end, "act_end_date", parse_minutes),
            target_start_date: cell.optional(target_start, "target_start_date", parse_minutes),
            target_end_date: cell.optional(target_end, "target_end_date", parse_minutes),
            total_float_hr_cnt: cell.optional(total_float, "total_float_hr_cnt", parse_hours),
            free_float_hr_cnt: cell.optional(free_float, "free_float_hr_cnt", parse_hours),
            complete_pct_type: cell.text(complete_type),
            phys_complete_pct: cell.optional(phys_complete, "phys_complete_pct", parse_hours),
            complete_pct: cell.optional(complete, "complete_pct", parse_hours),
        });
    }
    Ok(out)
}

pub(super) fn read_preds(
    xer: &Xer<'_>,
    warnings: &mut ValueWarnings,
) -> Result<Vec<PredRow>, Error> {
    let Some(table) = xer.table("TASKPRED").filter(|table| !table.is_empty()) else {
        return Ok(Vec::new());
    };
    let (task_id, pred_task_id) = (
        required_column(table, "task_id")?,
        required_column(table, "pred_task_id")?,
    );
    let (proj_id, pred_proj_id, kind, lag) = (
        table.column("proj_id"),
        table.column("pred_proj_id"),
        table.column("pred_type"),
        table.column("lag_hr_cnt"),
    );
    let mut out = Vec::with_capacity(table.len());
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "TASKPRED", row.line(), warnings);
        out.push(PredRow {
            task_id: cell.key(task_id, "task_id", parse_u32)?,
            pred_task_id: cell.key(pred_task_id, "pred_task_id", parse_u32)?,
            proj_id: cell.optional(proj_id, "proj_id", parse_u32),
            pred_proj_id: cell.optional(pred_proj_id, "pred_proj_id", parse_u32),
            pred_type: cell.text(kind).map(|value| PredType::from_code(&value)),
            lag_hr_cnt: cell.optional(lag, "lag_hr_cnt", parse_hours),
        });
    }
    Ok(out)
}

pub(super) fn read_sched_options(
    xer: &Xer<'_>,
    warnings: &mut ValueWarnings,
) -> Vec<SchedOptionsRow> {
    let Some(table) = xer.table("SCHEDOPTIONS").filter(|table| !table.is_empty()) else {
        return Vec::new();
    };
    let proj_id = table.column("proj_id");
    let lag_calendar = table.column("sched_calendar_on_relationship_lag");
    let mut out = Vec::with_capacity(table.len());
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "SCHEDOPTIONS", row.line(), warnings);
        out.push(SchedOptionsRow {
            proj_id: cell.optional(proj_id, "proj_id", parse_u32),
            sched_calendar_on_relationship_lag: cell.text(lag_calendar),
        });
    }
    out
}
