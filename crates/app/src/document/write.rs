//! [`write`]: renumber the live tasks in outline order, then emit every key in its fixed order.

use super::{Extras, FORMAT, VERSION, base64};
use crate::Session;
use crate::constraint::fallback_floor;
use crate::json::quote;
use engine::{
    Calendar, CalendarId, DEFAULT_STARTED_LATE_START, DEFAULT_UNSTARTED_REMAINING, DepId,
    LagCalendar, Limit, LinkType, Pin, ScheduleOptions, TaskId,
};
use std::fmt::Write;

/// The session as a schedule document (version 1): one JSON text, keys in a fixed order, no
/// insignificant whitespace. Only live tasks are written, numbered 0 to n-1 in outline order (id
/// order when the session has no outline); every task id in the text is a new id. A link that is
/// removed or has a deleted end is not written. The undo history, the selection, the row filter
/// and the viewport are not part of the document.
#[expect(clippy::too_many_lines, reason = "fields are written in layout order")]
pub fn write(session: &Session, extras: &Extras<'_>) -> String {
    let schedule = session.schedule();
    let keys = session.host_keys();
    let slots = schedule.tasks().len();
    let order = live_order(session);
    let outlined = session.outline().is_some();
    let mut remap: Vec<Option<u32>> = vec![None; slots];
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    for (new, old) in order.iter().enumerate() {
        remap[old.0 as usize] = Some(new as u32);
    }
    let new_id = |old: TaskId| remap.get(old.0 as usize).copied().flatten();

    let source_len = extras.source.map_or(0, |s| s.bytes.len() / 3 * 4 + 64);
    let mut out = String::with_capacity(order.len() * 260 + source_len + 1024);
    let _ = write!(out, "{{\"format\":\"{FORMAT}\",\"version\":{VERSION}");
    out.push_str(",\"name\":");
    out.push_str(&quote(extras.name));
    let _ = write!(out, ",\"origin\":{}", session.origin.day());
    out.push_str(",\"data_date\":");
    push_opt(&mut out, session.data_date());
    out.push_str(",\"lag_calendar\":");
    push_lag(&mut out, Some(schedule.lag_calendar()));

    out.push_str(",\"calendars\":[");
    for (i, calendar) in schedule.calendars().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a calendar index; Schedule::add_calendar casts the count to u32 with no check against u32::MAX, so past 2^32 calendars it wraps, as that id does"
        )]
        let id = CalendarId(i as u32);
        push_calendar(&mut out, calendar, keys.calendar_key(id));
    }

    out.push_str("],\"tasks\":[");
    let progress = session.timeline().progress();
    let fallback = fallback_floor(order.iter().map(|old| &schedule.tasks()[old.0 as usize]));
    for (i, &old) in order.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let task = &schedule.tasks()[old.0 as usize];
        let meta = &session.meta[old.0 as usize];
        let _ = write!(
            out,
            "{{\"duration\":{},\"calendar\":{}",
            task.duration, task.calendar.0
        );
        for (key, value) in [
            ("snet", task.snet),
            ("actual_start", task.actual_start),
            ("actual_finish", task.actual_finish),
            ("remaining", task.remaining),
        ] {
            let _ = write!(out, ",\"{key}\":");
            push_opt(&mut out, value);
        }
        // Only a floor `read` would not give back unasked: a task at the fallback adds no key, so a
        // document that never carried floors keeps its text. A task with no `snet` has no
        // constraint for its floor to qualify, so it adds none either.
        if task.snet.is_some() && meta.floor != fallback {
            out.push_str(",\"floor\":");
            push_opt(&mut out, meta.floor);
        }
        // Only the keys that are set, and none for a task with no limits.
        if task.limit != Limit::default() {
            push_limit(&mut out, &task.limit);
        }
        out.push_str(",\"name\":");
        out.push_str(&quote(&meta.name));
        // Only when set: a document with neither keeps its text, and a build before them reads it.
        if let Some(label) = &meta.label {
            out.push_str(",\"label\":");
            out.push_str(&quote(label));
        }
        if let Some([r, g, b, _]) = meta.color {
            let _ = write!(out, ",\"color\":\"#{r:02x}{g:02x}{b:02x}\"");
        }
        push_key(&mut out, meta.key.as_deref());
        // Parents describe an outline. A session with none (the stress fixture keeps parents for its row paths) writes
        // none: `read` builds an outline from them, and the stress links that touch those parents would be refused.
        let (parent, level) = if outlined {
            (meta.parent.and_then(new_id), meta.outline_level)
        } else {
            (None, 1)
        };
        out.push_str(",\"parent\":");
        push_opt(&mut out, parent);
        let _ = write!(out, ",\"outline_level\":{level}");
        out.push_str(",\"code\":");
        push_str_opt(&mut out, meta.code.as_deref());
        out.push_str(",\"wbs_code\":");
        push_str_opt(&mut out, meta.wbs_code.as_deref());
        out.push_str(",\"activity_codes\":[");
        for (j, code) in meta.activity_codes.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            let _ = write!(out, "[{},{}]", quote(&code.code_type), quote(&code.value));
        }
        let percent = progress.get(old.0 as usize).copied().unwrap_or(0);
        let _ = write!(out, "],\"progress\":{percent}}}");
    }

    out.push_str("],\"links\":[");
    let mut first = true;
    for (i, dep) in schedule.dependencies().iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a dependency index; Schedule::add_dependency casts the count to u32 with no check against u32::MAX, so past 2^32 links it wraps, as that id does"
        )]
        let id = DepId(i as u32);
        if !schedule.is_dependency_live(id) {
            continue;
        }
        let (Some(pred), Some(succ)) = (new_id(dep.pred), new_id(dep.succ)) else {
            continue;
        };
        if !first {
            out.push(',');
        }
        first = false;
        let kind = match dep.kind {
            LinkType::FS => "FS",
            LinkType::SS => "SS",
            LinkType::FF => "FF",
            LinkType::SF => "SF",
        };
        let _ = write!(
            out,
            "{{\"pred\":{pred},\"succ\":{succ},\"kind\":\"{kind}\",\"lag\":{},\"lag_calendar\":",
            dep.lag
        );
        push_lag(&mut out, schedule.dependency_lag_calendar(id));
        push_key(&mut out, keys.link_key(id));
        out.push('}');
    }

    out.push_str("],\"baseline\":");
    match session.baseline() {
        None => out.push_str("null"),
        Some(baseline) => {
            out.push_str("{\"data_date\":");
            push_opt(&mut out, baseline.data_date());
            let _ = write!(
                out,
                ",\"project_finish\":{},\"tasks\":[",
                baseline.project_finish()
            );
            for (i, &old) in order.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                match baseline.task(old) {
                    Some(t) => {
                        let _ = write!(out, "[{},{},", t.es, t.ef);
                        match t.float {
                            Some(float) => {
                                let _ = write!(out, "{float}]");
                            }
                            None => out.push_str("null]"),
                        }
                    }
                    None => out.push_str("null"),
                }
            }
            out.push_str("]}");
        }
    }

    out.push_str(",\"view\":{\"collapsed\":[");
    let mut first = true;
    for (new, &old) in order.iter().enumerate() {
        if session.is_collapsed(old) {
            if !first {
                out.push(',');
            }
            first = false;
            let _ = write!(out, "{new}");
        }
    }

    out.push_str("]},\"notes\":[");
    for (i, note) in extras.notes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&quote(note));
    }

    out.push_str("],\"source\":");
    match extras.source {
        None => out.push_str("null"),
        Some(source) => {
            let _ = write!(
                out,
                "{{\"format\":\"{}\",\"base64\":\"",
                source.format.name()
            );
            base64::encode_into(source.bytes, &mut out);
            out.push_str("\"}");
        }
    }
    push_options(&mut out, schedule.options());
    if let Some(at) = schedule.finish_by() {
        let _ = write!(out, ",\"must_finish_by\":{at}");
    }
    out.push('}');
    out
}

/// The live tasks in outline order (id order when the session has no outline): the order both the
/// document and the MS Project file list them in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the task slot count; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
)]
pub(crate) fn live_order(session: &Session) -> Vec<TaskId> {
    match session.outline() {
        Some(outline) => outline.preorder().to_vec(),
        None => (0..session.schedule().tasks().len() as u32)
            .map(TaskId)
            .filter(|&t| !session.schedule().is_deleted(t))
            .collect(),
    }
}

/// The `options` key, with the members that differ from their defaults and no key when none does.
fn push_options(out: &mut String, options: ScheduleOptions) {
    let mut sep = ",\"options\":{";
    for (key, name, default) in [
        (
            "startedLateStart",
            options.started_late_start.name(),
            options.started_late_start == DEFAULT_STARTED_LATE_START,
        ),
        (
            "unstartedRemaining",
            options.unstarted_remaining.name(),
            options.unstarted_remaining == DEFAULT_UNSTARTED_REMAINING,
        ),
    ] {
        if !default {
            let _ = write!(out, "{sep}\"{key}\":\"{name}\"");
            sep = ",";
        }
    }
    if sep == "," {
        out.push('}');
    }
}

fn push_limit(out: &mut String, limit: &Limit) {
    let (start, finish) = match limit.pin {
        Some(Pin::Start(at)) => (Some(at), None),
        Some(Pin::Finish(at)) => (None, Some(at)),
        None => (None, None),
    };
    let mut sep = ",\"limit\":{";
    for (key, value) in [
        ("snlt", limit.snlt),
        ("fnet", limit.fnet),
        ("fnlt", limit.fnlt),
        ("pin_start", start),
        ("pin_finish", finish),
        ("resume", limit.resume),
    ] {
        if let Some(at) = value {
            let _ = write!(out, "{sep}\"{key}\":{at}");
            sep = ",";
        }
    }
    if limit.alap {
        let _ = write!(out, "{sep}\"alap\":true");
    }
    out.push('}');
}

fn push_opt<T: std::fmt::Display>(out: &mut String, value: Option<T>) {
    match value {
        Some(v) => {
            let _ = write!(out, "{v}");
        }
        None => out.push_str("null"),
    }
}

fn push_str_opt(out: &mut String, value: Option<&str>) {
    match value {
        Some(v) => out.push_str(&quote(v)),
        None => out.push_str("null"),
    }
}

fn push_lag(out: &mut String, lag: Option<LagCalendar>) {
    match lag {
        None => out.push_str("null"),
        Some(LagCalendar::Successor) => out.push_str("\"successor\""),
        Some(LagCalendar::Predecessor) => out.push_str("\"predecessor\""),
        Some(LagCalendar::TwentyFourHour) => out.push_str("\"24h\""),
        Some(LagCalendar::Fixed(id)) => {
            let _ = write!(out, "{{\"calendar\":{}}}", id.0);
        }
    }
}

fn push_periods(out: &mut String, periods: &[(u16, u16)]) {
    out.push('[');
    for (i, (from, to)) in periods.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "[{from},{to}]");
    }
    out.push(']');
}

/// The `key` member, only when there is a key.
fn push_key(out: &mut String, key: Option<&str>) {
    if let Some(key) = key {
        out.push_str(",\"key\":");
        out.push_str(&quote(key));
    }
}

fn push_calendar(out: &mut String, calendar: &Calendar, key: Option<&str>) {
    out.push_str("{\"week\":[");
    for (i, day) in calendar.week().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_periods(out, day);
    }
    out.push_str("],\"exceptions\":[");
    for (i, (day, periods)) in calendar.exceptions().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "[{day},");
        push_periods(out, periods);
        out.push(']');
    }
    out.push(']');
    push_key(out, key);
    out.push('}');
}
