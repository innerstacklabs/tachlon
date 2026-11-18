//! Date limits for a generated schedule.

use super::Rng;
use crate::calendar::Instant;
use crate::model::{Limit, Pin, Schedule, TaskId};

/// Give about a fraction `share` of the tasks date limits, drawn from `seed` on a stream of
/// their own: each limit of a chosen task is set with probability 0.4, and a pin (start or
/// finish, even odds) with probability 0.2, to an instant in `0..=span`; `alap` is set with
/// probability 0.15.
///
/// # Panics
///
/// Panics if the schedule has more than `u32::MAX` tasks.
pub fn add_limits(s: &mut Schedule, seed: u64, share: f64, span: i64) {
    let mut rng = Rng::new(seed ^ 0x11a7_117e);
    let n = u32::try_from(s.tasks().len()).expect("the task count fits a TaskId");
    for i in 0..n {
        if !rng.chance(share) {
            continue;
        }
        let mut draw = || -> Option<Instant> { rng.chance(0.4).then(|| rng.range(0, span)) };
        let (late_start, fin_floor, late_fin) = (draw(), draw(), draw());
        let pin = rng.chance(0.2).then(|| {
            let at = rng.range(0, span);
            if rng.chance(0.5) {
                Pin::Start(at)
            } else {
                Pin::Finish(at)
            }
        });
        let limit = Limit {
            snlt: late_start,
            fnet: fin_floor,
            fnlt: late_fin,
            pin,
            alap: rng.chance(0.15),
            resume: None,
            milestone: None,
        };
        s.set_limit(TaskId(i), limit).expect("the task exists");
    }
}
