//! The `options` key of a document (decision 0018): the schedule options, as an object that holds
//! only the members that differ from their defaults. An unknown member or value is refused.

use super::{Field, within};
use crate::document::json_read::Value;
use engine::{ScheduleOptions, StartedLateStart, UnstartedRemaining};

/// The options a document saves; no `options` key is the defaults.
pub(super) fn read_options(root: &Value) -> Field<ScheduleOptions> {
    if matches!(root, Value::Object(m) if m.iter().filter(|(k, _)| k == "options").count() > 1) {
        return Err("options: given twice".to_string());
    }
    root.get("options").map_or_else(
        || Ok(ScheduleOptions::default()),
        |value| members(value).map_err(|e| within("options", &e)),
    )
}

fn members(value: &Value) -> Field<ScheduleOptions> {
    let Value::Object(members) = value else {
        return Err(format!("expected an object, found {}", value.describe()));
    };
    let mut options = ScheduleOptions::default();
    for (key, value) in members {
        if members.iter().filter(|(k, _)| k == key).count() > 1 {
            return Err(format!("{key}: given twice"));
        }
        let name = value.as_str().unwrap_or_default();
        let bad = || format!("{key}: unknown value {}", value.describe());
        match &**key {
            "startedLateStart" => {
                options.started_late_start = StartedLateStart::from_name(name).ok_or_else(bad)?;
            }
            "unstartedRemaining" => {
                options.unstarted_remaining =
                    UnstartedRemaining::from_name(name).ok_or_else(bad)?;
            }
            _ => return Err(format!("{key}: unknown member")),
        }
    }
    Ok(options)
}
