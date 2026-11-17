//! Map the XER activity code tables onto the imported tasks.
//!
//! `TASKACTV` assigns one code (`ACTVCODE`) of one code type (`ACTVTYPE`) to one activity. A task
//! gets its assignments in file order; a task with no row gets none.

use std::collections::HashMap;

use xer::{ActvCodeRow, ActvTypeRow, Data};

use crate::notes::{Notes, Section};
use crate::xer_activities::TaskInfo;
use crate::{ActivityCode, ImportedTask};

/// Set `activity_codes` on every imported activity. `task_info` parallels `imported`; a summary
/// keeps an empty list. An assignment naming a code or type the file lacks, or a code with no name,
/// is dropped and noted; that includes a file with `TASKACTV` rows but no `ACTVTYPE` or no
/// `ACTVCODE` rows at all, which cannot name any code.
pub(crate) fn attach(
    data: &Data,
    task_info: &[TaskInfo],
    imported: &mut [ImportedTask],
    notes: &mut Notes,
) {
    if data.task_actv_rows.is_empty() {
        return;
    }
    let types: HashMap<u32, &ActvTypeRow> = data
        .actv_type_rows
        .iter()
        .map(|row| (row.actv_code_type_id, row))
        .collect();
    let codes: HashMap<u32, &ActvCodeRow> = data
        .actv_code_rows
        .iter()
        .map(|row| (row.actv_code_id, row))
        .collect();
    let mut by_task: HashMap<u32, Vec<ActivityCode>> = HashMap::new();
    for row in &data.task_actv_rows {
        let task = row.task_id.to_string();
        let (Some(code_type), Some(code)) = (
            types.get(&row.actv_code_type_id),
            codes.get(&row.actv_code_id),
        ) else {
            notes.add(
                Section::Activity,
                "Activity codes whose code or code type is not in the file were not read; no dates are affected",
                Some(&task),
            );
            continue;
        };
        let Some(value) = code
            .short_name
            .clone()
            .or_else(|| code.actv_code_name.clone())
        else {
            notes.add(
                Section::Activity,
                "Activity codes with no name were not read; no dates are affected",
                Some(&task),
            );
            continue;
        };
        by_task.entry(row.task_id).or_default().push(ActivityCode {
            code_type: code_type
                .actv_code_type
                .clone()
                .unwrap_or_else(|| format!("code type {}", code_type.actv_code_type_id)),
            value,
        });
    }
    for (task, info) in imported.iter_mut().zip(task_info) {
        if info.is_activity
            && let Some(assigned) = by_task.remove(&task.uid)
        {
            task.activity_codes = assigned;
        }
    }
}
