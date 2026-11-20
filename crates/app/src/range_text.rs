//! The range as text for the clipboard (gs#703, R5): a header line and one tab-separated line per
//! visible row, in the columns the table shows from the start (ID when the file has IDs, WBS when
//! it has WBS codes, then Task, Dur, Start, Finish, Float), then, as the table orders them, the
//! three variance columns while a baseline is set and Actual start, Actual finish, Remaining and
//! % complete when the file has progress. A file with neither copies the five columns alone.

use crate::{Session, meta};
use engine::TaskId;

impl Session {
    /// The range's rows, or with no range the selected row, as tab-separated text: a header line,
    /// then one line per row in table order. With no range and a summary selected, the rows are
    /// the summary and everything under it in outline order, collapsed children included. `None` with nothing to copy (no selection, or a
    /// selection on no row). A name's tabs and line breaks read as spaces so a line stays one
    /// line of cells; the Task cell has no indentation; a summary's Float cell is empty, as the
    /// table shows it.
    pub fn range_text(&self) -> Option<String> {
        let tasks = match self.range_tasks() {
            tasks if !tasks.is_empty() => tasks,
            _ => {
                let task = self.selection()?.task;
                self.visible_row_of(task)?;
                // A summary copies the work under it too, collapsed children included (Q6).
                match self.outline() {
                    Some(outline) if outline.is_summary(TaskId(task)) => outline
                        .subtree(TaskId(task))
                        .iter()
                        .map(|id| id.0)
                        .collect(),
                    _ => vec![task],
                }
            }
        };
        Some(self.rows_text(tasks, None))
    }

    /// Every visible row (after collapse and the row filter) as the same text `range_text` makes:
    /// a header line, then one line per row in table order. The page prints it (Ctrl+P).
    pub fn list_text(&self) -> String {
        let tasks = self.timeline().order().to_vec();
        self.rows_text(tasks, None)
    }

    /// The header line and one line per task in `tasks`, in `columns` or the columns the table shows. A name's tabs
    /// and line breaks read as spaces so a line stays one line of cells.
    pub(crate) fn rows_text(&self, tasks: Vec<u32>, columns: Option<&[String]>) -> String {
        self.rows_cells(tasks, columns)
            .iter()
            .map(|cells| {
                cells
                    .iter()
                    .map(|cell| cell.replace(['\t', '\n', '\r'], " "))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The header cells, then the cells of each task in `tasks`, with every cell as the table reads it (a name keeps its tabs
    /// and line breaks): in the columns the table shows, or with `Some(columns)` only those named, in that order. A name the
    /// file has no column for (ID with no IDs, a variance column with no baseline, a progress column with no progress) and a
    /// repeated name leave no cell.
    pub(crate) fn rows_cells(
        &self,
        tasks: Vec<u32>,
        columns: Option<&[String]>,
    ) -> Vec<Vec<String>> {
        // Free float is a column only a layout or a print asks for by name; the copy the page makes from the start has the five columns it always had.
        let free = columns.is_some_and(|names| names.iter().any(|name| name == "Free float"));
        let rows = self.all_cells(tasks, free);
        let Some(names) = columns else { return rows };
        let mut picked: Vec<usize> = Vec::new();
        for name in names {
            let at = rows[0].iter().position(|head| head == name);
            picked.extend(at.filter(|at| !picked.contains(at)));
        }
        rows.iter()
            .map(|row| picked.iter().map(|&at| row[at].clone()).collect())
            .collect()
    }

    fn all_cells(&self, tasks: Vec<u32>, free: bool) -> Vec<Vec<String>> {
        let ids = self.has_codes();
        let wbs = meta::has_wbs_codes(self.meta());

        let mut header = Vec::new();
        if ids {
            header.push("ID");
        }
        if wbs {
            header.push("WBS");
        }
        header.extend(["Task", "Dur", "Start", "Finish", "Float"]);
        if free {
            header.push("Free float");
        }
        let variance = self.baseline().is_some();
        let progress = self.has_progress();
        if variance {
            header.extend(["Start variance", "Finish variance", "Float variance"]);
        }
        if progress {
            header.extend(["Actual start", "Actual finish", "Remaining", "% complete"]);
        }

        let mut rows = vec![header.iter().map(ToString::to_string).collect()];
        for task in tasks {
            let (start, finish) = self.summary_span(task).unwrap_or_else(|| {
                let dates = &self.solution().dates[task as usize];
                (dates.es, dates.ef)
            });
            let float = if self.summary_span(task).is_some() {
                String::new()
            } else {
                self.task_float(task)
            };
            let mut cells = Vec::new();
            if ids {
                cells.push(self.task_code(task).to_string());
            }
            if wbs {
                cells.push(self.meta()[task as usize].wbs_code_text().to_string());
            }
            cells.extend([
                self.task_label(task).to_string(),
                self.task_duration(task),
                self.date_text(start),
                self.date_text(finish),
                float,
            ]);
            if free {
                // A summary and a finished task have an empty cell, as the table shows (the engine's value for a finished task is 0).
                cells.push(if self.summary_span(task).is_some() {
                    String::new()
                } else {
                    self.free_float_text(task).unwrap_or_default()
                });
            }
            if variance {
                // A summary, or a task the baseline has no entry for, has empty variance cells.
                let cells_of = if self.summary_span(task).is_some() {
                    None
                } else {
                    self.variance_cells(TaskId(task))
                };
                let [vs, vf, vl] = cells_of.unwrap_or_default();
                cells.extend([vs, vf, vl].map(Option::unwrap_or_default));
            }
            if progress {
                cells.extend([
                    self.task_actual_start(task),
                    self.task_actual_finish(task),
                    self.task_remaining(task),
                    self.task_percent(task),
                ]);
            }
            rows.push(cells);
        }
        rows
    }
}
