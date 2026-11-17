//! The calendars, tasks and links of a document: each one read from its JSON value, by the
//! fields in layout order, so the first value that cannot be read is the one named.

use super::super::fields::{
    Field, fields, fields_allowing, int, list, opt, small, str_of, text_of, within,
};
use super::super::json_read::Value;
use super::limit::read_limit;
use crate::meta::TaskMeta;
use engine::{
    Calendar, CalendarId, DayPeriod, Dependency, LagCalendar, LinkType, Schedule, ScheduleError,
    Task, TaskId,
};
use import::ActivityCode;

const TASK: [&str; 13] = [
    "duration",
    "calendar",
    "snet",
    "actual_start",
    "actual_finish",
    "remaining",
    "name",
    "parent",
    "outline_level",
    "code",
    "wbs_code",
    "activity_codes",
    "progress",
];

fn periods(value: &Value) -> Field<Vec<DayPeriod>> {
    let bad = || "expected [[from, to], ...] in minutes of the day".to_string();
    let mut out = Vec::new();
    for pair in value.as_array().ok_or_else(bad)? {
        match pair.as_array().ok_or_else(bad)? {
            [from, to] => out.push((
                small(from, "period").map_err(|_| bad())?,
                small(to, "period").map_err(|_| bad())?,
            )),
            _ => return Err(bad()),
        }
    }
    Ok(out)
}

/// The calendar and its host key.
pub(super) fn build_calendar(value: &Value) -> Field<(Calendar, Option<String>)> {
    let [week, exceptions] = fields_allowing(value, &["week", "exceptions"], &["key"])?;
    let days = list(week, "week")?;
    if days.len() != 7 {
        return Err(format!("week: expected 7 days, found {}", days.len()));
    }
    let mut pattern: [Vec<DayPeriod>; 7] = Default::default();
    for (i, day) in days.iter().enumerate() {
        pattern[i] = periods(day).map_err(|e| within(&format!("week[{i}]"), &e))?;
    }
    let mut extra = Vec::new();
    for (i, entry) in list(exceptions, "exceptions")?.iter().enumerate() {
        let at = |e: String| within(&format!("exceptions[{i}]"), &e);
        match list(entry, "exceptions").map_err(at)? {
            [day, periods_of_day] => extra.push((
                int(day, "day").map_err(at)?,
                periods(periods_of_day).map_err(at)?,
            )),
            _ => return Err(format!("exceptions[{i}]: expected [day, periods]")),
        }
    }
    let calendar = Calendar::with_exceptions(pattern, &extra).map_err(|e| e.to_string())?;
    Ok((calendar, key_of(value)?.map(str::to_string)))
}

pub(super) fn lag_calendar(value: &Value) -> Field<LagCalendar> {
    match value {
        Value::Str(s) if s == "successor" => Ok(LagCalendar::Successor),
        Value::Str(s) if s == "predecessor" => Ok(LagCalendar::Predecessor),
        Value::Str(s) if s == "24h" => Ok(LagCalendar::TwentyFourHour),
        Value::Object(_) => {
            let [id] = fields(value, &["calendar"])?;
            Ok(LagCalendar::Fixed(CalendarId(small(id, "calendar")?)))
        }
        other => Err(format!(
            "expected \"successor\", \"predecessor\", \"24h\" or {{\"calendar\": n}}, found {}",
            other.describe()
        )),
    }
}

pub(super) fn read_task(value: &Value) -> Field<(Task, TaskMeta, u8)> {
    let [
        duration,
        calendar,
        snet,
        start,
        finish,
        remaining,
        name,
        parent,
        level,
        code,
        wbs_code,
        codes,
        progress,
    ] = fields_allowing(value, &TASK, &["floor", "limit", "color", "label", "key"])?;
    let parent: Option<u32> = opt(parent, |v| small(v, "parent"))?;
    let percent: u8 = small(progress, "progress")?;
    if percent > 100 {
        return Err(format!("progress: {percent} is over 100"));
    }
    let mut activity_codes = Vec::new();
    for entry in list(codes, "activity_codes")? {
        match list(entry, "activity_codes")? {
            [code_type, value] => activity_codes.push(ActivityCode {
                code_type: text_of(code_type, "activity_codes")?,
                value: text_of(value, "activity_codes")?,
            }),
            _ => return Err("activity_codes: expected [type, value]".to_string()),
        }
    }
    let task = Task {
        duration: int(duration, "duration")?,
        calendar: CalendarId(small(calendar, "calendar")?),
        snet: opt(snet, |v| int(v, "snet"))?,
        limit: read_limit(value)?,
        deleted: false,
        actual_start: opt(start, |v| int(v, "actual_start"))?,
        actual_finish: opt(finish, |v| int(v, "actual_finish"))?,
        remaining: opt(remaining, |v| int(v, "remaining"))?,
    };
    let meta = TaskMeta {
        name: text_of(name, "name")?,
        parent: parent.map(TaskId),
        outline_level: small(level, "outline_level")?,
        code: opt(code, |v| text_of(v, "code"))?,
        activity_codes,
        wbs_code: opt(wbs_code, |v| text_of(v, "wbs_code"))?,
        uid: None,
        floor: None,
        color: value.get("color").map(color_of).transpose()?,
        label: value
            .get("label")
            .map(|v| text_of(v, "label"))
            .transpose()?,
        key: key_of(value)?.map(str::to_string),
    };
    Ok((task, meta, percent))
}

/// An opaque color from `"#rrggbb"`, hex digits in either case.
fn color_of(value: &Value) -> Field<[u8; 4]> {
    let bad = || format!("color: expected \"#rrggbb\", found {}", value.describe());
    let hex = value
        .as_str()
        .and_then(|s| s.strip_prefix('#'))
        .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(bad)?;
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| bad());
    Ok([byte(0)?, byte(2)?, byte(4)?, 255])
}

/// Add the link to `schedule`; its host key.
pub(super) fn add_link<'a>(schedule: &mut Schedule, value: &'a Value) -> Field<Option<&'a str>> {
    let [pred, succ, kind, lag, lag_over] = fields_allowing(
        value,
        &["pred", "succ", "kind", "lag", "lag_calendar"],
        &["key"],
    )?;
    let kind = match kind.as_str() {
        Some("FS") => LinkType::FS,
        Some("SS") => LinkType::SS,
        Some("FF") => LinkType::FF,
        Some("SF") => LinkType::SF,
        _ => {
            return Err(format!(
                "kind: expected \"FS\", \"SS\", \"FF\" or \"SF\", found {}",
                kind.describe()
            ));
        }
    };
    let dependency = Dependency {
        pred: TaskId(small(pred, "pred")?),
        succ: TaskId(small(succ, "succ")?),
        kind,
        lag: int(lag, "lag")?,
    };
    let over = opt(lag_over, lag_calendar).map_err(|e| within("lag_calendar", &e))?;
    // The schedule is the one that knows the task count: it refuses the predecessor first.
    let dep = schedule.add_dependency(dependency).map_err(|e| match e {
        ScheduleError::UnknownTask(task) => {
            let end = if task == dependency.pred {
                "pred"
            } else {
                "succ"
            };
            format!("{end}: no task {}", task.0)
        }
        other => other.to_string(),
    })?;
    if over.is_some() {
        schedule
            .set_dependency_lag_calendar(dep, over)
            .map_err(|e| format!("lag_calendar: {e}"))?;
    }
    key_of(value)
}

/// The `key` member, when there is one: text, and not empty, so that "no key" has one spelling.
fn key_of<'a>(value: &'a Value) -> Field<Option<&'a str>> {
    let key = value.get("key").map(|v| str_of(v, "key")).transpose()?;
    match key {
        Some("") => Err("key: is empty".to_string()),
        other => Ok(other),
    }
}
