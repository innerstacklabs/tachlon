//! What a host says the scheduler computed (its `host*` members, tl#2087), set against the chart's own.

use super::{results::days, update::UpdateError};
use crate::{Session, document::json_read::Value, edit::EditError, json::quote};
use engine::Instant;

/// The `host*` members: two dates, the late two, two floats in working days, a flag. `differs` names each by
/// the rest (`lateStart`), as `results` does.
const NAMES: &str =
    "hostStart hostFinish hostLateStart hostLateFinish hostFloat hostFreeFloat hostCritical";

pub(super) enum Said {
    At(Instant),
    Days(f64),
    Flag(bool),
}

pub(super) fn is_host(name: &str) -> bool {
    NAMES.split(' ').any(|host| host == name)
}

impl Session {
    /// The place of the `host*` member `name` in `NAMES` and what `value` says, or `None` when it is no kind of
    /// value that member takes.
    pub(super) fn read_host(&self, name: &str, value: &Value) -> Option<(usize, Said)> {
        let kind = NAMES.split(' ').position(|host| host == name)?;
        let said = match (kind, value) {
            (0 | 2, _) => Said::At(self.read_date(value)?),
            (1 | 3, _) => Said::At(self.read_finish(value)?),
            (4 | 5, Value::Frac(days)) => Said::Days(*days),
            (4 | 5, Value::Int(n)) => Said::Days(f64::from(i32::try_from(*n).ok()?)),
            (_, Value::Bool(flag)) => Said::Flag(*flag),
            _ => return None,
        };
        Some((kind, said))
    }

    /// Each `host*` member of `records` that the chart computed otherwise, as JSON: a date to the minute (a day
    /// alone compares the day, a finish's day being its last minute's), a float in working days of the task's
    /// calendar to the half minute. The free floats are worked out only for a call that has one.
    pub(super) fn differences(
        &self,
        records: &[Value],
        planned: &[super::update::Written],
    ) -> Result<Vec<String>, UpdateError> {
        let wanted = records.iter().any(|r| r.get("hostFreeFloat").is_some());
        let free = (wanted.then(|| self.schedule.free_floats(&self.solution))).transpose();
        let free = free.map_err(|e| UpdateError::new(EditError::from(e).kind(), None, None))?;
        let mut found = vec![];
        for (record, ((task, _), row)) in planned.iter().zip(records).enumerate() {
            let (Value::Object(members), n) = (row, task.0 as usize) else {
                continue;
            };
            let (d, key) = (
                &self.solution.dates[n],
                row.get("key").and_then(Value::as_str),
            );
            let day = crate::edit::day_minutes(self.schedule.calendar_of(*task));
            for (name, value) in members {
                let Some((kind, said)) = self.read_host(name, value) else {
                    continue;
                };
                let text = value.as_str().unwrap_or_default();
                let shown = match said {
                    Said::At(host) => {
                        let late = self.schedule.late_start_shown(*task, d);
                        let chart = [d.es, d.ef, late, d.lf][kind];
                        let day_of = |t| self.origin.day_text(t - i64::from(kind % 2 == 1));
                        let timed = text.contains('T');
                        let shown = [day_of(chart), self.date_text(chart).replacen(' ', "T", 1)];
                        let differs = host != chart && (timed || day_of(host) != day_of(chart));
                        differs.then(|| [quote(text), quote(&shown[usize::from(timed)])])
                    }
                    Said::Days(host) => {
                        let free = free.as_ref().filter(|_| kind == 5);
                        let chart = days(free.map_or(d.float, |f| f[n]), day);
                        let off = (host - chart).abs() >= days(1, 2 * day);
                        off.then(|| [host.to_string(), chart.to_string()])
                    }
                    Said::Flag(host) => (host != d.is_critical())
                        .then(|| [host.to_string(), d.is_critical().to_string()]),
                };
                if let Some([host, chart]) = shown {
                    let member = format!("{}{}", name[4..5].to_lowercase(), &name[5..]);
                    let key = quote(key.unwrap_or_default());
                    found.push(format!(
                        r#"{{"record":{record},"key":{key},"member":"{member}","host":{host},"chart":{chart}}}"#
                    ));
                }
            }
        }
        Ok(found)
    }
}
