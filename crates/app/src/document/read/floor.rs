//! The task `floor` of a document (gs#703 K2): the instant an import floored the task at.

use super::{Field, int, opt};
use crate::constraint::fallback_floor;
use crate::document::json_read::Value;
use crate::meta::TaskMeta;
use engine::{Instant, Schedule};

/// The floor a task object saves. The writer leaves the key out when it equals the fallback.
pub(super) enum Saved {
    /// No `floor` key: a document saved before the floor was kept.
    Missing,
    /// The key's value, `None` for `null`.
    Floor(Option<Instant>),
}

pub(super) fn read_floor(task: &Value) -> Field<Saved> {
    let Some(value) = task.get("floor") else {
        return Ok(Saved::Missing);
    };
    opt(value, |v| int(v, "floor")).map(Saved::Floor)
}

/// Put each task's floor in `meta`. A task saved with none gets [`fallback_floor`]: right when one
/// project holds every task (an MS Project file, a one-project XER), and wrong for the later
/// projects of a several-project XER, whose floors the writer therefore keeps.
pub(super) fn install(meta: &mut [TaskMeta], saved: &[Saved], schedule: &Schedule) {
    let earliest = fallback_floor(schedule.tasks());
    for (task, saved) in meta.iter_mut().zip(saved) {
        task.floor = match saved {
            Saved::Missing => earliest,
            Saved::Floor(floor) => *floor,
        };
    }
}
