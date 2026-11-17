//! An XER activity's actual dates and remaining duration, as the engine takes them (decision
//! 0008): a started activity keeps its actual start and finishes its remaining duration after the
//! data date; a finished one is fixed at its actuals.

use engine::{Instant, Minutes};
use xer::{StatusCode, TaskRow};

use crate::notes::Notes;
use crate::range;
use crate::xer_import::instant_from_origin;

/// What the file states of an activity's progress.
pub(super) struct Actuals {
    pub(super) start: Option<Instant>,
    pub(super) finish: Option<Instant>,
    pub(super) remaining: Option<Minutes>,
}

impl Actuals {
    /// The actuals of an Active or Complete activity; every other activity has none. The finish
    /// is read for a Complete activity only, the start only when the file states one, and the remaining duration only with a start (the
    /// engine ignores it otherwise).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the filter leaves a finite value that is not negative; `as` saturates at the top of the range instead of wrapping, and the file states no cap on a remaining duration"
    )]
    pub(super) fn of(activity: &TaskRow, origin_day: i64, label: &str, notes: &mut Notes) -> Self {
        let status = activity.status_code.as_ref();
        let started = matches!(status, Some(StatusCode::Active | StatusCode::Complete));
        let complete = matches!(status, Some(StatusCode::Complete));
        let instant = |minutes: Option<i64>, notes: &mut Notes| {
            minutes
                .and_then(|minutes| instant_from_origin(minutes, origin_day))
                .and_then(|at| range::date(at, label, notes))
        };
        let start = instant(activity.act_start_date.filter(|_| started), notes);
        let finish = instant(activity.act_end_date.filter(|_| complete), notes);
        let remaining = activity
            .remain_drtn_hr_cnt
            .map(|hours| hours * 60.0)
            .filter(|minutes| start.is_some() && minutes.is_finite() && *minutes >= 0.0)
            .map(|minutes| minutes.round() as Minutes)
            .and_then(|minutes| range::remaining(minutes, label, notes));
        Self {
            start,
            finish,
            remaining,
        }
    }
}
