//! The schedule checks as one JSON report, for a Checks panel. It joins the link counts
//! ([`Checks`]) and the float, duration, date and resource counts ([`ScheduleChecks`]) and judges nothing:
//! no pass or fail, no threshold beyond the 44 days the counts already use.

use super::{Checks, baseline::BaselineChecks, schedule::ScheduleChecks};
use crate::Session;

/// The counts for `session` as a JSON object, keys in this order: `missing_logic`, `open_starts`,
/// `open_ends`, `leads`, `lags`, `links`, `finish_to_start`, `pin_broken_links`, `high_float`,
/// `high_duration`, `hard_constraints`, `negative_float`, `invalid_dates`, `missing_resources`,
/// `missed`, `execution_index`. Every value is a count except
/// `invalid_dates`, which is `null` when the session has no data date, `missing_resources`, which is
/// `null` when the file names no resource (a 0 in either would read as a clean check that never ran), and
/// `execution_index`, the share of the tasks baselined to finish by the data date that have
/// finished, which is `null` when none was (no baseline, no data date, or none due yet).
pub fn json(session: &Session) -> String {
    let links = Checks::of(session);
    let schedule = ScheduleChecks::of(session);
    let baseline = BaselineChecks::of(session);
    let execution_index = baseline
        .execution_index()
        .map_or_else(|| "null".to_string(), |index| index.to_string());
    let invalid_dates = session.data_date().map_or_else(
        || "null".to_string(),
        |_| schedule.invalid_dates.to_string(),
    );
    let missing_resources = schedule
        .missing_resources
        .map_or_else(|| "null".to_string(), |count| count.to_string());
    format!(
        concat!(
            r#"{{"missing_logic":{},"open_starts":{},"open_ends":{},"leads":{},"lags":{},"#,
            r#""links":{},"finish_to_start":{},"pin_broken_links":{},"#,
            r#""high_float":{},"high_duration":{},"hard_constraints":{},"negative_float":{},"#,
            r#""invalid_dates":{},"missing_resources":{},"#,
            r#""missed":{},"execution_index":{}}}"#
        ),
        links.missing_logic,
        links.open_starts,
        links.open_ends,
        links.leads,
        links.lags,
        links.links,
        links.finish_to_start,
        links.pin_broken_links,
        schedule.high_float,
        schedule.high_duration,
        schedule.hard_constraints,
        schedule.negative_float,
        invalid_dates,
        missing_resources,
        baseline.missed_tasks,
        execution_index,
    )
}
