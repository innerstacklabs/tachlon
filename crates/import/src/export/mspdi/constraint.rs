//! The one `ConstraintType` a task carries in an MS Project file (gantt#1875). A task here can hold
//! a start-no-earlier-than and the engine's limits together, as a P6 activity with a soft Start On
//! or Finish On does (a no-earlier-than and a no-later-than half). The file holds one.

use engine::{Limit, Pin};

/// The constraint a task is written with: its `ConstraintType` code, its name in a scheduler's
/// words, and its `ConstraintDate` as an instant (as late as possible has none).
pub(super) struct Written {
    pub(super) code: &'static str,
    pub(super) name: &'static str,
    pub(super) at: Option<i64>,
}

/// The constraint to write and the names of the others the task holds, which the file cannot.
/// The first of these wins: a pin (2, 3), start-no-earlier-than (4), finish-no-earlier-than (6),
/// start-no-later-than (5), finish-no-later-than (7), as late as possible (1). So a soft Start On
/// or Finish On writes its no-earlier-than half and leaves the other out.
pub(super) fn choose(snet: Option<i64>, limit: &Limit) -> (Option<Written>, Vec<&'static str>) {
    let pin = match limit.pin {
        Some(Pin::Start(at)) => Some(("2", "must start on", Some(at))),
        Some(Pin::Finish(at)) => Some(("3", "must finish on", Some(at))),
        None => None,
    };
    let mut all = [
        pin,
        snet.map(|at| ("4", "start no earlier than", Some(at))),
        limit
            .fnet
            .map(|at| ("6", "finish no earlier than", Some(at))),
        limit.snlt.map(|at| ("5", "start no later than", Some(at))),
        limit.fnlt.map(|at| ("7", "finish no later than", Some(at))),
        limit.alap.then_some(("1", "as late as possible", None)),
    ]
    .into_iter()
    .flatten();
    let written = all
        .next()
        .map(|(code, name, at)| Written { code, name, at });
    (written, all.map(|(_, name, _)| name).collect())
}
