//! A schedule just read, fitted to what the host stores (gantt#2090, decision 0022, rule 8.3), with one note per kind of fit.

use super::capabilities::Capabilities;
use super::capabilities::values::{STEPS, Stores, held, kinds, whole_day};
use crate::constraint::constraint_of;
use crate::edit::day_minutes;
use crate::meta::TaskMeta;
use crate::texts::{self as t, Texts};
use engine::{Calendar, DAY, Instant, Limit, Pin, Schedule, TaskId};

/// Fit `schedule` to the profile `profile` (`Session::set_capabilities`'s JSON): a task's later date limits past the number
/// it may hold are dropped, a duration goes to the nearest whole step and a date with a time of day to the start of its day.
/// A summary is skipped. Adds a line to `notes` per kind of fit; returns whether anything changed, so the caller solves again.
///
/// # Errors
/// A message for text that is not a profile, or for a fitted value the schedule refuses.
pub fn conform(
    schedule: &mut Schedule,
    meta: &[TaskMeta],
    notes: &mut Vec<String>,
    profile: &str,
) -> Result<bool, String> {
    let stores = Capabilities::read(profile)?.stores;
    let mut summary = vec![false; schedule.tasks().len()];
    for parent in meta.iter().filter_map(|m| m.parent) {
        summary[parent.0 as usize] = true;
    }
    let (mut counts, mut changes) = ([0usize; 3], Vec::new());
    for (id, task) in (0..).map(TaskId).zip(schedule.tasks()) {
        if task.deleted || summary[id.0 as usize] {
            continue;
        }
        let calendar = schedule.calendar_of(id);
        let floor = meta.get(id.0 as usize).and_then(|m| m.floor);
        let (snet, limit, [cut, timed]) =
            fit_limits(stores, calendar, floor, task.snet, task.limit);
        let duration = stores.snap_length(task.duration, || day_minutes(calendar));
        for (count, did) in counts
            .iter_mut()
            .zip([cut, duration != task.duration, timed])
        {
            *count += usize::from(did);
        }
        if (snet, limit, duration) != (task.snet, task.limit, task.duration) {
            changes.push((id, snet, limit, duration));
        }
    }
    for (id, snet, limit, duration) in changes {
        let set = schedule.set_snet(id, snet);
        set.and_then(|()| schedule.set_limit(id, limit))
            .and_then(|()| schedule.set_duration(id, duration))
            .map_err(|e| e.to_string())?;
    }
    let said = [
        (&t::CONFORM_LIMITS_ONE, &t::CONFORM_LIMITS_MANY),
        (&t::CONFORM_DURATIONS_ONE, &t::CONFORM_DURATIONS_MANY),
        (&t::CONFORM_DATES_ONE, &t::CONFORM_DATES_MANY),
    ];
    for (((one, many), count), unit) in
        said.into_iter()
            .zip(counts)
            .zip(["", STEPS[stores.steps[0]], ""])
    {
        let total = count.to_string();
        let holes = [("count", total.as_str()), ("unit", unit)];
        if count > 0 {
            notes.push(Texts::default().say_by(count == 1, one, many, &holes));
        }
    }
    Ok(counts.iter().any(|&count| count > 0))
}

/// What a task's start constraint `snet` and `limit` become: the later limits past the profile's number dropped, then each
/// date with a time of day moved to the start of its day; and whether it dropped any, and whether it moved any.
fn fit_limits(
    stores: Stores,
    calendar: &Calendar,
    floor: Option<Instant>,
    snet: Option<Instant>,
    limit: Limit,
) -> (Option<Instant>, Limit, [bool; 2]) {
    let bits = kinds(&held(constraint_of(snet, floor), &limit), stores.not_kinds);
    let mut keep = bits;
    while stores.per_task.is_some_and(|most| keep.count_ones() > most) {
        keep &= !(1 << keep.ilog2());
    }
    let cut = bits & !keep;
    let gone = cut | ((cut >> 7 & 1) * 0b11) | ((cut >> 8 & 1) * 0b1100);
    let dropped = |bit: u16| gone >> bit & 1 == 1;
    let snet = if dropped(0) { floor } else { snet };
    let limit = Limit {
        snlt: limit.snlt.filter(|_| !dropped(1)),
        fnet: limit.fnet.filter(|_| !dropped(2)),
        fnlt: limit.fnlt.filter(|_| !dropped(3)),
        pin: limit.pin.filter(|_| !dropped(4) && !dropped(5)),
        alap: limit.alap && !dropped(6),
        ..limit
    };
    let day = |finish: bool, at: Instant| {
        let timed = stores.whole_days && !whole_day(calendar, finish, at);
        if timed { at - at.rem_euclid(DAY) } else { at }
    };
    let moved = Limit {
        snlt: limit.snlt.map(|at| day(false, at)),
        fnet: limit.fnet.map(|at| day(true, at)),
        fnlt: limit.fnlt.map(|at| day(true, at)),
        pin: limit.pin.map(|pin| match pin {
            Pin::Start(at) => Pin::Start(day(false, at)),
            Pin::Finish(at) => Pin::Finish(day(true, at)),
        }),
        ..limit
    };
    let start = constraint_of(snet, floor).map(|at| day(false, at)).or(snet);
    (start, moved, [cut != 0, (start, moved) != (snet, limit)])
}
