//! The task `limit` of a document (gantt#1875): the date limits beyond `snet`, as an object that
//! holds only the keys that are set.

use super::{Field, int, within};
use crate::document::json_read::Value;
use engine::{Limit, Milestone, Pin};

/// The limit a task object saves; no `limit` key is the default, a task with no limits.
pub(super) fn read_limit(task: &Value) -> Field<Limit> {
    task.get("limit").map_or_else(
        || Ok(Limit::default()),
        |value| members(value).map_err(|e| within("limit", &e)),
    )
}

fn members(value: &Value) -> Field<Limit> {
    let Value::Object(members) = value else {
        return Err(format!("expected an object, found {}", value.describe()));
    };
    let mut limit = Limit::default();
    for (key, value) in members {
        if members.iter().filter(|(k, _)| k == key).count() > 1 {
            return Err(format!("{key}: given twice"));
        }
        let pin = |make: fn(i64) -> Pin, pinned: Option<Pin>| match pinned {
            Some(_) => Err(format!("{key}: only one pin key may be given")),
            None => int(value, key).map(|at| Some(make(at))),
        };
        match &**key {
            "snlt" => limit.snlt = Some(int(value, key)?),
            "fnet" => limit.fnet = Some(int(value, key)?),
            "fnlt" => limit.fnlt = Some(int(value, key)?),
            "resume" => limit.resume = Some(int(value, key)?),
            "pin_start" => limit.pin = pin(Pin::Start, limit.pin)?,
            "pin_finish" => limit.pin = pin(Pin::Finish, limit.pin)?,
            "milestone" => {
                limit.milestone = Some(match value.as_str() {
                    Some("start") => Milestone::Start,
                    Some("finish") => Milestone::Finish,
                    _ => return Err(format!("{key}: expected \"start\" or \"finish\"")),
                });
            }
            "alap" => match value {
                Value::Bool(alap) => limit.alap = *alap,
                other => {
                    return Err(format!(
                        "{key}: expected a boolean, found {}",
                        other.describe()
                    ));
                }
            },
            _ => return Err(format!("{key}: unknown key")),
        }
    }
    Ok(limit)
}
