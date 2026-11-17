//! An XER activity's duration: which field to read, the activity type, and the rounding notes.

use xer::{StatusCode, TaskRow, TaskType};

use super::ascii;
use crate::ImportError;
use crate::notes::{Notes, Section};
use crate::range;

pub(super) fn duration(
    activity: &TaskRow,
    label: &str,
    notes: &mut Notes,
) -> Result<i64, ImportError> {
    let (preferred, fallback) = match &activity.status_code {
        Some(StatusCode::Active | StatusCode::Complete) => {
            (activity.target_drtn_hr_cnt, activity.remain_drtn_hr_cnt)
        }
        Some(StatusCode::NotStarted) => (activity.remain_drtn_hr_cnt, activity.target_drtn_hr_cnt),
        Some(StatusCode::Other(code)) => {
            notes.add(
                Section::Activity,
                &format!(
                    "Activities with status {}, which this import does not know, are treated as not started, so their dates can differ from the file's",
                    ascii(code)
                ),
                Some(label),
            );
            (activity.remain_drtn_hr_cnt, activity.target_drtn_hr_cnt)
        }
        None => {
            notes.add(
                Section::Activity,
                "Activities with no status are treated as not started, so their dates can differ from the file's",
                Some(label),
            );
            (activity.remain_drtn_hr_cnt, activity.target_drtn_hr_cnt)
        }
    };
    let hours = if let Some(hours) = preferred {
        hours
    } else {
        let Some(hours) = fallback else {
            notes.add(Section::Activity, "Activities with no duration are given zero days, so their dates can differ from the file's", Some(label));
            return Ok(0);
        };
        notes.add(
            Section::Activity,
            "Activities with no duration in the usual field take it from the other one (remaining or original), so their dates can differ from the file's",
            Some(label),
        );
        hours
    };
    let minutes = hours * 60.0;
    if !minutes.is_finite() || minutes < 0.0 {
        return Err(ImportError::Invalid {
            element: format!("activity {} duration", activity.task_id),
            value: hours.to_string(),
        });
    }
    let rounded = minutes.round();
    if (minutes - rounded).abs() > 1e-6 {
        notes.add(
            Section::Activity,
            "Activity durations were rounded to the whole minute; dates can differ by under a minute",
            Some(label),
        );
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the check above leaves a finite value that is not negative, and `as` saturates at the top of the range instead of wrapping; `range::duration` then bounds it"
    )]
    let whole = rounded as i64;
    Ok(range::duration(whole, label, notes))
}

pub(super) fn apply_task_type(
    activity: &TaskRow,
    label: &str,
    duration: &mut i64,
    notes: &mut Notes,
) {
    // Spellings other than the observed TT_Task are unverified on a real P6 export
    // (ruling 3, gs#116 corpus).
    match &activity.task_type {
        Some(TaskType::Task) => {}
        Some(TaskType::StartMilestone | TaskType::FinishMilestone) => {
            if *duration != 0 {
                notes.add(Section::Activity, "Milestones that had a duration were made zero days long, so their dates can differ from the file's", Some(label));
            }
            *duration = 0;
        }
        Some(TaskType::LevelOfEffort) => {
            notes.add(
                Section::Activity,
                "Level of effort activities were imported as zero-day tasks and do not stretch over their span, so their dates can differ from the file's",
                Some(label),
            );
            *duration = 0;
        }
        Some(TaskType::WbsSummary) => {
            notes.add(
                Section::Activity,
                "WBS summary activities were imported as zero-day tasks and do not stretch over their span, so their dates can differ from the file's",
                Some(label),
            );
            *duration = 0;
        }
        Some(TaskType::ResourceDependent) => notes.add(
            Section::Activity,
            "Resource dependent activities are scheduled on their own calendar; the resource calendar was not read, so their dates can differ from the file's",
            Some(label),
        ),
        Some(TaskType::Other(code)) => notes.add(
            Section::Activity,
            &format!(
                "Activities of type {}, which this import does not know, are treated as ordinary tasks, so their dates can differ from the file's",
                ascii(code)
            ),
            Some(label),
        ),
        None => notes.add(
            Section::Activity,
            "Activities with no type are treated as ordinary tasks, so their dates can differ from the file's",
            Some(label),
        ),
    }
}
