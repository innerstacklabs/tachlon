//! Write an imported project as an XER file: the inverse of `parse_xer`.
//!
//! Today the file holds the `ERMHDR` line, the `PROJECT` rows, the `CALENDAR` table, the `PROJWBS`
//! rows and each task's row: its identity (id, project, WBS node, calendar, activity ID and name),
//! its status, durations, percent complete and date rules (the primary constraint, `cstr_type`
//! with `cstr_date`, and the secondary, `cstr_type2` with `cstr_date2`), and each project's start
//! and data date. Links come in a later story. The file is a pure function of its
//! inputs: the export time is a parameter, never the clock.

use xer::{Encoding, LineEnding, Writer};

use super::Exported;
use crate::{ImportError, Imported};
use calendars::write_calendars;
use links::write_links;
use rows::{write_header, write_projects};
use tasks::Layout;

mod calendars;
mod constraint;
mod links;
mod rows;
mod source;
mod start;
mod tasks;

/// How the project's tasks are laid out in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XerLayout {
    /// One project, named from the options, holding every task.
    One,
    /// The source was an XER: each level-1 summary is a project.
    FromXer,
}

/// What [`export_xer`] needs besides the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XerOptions {
    /// The export time written in the header, in minutes since 1970-01-01 00:00.
    pub exported_at: i64,
    /// The project's short name (`proj_short_name`).
    pub name: String,
    /// How the tasks are laid out.
    pub layout: XerLayout,
}

/// Write `imported` as an XER file: a header, the projects, the calendars, the WBS and the task
/// values (identity, durations, percent complete, constraint, status, actual dates and remaining
/// work). A project's calendar is its first task's, so the reader adds none the tasks do not use.
/// A leaf with no activity ID gets one in the file, not in `imported`. [`Exported::left_out`]
/// names what the file does not carry, such as a summary end of a link. For a schedule opened
/// from an XER, see [`XerOptions::export_with_source`].
///
/// # Errors
///
/// [`ImportError::Invalid`] when the export time or an exception date does not fit an XER date,
/// or a cell holds a character the file cannot carry.
pub fn export_xer(imported: &Imported, options: &XerOptions) -> Result<Exported, ImportError> {
    write(imported, options)
}

impl XerOptions {
    /// Write `imported` as [`export_xer`] does, given `source`, the bytes of the XER file the
    /// schedule was opened from. The file keeps the source's ids (a task by its activity ID within
    /// its project), its `ERMHDR` cells (the export time is new), its encoding and line ending, and
    /// its `CALENDAR` rows. A row the person inserted takes an activity ID and an id the source
    /// does not hold. The source rows the schedule no longer has are not written and are listed in
    /// [`Exported::left_out`].
    ///
    /// # Errors
    ///
    /// As [`export_xer`], and [`ImportError::Invalid`] when `source` is not an XER file.
    pub fn export_with_source(
        &self,
        imported: &Imported,
        source: &[u8],
    ) -> Result<Exported, ImportError> {
        let imported = rows::with_codes(imported, source)?;
        let written = write(&imported, self)?;
        let mut left_out = written.left_out;
        let bytes = source::apply(&written.bytes, source, &mut left_out)?;
        Ok(Exported { bytes, left_out })
    }
}

fn write(imported: &Imported, options: &XerOptions) -> Result<Exported, ImportError> {
    let layout = Layout::new(imported, options);
    let mut writer = Writer::new(Encoding::Utf8, LineEnding::Lf);
    write_header(&mut writer, options.exported_at)?;
    write_projects(
        &mut writer,
        &layout.projects,
        imported.origin_day,
        imported.data_date,
    )?;
    write_calendars(&mut writer, &imported.schedule, imported.origin_day)?;
    layout.write_wbs(&mut writer)?;
    let mut left_out = Vec::new();
    layout.write_tasks(&mut writer, imported.origin_day)?;
    layout.list_unwritten(&mut left_out);
    write_links(&mut writer, imported, &layout.projects, &mut left_out)?;
    Ok(Exported {
        bytes: writer.finish(),
        left_out,
    })
}
