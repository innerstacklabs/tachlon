//! The activity code tables: `ACTVTYPE` (the types), `ACTVCODE` (the values) and `TASKACTV`
//! (which value each activity carries).

use super::{Cells, ValueWarnings, required_column};
use crate::value::parse_u32;
use crate::{ActvCodeRow, ActvTypeRow, Error, TaskActvRow, Xer};

pub(super) fn read_actv_types(
    xer: &Xer<'_>,
    warnings: &mut ValueWarnings,
) -> Result<Vec<ActvTypeRow>, Error> {
    let Some(table) = xer.table("ACTVTYPE").filter(|table| !table.is_empty()) else {
        return Ok(Vec::new());
    };
    let type_id = required_column(table, "actv_code_type_id")?;
    let (name, scope, project, sequence) = (
        table.column("actv_code_type"),
        table.column("actv_code_type_scope"),
        table.column("proj_id"),
        table.column("seq_num"),
    );
    let mut out = Vec::with_capacity(table.len());
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "ACTVTYPE", row.line(), warnings);
        out.push(ActvTypeRow {
            actv_code_type_id: cell.key(type_id, "actv_code_type_id", parse_u32)?,
            actv_code_type: cell.text(name),
            actv_code_type_scope: cell.text(scope),
            proj_id: cell.optional(project, "proj_id", parse_u32),
            seq_num: cell.optional(sequence, "seq_num", parse_u32),
        });
    }
    Ok(out)
}

pub(super) fn read_actv_codes(
    xer: &Xer<'_>,
    warnings: &mut ValueWarnings,
) -> Result<Vec<ActvCodeRow>, Error> {
    let Some(table) = xer.table("ACTVCODE").filter(|table| !table.is_empty()) else {
        return Ok(Vec::new());
    };
    let (code_id, type_id) = (
        required_column(table, "actv_code_id")?,
        required_column(table, "actv_code_type_id")?,
    );
    let (parent, name, short_name, sequence) = (
        table.column("parent_actv_code_id"),
        table.column("actv_code_name"),
        table.column("short_name"),
        table.column("seq_num"),
    );
    let mut out = Vec::with_capacity(table.len());
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "ACTVCODE", row.line(), warnings);
        out.push(ActvCodeRow {
            actv_code_id: cell.key(code_id, "actv_code_id", parse_u32)?,
            actv_code_type_id: cell.key(type_id, "actv_code_type_id", parse_u32)?,
            parent_actv_code_id: cell.optional(parent, "parent_actv_code_id", parse_u32),
            actv_code_name: cell.text(name),
            short_name: cell.text(short_name),
            seq_num: cell.optional(sequence, "seq_num", parse_u32),
        });
    }
    Ok(out)
}

pub(super) fn read_task_actvs(
    xer: &Xer<'_>,
    warnings: &mut ValueWarnings,
) -> Result<Vec<TaskActvRow>, Error> {
    let Some(table) = xer.table("TASKACTV").filter(|table| !table.is_empty()) else {
        return Ok(Vec::new());
    };
    let (task_id, type_id, code_id) = (
        required_column(table, "task_id")?,
        required_column(table, "actv_code_type_id")?,
        required_column(table, "actv_code_id")?,
    );
    let project = table.column("proj_id");
    let mut out = Vec::with_capacity(table.len());
    let mut cells = Vec::with_capacity(table.field_count());
    for row in table.rows() {
        cells.clear();
        cells.extend(row.raw_cells());
        let mut cell = Cells(&cells, xer.encoding(), "TASKACTV", row.line(), warnings);
        out.push(TaskActvRow {
            task_id: cell.key(task_id, "task_id", parse_u32)?,
            actv_code_type_id: cell.key(type_id, "actv_code_type_id", parse_u32)?,
            actv_code_id: cell.key(code_id, "actv_code_id", parse_u32)?,
            proj_id: cell.optional(project, "proj_id", parse_u32),
        });
    }
    Ok(out)
}
