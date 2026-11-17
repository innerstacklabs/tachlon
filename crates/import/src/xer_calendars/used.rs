use std::collections::HashSet;

use xer::{ProjectRow, Warning};

use crate::notes::{Notes, Section};

/// The calendars the selected projects use: each project's calendar, the calendar of each of
/// its activities, and every calendar on their base chains, because a used calendar takes its
/// week from the chain. An activity the import later skips still counts here, so a doubtful
/// calendar is reported rather than hidden. A chain that loops or names a missing calendar
/// just ends.
pub(super) fn calendars(data: &xer::Data, projects: &[&ProjectRow]) -> HashSet<u32> {
    let selected: HashSet<u32> = projects.iter().map(|project| project.proj_id).collect();
    let mut pending: Vec<u32> = projects
        .iter()
        .filter_map(|project| project.clndr_id)
        .collect();
    pending.extend(
        data.task_rows
            .iter()
            .filter(|task| selected.contains(&task.proj_id))
            .filter_map(|task| task.clndr_id),
    );
    let mut used = HashSet::new();
    while let Some(id) = pending.pop() {
        if used.insert(id)
            && let Some(base) = data.calendar(id).and_then(|row| row.base_clndr_id)
        {
            pending.push(base);
        }
    }
    used
}

/// One note per calendar problem the XER reader found in a calendar in `used`. A problem in a
/// calendar nothing uses is left out: it changes no date and would only add noise.
pub(super) fn report_warnings(data: &xer::Data, used: &HashSet<u32>, notes: &mut Notes) {
    for warning in data.warnings() {
        let (clndr_id, kind) = match warning {
            Warning::CalendarMalformed { clndr_id, .. } => (
                clndr_id,
                "part of its data is damaged and was not read; working time can differ from the file's",
            ),
            Warning::CalendarUnknownNode { clndr_id, .. } => (
                clndr_id,
                "it holds an item this import does not know and was not read; working time can differ from the file's",
            ),
            Warning::CalendarOverlap { clndr_id, .. } => (
                clndr_id,
                "a working period that overlaps another was not read; working hours can differ from the file's",
            ),
            Warning::CalendarCrossesMidnight { clndr_id, .. } => (
                clndr_id,
                "a working period that runs past midnight was not read; working hours can differ from the file's",
            ),
            Warning::CalendarDuplicate { clndr_id, .. } => (
                clndr_id,
                "the file lists it twice and the second listing was not read; working days can differ from the file's",
            ),
            Warning::LongRow { .. }
            | Warning::MissingEnd
            | Warning::AfterEnd { .. }
            | Warning::BadValue { .. } => continue,
        };
        if used.contains(clndr_id) {
            notes.add(
                Section::Calendar,
                &format!("Calendar {clndr_id}: {kind}"),
                None,
            );
        }
    }
}
