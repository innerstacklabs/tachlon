use super::convert_periods;
use crate::ImportError;
use crate::calendar_rule::{Exception, dedupe_counting};
use crate::notes::{Notes, Section};

/// The exceptions of calendar `clndr_id` on or after the project origin, one per date, in the
/// engine's day numbering. The notes about them go to `pending`.
pub(super) fn read(
    data: Option<&xer::CalendarData>,
    clndr_id: u32,
    origin_day: i64,
    pending: &mut Notes,
) -> Result<Vec<Exception>, ImportError> {
    let mut found: Vec<Exception> = Vec::new();
    for exception in data.map_or(&[][..], |data| &data.exceptions[..]) {
        let day = i64::from(exception.serial)
            .checked_sub(25_569)
            .and_then(|day| day.checked_sub(origin_day))
            .ok_or_else(|| ImportError::Invalid {
                element: format!("calendar {clndr_id} exception"),
                value: exception.serial.to_string(),
            })?;
        if day < 0 {
            continue;
        }
        if exception.unparsed_periods > 0 {
            note_unparsed(exception, clndr_id, pending);
            if exception.periods.is_empty() {
                // Not a holiday: the day keeps its base-week hours.
                continue;
            }
        }
        found.push((day, convert_periods(&exception.periods, clndr_id, pending)));
    }
    let (exceptions, repeats) = dedupe_counting(found);
    pending.add_many(
        Section::Calendar,
        &format!("Calendar {clndr_id}: two exceptions fall on one date and only one was read; working days can differ from the file's"),
        repeats,
    );
    Ok(exceptions)
}

/// One note per exception that lost period nodes, naming the calendar and the date.
fn note_unparsed(exception: &xer::CalendarException, clndr_id: u32, notes: &mut Notes) {
    let kind = if exception.periods.is_empty() {
        format!(
            "Calendar {clndr_id}: an exception whose working periods could not be read was not read; that day keeps its normal hours, so working days can differ from the file's"
        )
    } else {
        format!(
            "Calendar {clndr_id}: some working periods of an exception could not be read and were left out; working hours on that day can differ from the file's"
        )
    };
    notes.add(Section::Calendar, &kind, Some(&exception.date()));
}
