use crate::rows::{
    ActvCodeRow, ActvTypeRow, CalendarRow, Header, PredRow, ProjectRow, SchedOptionsRow,
    TaskActvRow, TaskRow, WbsRow,
};
use crate::{Encoding, Warning};
use std::collections::HashMap;

/// A table omitted from typed reading in this layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotRead {
    /// The table name.
    pub name: String,
    /// The number of rows in the table.
    pub rows: usize,
}

/// A compact summary of one project in file order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSummary {
    /// The project identifier.
    pub proj_id: u32,
    /// The project's short name.
    pub proj_short_name: Option<String>,
    /// The number of activities belonging to the project.
    pub activity_count: usize,
    /// Whether the project was exported rather than only referenced, an unverified interpretation.
    pub exported: bool,
}

/// Typed XER rows, retained metadata, and recoverable warnings.
#[derive(Debug, Clone)]
pub struct Data {
    /// The typed XER header.
    pub header: Header,
    /// Rows read from the `PROJECT` table.
    pub project_rows: Vec<ProjectRow>,
    /// Rows read from the `PROJWBS` table.
    pub wbs_rows: Vec<WbsRow>,
    /// Rows read from the `TASK` table.
    pub task_rows: Vec<TaskRow>,
    /// Rows read from the `TASKPRED` table.
    pub pred_rows: Vec<PredRow>,
    /// Rows read from the `SCHEDOPTIONS` table.
    pub sched_options_rows: Vec<SchedOptionsRow>,
    /// Rows read from the `CALENDAR` table.
    pub calendar_rows: Vec<CalendarRow>,
    /// Rows read from the `ACTVTYPE` table.
    pub actv_type_rows: Vec<ActvTypeRow>,
    /// Rows read from the `ACTVCODE` table.
    pub actv_code_rows: Vec<ActvCodeRow>,
    /// Rows read from the `TASKACTV` table.
    pub task_actv_rows: Vec<TaskActvRow>,
    /// Tables not interpreted by the typed reader, in file order.
    pub not_read: Vec<NotRead>,
    encoding: Encoding,
    warnings: Vec<Warning>,
    pub(crate) calendar_index: HashMap<u32, usize>,
}

/// Crate-private retained metadata grouped for `Data` construction.
pub(crate) struct DataMetadata {
    /// Tables not interpreted by the typed reader, in file order.
    pub(crate) not_read: Vec<NotRead>,
    /// The encoding detected for the XER file.
    pub(crate) encoding: Encoding,
    /// Recoverable table and typed-value warnings in encounter order.
    pub(crate) warnings: Vec<Warning>,
}

impl Data {
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per table the reader builds; a parameter struct would repeat the field list of Data"
    )]
    pub(crate) fn new(
        header: Header,
        project_rows: Vec<ProjectRow>,
        wbs_rows: Vec<WbsRow>,
        task_rows: Vec<TaskRow>,
        pred_rows: Vec<PredRow>,
        sched_options_rows: Vec<SchedOptionsRow>,
        calendar_rows: Vec<CalendarRow>,
        actv_type_rows: Vec<ActvTypeRow>,
        actv_code_rows: Vec<ActvCodeRow>,
        task_actv_rows: Vec<TaskActvRow>,
        metadata: DataMetadata,
    ) -> Self {
        let mut calendar_index = HashMap::new();
        for (index, calendar) in calendar_rows.iter().enumerate() {
            calendar_index.entry(calendar.clndr_id).or_insert(index);
        }
        Self {
            header,
            project_rows,
            wbs_rows,
            task_rows,
            pred_rows,
            sched_options_rows,
            calendar_rows,
            actv_type_rows,
            actv_code_rows,
            task_actv_rows,
            not_read: metadata.not_read,
            encoding: metadata.encoding,
            warnings: metadata.warnings,
            calendar_index,
        }
    }

    /// Returns one summary per project row, preserving file order.
    #[must_use]
    pub fn projects(&self) -> Vec<ProjectSummary> {
        let mut activity_counts = HashMap::<u32, usize>::new();
        for task in &self.task_rows {
            let count = activity_counts.entry(task.proj_id).or_default();
            *count = count.saturating_add(1);
        }

        self.project_rows
            .iter()
            .map(|project| ProjectSummary {
                proj_id: project.proj_id,
                proj_short_name: project.proj_short_name.clone(),
                activity_count: activity_counts.get(&project.proj_id).copied().unwrap_or(0),
                exported: project.export_flag != Some(false),
            })
            .collect()
    }

    /// Returns the encoding detected for the XER file.
    #[must_use]
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Returns recoverable table and typed-value warnings in encounter order.
    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }
}

#[cfg(test)]
mod tests {
    use super::{Data, DataMetadata};
    use crate::Encoding;
    use crate::rows::{Header, ProjectRow, TaskRow};

    #[test]
    fn projects_count_activities_and_respect_explicit_false_export_flags() {
        let data = Data::new(
            Header {
                version: None,
                export_date: None,
                currency: None,
            },
            vec![
                project(10, "exported", None),
                project(20, "referenced", Some(false)),
            ],
            Vec::new(),
            vec![task(10), task(10), task(20), task(30)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            DataMetadata {
                not_read: Vec::new(),
                encoding: Encoding::Utf8,
                warnings: Vec::new(),
            },
        );

        let projects = data.projects();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].proj_id, 10);
        assert_eq!(projects[0].proj_short_name.as_deref(), Some("exported"));
        assert_eq!(projects[0].activity_count, 2);
        assert!(projects[0].exported);
        assert_eq!(projects[1].proj_id, 20);
        assert_eq!(projects[1].activity_count, 1);
        assert!(!projects[1].exported);
    }

    fn project(proj_id: u32, name: &str, export_flag: Option<bool>) -> ProjectRow {
        ProjectRow {
            proj_id,
            proj_short_name: Some(name.to_owned()),
            clndr_id: None,
            plan_start_date: None,
            plan_end_date: None,
            last_recalc_date: None,
            export_flag,
            complete_pct_type: None,
        }
    }

    fn task(proj_id: u32) -> TaskRow {
        TaskRow {
            task_id: proj_id,
            proj_id,
            wbs_id: 1,
            clndr_id: None,
            task_code: None,
            task_name: None,
            task_type: None,
            status_code: None,
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
}
