use std::collections::HashSet;

use xer::{Encoding, Warning};

use crate::notes::{Notes, Section};

pub(super) fn note_activities_without_project(data: &xer::Data, notes: &mut Notes) {
    let project_ids: HashSet<_> = data.project_rows.iter().map(|row| row.proj_id).collect();
    for activity in &data.task_rows {
        if !project_ids.contains(&activity.proj_id) {
            let label = activity
                .task_code
                .as_deref()
                .map_or_else(|| activity.task_id.to_string(), ascii);
            notes.add(
                Section::Activity,
                "Activities in a project that is not in the file were not read, and links to them were dropped, so dates can differ from the file's",
                Some(&label),
            );
        }
    }
}

pub(super) fn file_notes(data: &xer::Data, notes: &mut Notes) {
    if data.encoding() == Encoding::Windows1252 {
        notes.add(
            Section::File,
            "The file is not saved as UTF-8; its text was converted and no dates are affected",
            None,
        );
    }
    for warning in data.warnings() {
        match warning {
            Warning::MissingEnd => notes.add(Section::File, "The file has no end marker, so it may stop early and some of it may not have been read; dates can differ from the file's", None),
            Warning::AfterEnd { .. } => notes.add(Section::File, "The file has content after its end marker that was not read; if it held schedule data, dates can differ from the file's", None),
            Warning::LongRow { table, .. } => notes.add(
                Section::File,
                &format!("A row in the {} table has more values than the table has columns; the extra values were not read, so a date can differ from the file's if it was one of them", ascii(table)),
                None,
            ),
            Warning::BadValue { table, field, .. } => notes.add(
                Section::File,
                &format!("A value in the {} table, column {}, could not be read and was left out; if it was a date or a duration, dates can differ from the file's", ascii(table), ascii(field)),
                None,
            ),
            Warning::CalendarMalformed { .. }
            | Warning::CalendarUnknownNode { .. }
            | Warning::CalendarOverlap { .. }
            | Warning::CalendarCrossesMidnight { .. }
            | Warning::CalendarDuplicate { .. } => {}
        }
    }
    let tables: Vec<String> = data
        .not_read
        .iter()
        .filter(|table| table.rows > 0)
        .map(|table| format!("{} ({} rows)", ascii(&table.name), table.rows))
        .collect();
    if !tables.is_empty() {
        notes.add(
            Section::File,
            &format!("Parts of the file that are not read, with their row counts: {}; the schedule is calculated without them, so resource-driven dates can differ from the file's", tables.join(", ")),
            None,
        );
    }
}

fn ascii(value: &str) -> String {
    value
        .chars()
        .map(
            |character| {
                if character.is_ascii() { character } else { '?' }
            },
        )
        .collect()
}
