//! What a Save as XER leaves out of a session, counted before the file is written.

use crate::Session;
use crate::document::live_order;

/// One line for each of the activity codes, baselines and deadlines the live tasks hold, with the
/// number of tasks that hold it; none for a kind the schedule lacks. The writer carries none of
/// them (docs/IMPORT.md, "What an open keeps and drops").
pub fn xer_left_out(session: &Session) -> Vec<String> {
    let tasks = live_order(session);
    let count = |has: &dyn Fn(engine::TaskId) -> bool| tasks.iter().filter(|&&t| has(t)).count();
    let kinds = [
        (
            "activity codes",
            count(&|t| !session.meta()[t.0 as usize].activity_codes.is_empty()),
        ),
        (
            "baselines",
            count(&|t| session.baseline().is_some_and(|b| b.task(t).is_some())),
        ),
        ("deadlines", count(&|t| session.deadline(t).is_some())),
    ];
    (kinds.into_iter())
        .filter(|&(_, n)| n > 0)
        .map(|(what, n)| {
            let task = if n == 1 { "task" } else { "tasks" };
            format!("the {what} on {n} {task} are not written")
        })
        .collect()
}
