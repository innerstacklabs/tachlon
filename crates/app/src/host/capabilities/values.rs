//! What a host's database stores (decision 0022, rule 8.2): which date limits, how many for a
//! task, whole minutes, hours or days for a duration or lag, and dates with or without a time.

use crate::Session;
use crate::constraint::constraint_of;
use crate::document::json_read::Value;
use crate::edit::{Edit, day_minutes};
use engine::{Calendar, DAY, Instant, Limit, Pin, TaskId};

/// The date limits a host can name, one bit each in this order; `snet` is a bar move's.
const KINDS: [&str; 7] = ["snet", "snlt", "fnet", "fnlt", "mso", "mfo", "alap"];
const STEPS: [&str; 3] = ["minutes", "hours", "days"];

/// What the host stores beyond the groups it turned off; the default stores everything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Stores {
    not_kinds: u8,
    per_task: Option<u32>,
    /// Places in [`STEPS`]: a duration's, then a lag's.
    steps: [usize; 2],
    whole_days: bool,
}

/// The place in `words` of the string at `key` of `root`; 0 when `key` is absent.
fn word(root: &Value, key: &str, words: &[&str]) -> Result<usize, String> {
    let Some(value) = root.get(key) else {
        return Ok(0);
    };
    let at = words.iter().position(|w| value.as_str() == Some(w));
    at.ok_or_else(|| format!("{key}: expected one of {}", words.join(", ")))
}

impl Stores {
    pub(super) fn read(root: &Value) -> Result<Self, String> {
        let mut stores = Self::default();
        if let Some(kinds) = root.get("kinds") {
            let said = "kinds: expected a list of snet, snlt, fnet, fnlt, mso, mfo, alap";
            let mut allowed = 0u8;
            for kind in kinds.as_array().ok_or(said)? {
                let at = KINDS.iter().position(|k| kind.as_str() == Some(k));
                allowed |= 1 << at.ok_or(said)?;
            }
            stores.not_kinds = !allowed & 0x7f;
        }
        if let Some(most) = root.get("perTask") {
            let most = most.as_int().and_then(|n| u32::try_from(n).ok());
            stores.per_task = Some(most.ok_or("perTask: expected a whole number, 0 or more")?);
        }
        stores.steps = [
            word(root, "durationStep", &STEPS)?,
            word(root, "lagStep", &STEPS)?,
        ];
        stores.whole_days = word(root, "dates", &["minute", "day"])? == 1;
        Ok(stores)
    }
}

/// What `snet` and `limit` hold, in the order of [`KINDS`]; an `alap` holds 0.
fn held(snet: Option<Instant>, limit: &Limit) -> [Option<Instant>; 7] {
    let pin = |start| match limit.pin {
        Some(Pin::Start(at)) if start => Some(at),
        Some(Pin::Finish(at)) if !start => Some(at),
        _ => None,
    };
    [
        snet,
        limit.snlt,
        limit.fnet,
        limit.fnlt,
        pin(true),
        pin(false),
        limit.alap.then_some(0),
    ]
}

/// What `limit` holds after `new` changed it from `now`: each date `new` changed is `new`'s.
fn merged(kept: Limit, now: Limit, new: Limit) -> Limit {
    let pick = |kept, now, new| if new == now { kept } else { new };
    Limit {
        snlt: pick(kept.snlt, now.snlt, new.snlt),
        fnet: pick(kept.fnet, now.fnet, new.fnet),
        fnlt: pick(kept.fnlt, now.fnlt, new.fnlt),
        ..new
    }
}

/// Is `at` midnight, or its calendar's opening (a start) or closing (a finish) of a working day?
fn whole_day(calendar: &Calendar, finish: bool, at: Instant) -> bool {
    let closing = calendar.is_period_end(at) && calendar.next_working(at) / DAY > (at - 1) / DAY;
    at % DAY == 0 || (finish && closing) || (!finish && calendar.day_start_at(at) == at)
}

/// The kinds among `held`, one bit each.
fn kinds(held: &[Option<Instant>]) -> u8 {
    held.iter()
        .enumerate()
        .fold(0, |bits, (i, at)| bits | u8::from(at.is_some()) << i)
}

impl Session {
    /// What the host cannot store in the first of `edits` that has such a value, as the end of
    /// "The host stores ...".
    pub(super) fn unstorable(&self, s: Stores, edits: &[Edit]) -> Option<String> {
        let mut folded = std::collections::BTreeMap::new();
        for edit in edits {
            let (task, snet, new) = match edit {
                Edit::Move { task, snet } => (task, Some(*snet), None),
                Edit::SetLimit { task, limit } => (task, None, Some(*limit)),
                _ => continue,
            };
            let Some(now) = self.live_task(task.0) else {
                continue;
            };
            let (kept_snet, kept) = folded.entry(task.0).or_insert((now.snet, now.limit));
            *kept_snet = snet.unwrap_or(*kept_snet);
            *kept = new.map_or(*kept, |new| merged(*kept, now.limit, new));
        }
        let limits = folded
            .iter()
            .find_map(|(&task, &(snet, limit))| self.limits_refusal(s, TaskId(task), snet, limit));
        let others = edits.iter().find_map(|edit| self.not_stored(s, edit));
        others.or(limits)
    }

    fn not_stored(&self, s: Stores, edit: &Edit) -> Option<String> {
        let [duration_step, lag_step] = s.steps;
        let live = |task: &TaskId| self.is_live(*task);
        let (step, minutes, day, what) = match edit {
            Edit::SetDuration { task, minutes } if live(task) => {
                let day = day_minutes(self.schedule.calendar_of(*task));
                (duration_step, *minutes, day, "durations")
            }
            Edit::Insert { place, .. } => {
                let calendar = self.insert_calendar(*place);
                let day = day_minutes(&self.schedule.calendars()[calendar.0 as usize]);
                (duration_step, day, day, "durations")
            }
            Edit::AddLink {
                pred, succ, lag, ..
            } if live(pred) && live(succ) => (
                lag_step,
                *lag,
                self.pair_day_minutes(*pred, *succ, None),
                "lags",
            ),
            Edit::SetLink { dep, lag, .. } if self.schedule.is_dependency_live(*dep) => {
                (lag_step, *lag, self.lag_day_minutes(*dep), "lags")
            }
            _ => return None,
        };
        let unit = [1, 60, day][step];
        (minutes % unit != 0).then(|| format!("{what} in whole {}", STEPS[step]))
    }

    /// The refusal for `task` holding `snet` and `limit`: a kind, a count or a time of day not stored.
    fn limits_refusal(
        &self,
        s: Stores,
        task: TaskId,
        snet: Option<Instant>,
        limit: Limit,
    ) -> Option<String> {
        let floor = self.meta.get(task.0 as usize).and_then(|m| m.floor);
        let now = held(constraint_of(snet, floor), &limit);
        let bits = kinds(&now);
        if bits & s.not_kinds != 0 {
            let kind = KINDS[(bits & s.not_kinds).trailing_zeros() as usize];
            return Some(format!("no {kind} limit"));
        }
        if let Some(most) = s.per_task.filter(|&most| bits.count_ones() > most) {
            return Some(format!("date limits: at most {most} for a task"));
        }
        let calendar = self.schedule.calendar_of(task);
        let timed = now.iter().enumerate().any(|(kind, at)| {
            at.is_some_and(|at| !whole_day(calendar, matches!(kind, 2 | 3 | 5), at))
        });
        (s.whole_days && timed).then(|| "dates without a time of day".into())
    }
}
