//! Percent complete of an XER activity, for display only: it never reaches the engine schedule.

use xer::{ProjectRow, TaskRow};

use crate::notes::Notes;

/// Which percent columns the `TASK` table has.
#[derive(Clone, Copy)]
pub(crate) struct PercentColumns {
    phys: bool,
    complete: bool,
}

impl PercentColumns {
    pub(crate) fn of(parsed: &xer::Xer<'_>) -> Self {
        let has = |name| {
            parsed
                .table("TASK")
                .is_some_and(|table| table.column(name).is_some())
        };
        PercentColumns {
            phys: has("phys_complete_pct"),
            complete: has("complete_pct"),
        }
    }
}

/// The activity's percent complete, 0 to 100, in the column its `complete_pct_type` (else the
/// project's) names: `CP_Phys` reads `phys_complete_pct`, `CP_Drtn` and `CP_Units` read
/// `complete_pct`, and no declared type reads `phys_complete_pct`. A type whose column is absent
/// gives no progress and one note. A value outside 0 to 100 is clamped, with one note.
pub(crate) fn percent(
    columns: PercentColumns,
    project: &ProjectRow,
    task: &TaskRow,
    notes: &mut Notes,
) -> Option<u8> {
    let declared = task
        .complete_pct_type
        .as_deref()
        .or(project.complete_pct_type.as_deref());
    let physical = declared.is_none_or(|kind| !is_other(kind));
    let (present, name, value) = if physical {
        (
            columns.phys,
            "physical percent complete",
            task.phys_complete_pct,
        )
    } else {
        (
            columns.complete,
            "duration or units percent complete",
            task.complete_pct,
        )
    };
    if !present {
        notes.line(format!(
            "Percent complete was not read: the file measures it as {name}, and has no column for that; no dates are affected."
        ));
        return None;
    }
    let value = value?;
    if !(0.0..=100.0).contains(&value) {
        notes.line("A percent complete outside 0 to 100 was changed to the nearest limit; no dates are affected.".to_owned());
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is clamped to 0.0..=100.0 before the cast, so it is not negative and fits u8; a NaN, which clamp passes through, casts to 0"
    )]
    let percent = value.clamp(0.0, 100.0).round() as u8;
    Some(percent)
}

fn is_other(kind: &str) -> bool {
    ["cp_drtn", "cp_units", "ct_drtn", "ct_units"]
        .iter()
        .any(|other| kind.trim().eq_ignore_ascii_case(other))
}
