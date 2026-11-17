//! Interprets a scanned calendar blob into weeks and exception days.

use super::scan::{Arena, Node};
use super::text::{attr, short};
use super::{CalendarData, CalendarException, Period, Week};
use crate::Warning;

pub(super) fn interpret(
    arena: &Arena<'_>,
    clndr_id: u32,
    line: usize,
    warnings: &mut Vec<Warning>,
) -> CalendarData {
    let mut week = None;
    let mut seen_days = [false; 7];
    let mut exceptions = Vec::new();

    for root_index in &arena.roots {
        let Some(root) = arena.nodes.get(*root_index) else {
            continue;
        };
        if root.name != "CalendarData" {
            unknown(warnings, clndr_id, line, root.name);
            continue;
        }

        for child_index in &root.children {
            let Some(child) = arena.nodes.get(*child_index) else {
                continue;
            };
            match child.name {
                "DaysOfWeek" => {
                    if week.is_none() {
                        week = Some(empty_week());
                    }
                    interpret_days(
                        arena,
                        child,
                        clndr_id,
                        line,
                        &mut week,
                        &mut seen_days,
                        warnings,
                    );
                }
                "Exceptions" => {
                    interpret_exceptions(arena, child, clndr_id, line, &mut exceptions, warnings);
                }
                _ => unknown(warnings, clndr_id, line, child.name),
            }
        }
    }

    CalendarData { week, exceptions }
}

fn empty_week() -> Week {
    std::array::from_fn(|_| Vec::new())
}

fn interpret_days(
    arena: &Arena<'_>,
    days_node: &Node<'_>,
    clndr_id: u32,
    line: usize,
    week: &mut Option<Week>,
    seen_days: &mut [bool; 7],
    warnings: &mut Vec<Warning>,
) {
    for day_index in &days_node.children {
        let Some(day) = arena.nodes.get(*day_index) else {
            continue;
        };
        if !is_numeric_name(day.name) {
            unknown(warnings, clndr_id, line, day.name);
            continue;
        }
        let Ok(day_number) = day.name.parse::<usize>() else {
            malformed(
                warnings,
                clndr_id,
                line,
                format!("day number {:?} is outside 1..=7", short(day.name)),
            );
            continue;
        };
        let Some(slot) = day_number.checked_sub(1).filter(|slot| *slot < 7) else {
            malformed(
                warnings,
                clndr_id,
                line,
                format!("day number {day_number} is outside 1..=7"),
            );
            continue;
        };
        let Some(seen) = seen_days.get_mut(slot) else {
            continue;
        };
        if *seen {
            malformed(
                warnings,
                clndr_id,
                line,
                format!("day {day_number} is repeated; the first node is used"),
            );
            continue;
        }
        *seen = true;

        let scope = format!("day {day_number}");
        let (periods, _) = interpret_periods(arena, day, clndr_id, line, &scope, warnings);
        if let Some(day_periods) = week.as_mut().and_then(|days| days.get_mut(slot)) {
            *day_periods = periods;
        }
    }
}

fn interpret_exceptions(
    arena: &Arena<'_>,
    exceptions_node: &Node<'_>,
    clndr_id: u32,
    line: usize,
    exceptions: &mut Vec<CalendarException>,
    warnings: &mut Vec<Warning>,
) {
    for exception_index in &exceptions_node.children {
        let Some(exception) = arena.nodes.get(*exception_index) else {
            continue;
        };
        if !is_numeric_name(exception.name) {
            unknown(warnings, clndr_id, line, exception.name);
            continue;
        }
        let Some(serial_text) = attr(exception.attrs, "d") else {
            malformed(warnings, clndr_id, line, "exception day serial is missing");
            continue;
        };
        let Some(serial) = parse_serial(serial_text) else {
            malformed(
                warnings,
                clndr_id,
                line,
                format!("bad exception day serial {:?}", short(serial_text)),
            );
            continue;
        };
        let scope = format!("exception {serial}");
        let (periods, unparsed_periods) =
            interpret_periods(arena, exception, clndr_id, line, &scope, warnings);
        exceptions.push(CalendarException {
            serial,
            periods,
            unparsed_periods,
        });
    }
}

fn interpret_periods(
    arena: &Arena<'_>,
    parent: &Node<'_>,
    clndr_id: u32,
    line: usize,
    scope: &str,
    warnings: &mut Vec<Warning>,
) -> (Vec<Period>, usize) {
    let mut periods = Vec::new();
    let mut nodes = 0_usize;
    for period_index in &parent.children {
        let Some(period_node) = arena.nodes.get(*period_index) else {
            continue;
        };
        if !is_numeric_name(period_node.name) {
            unknown(warnings, clndr_id, line, period_node.name);
            continue;
        }
        nodes = nodes.saturating_add(1);
        for unknown_index in &period_node.children {
            if let Some(unknown_node) = arena.nodes.get(*unknown_index) {
                unknown(warnings, clndr_id, line, unknown_node.name);
            }
        }

        let (Some(start_text), Some(finish_text)) =
            (attr(period_node.attrs, "s"), attr(period_node.attrs, "f"))
        else {
            malformed(
                warnings,
                clndr_id,
                line,
                format!("period in {scope} is missing s or f"),
            );
            continue;
        };
        let Some(start) = parse_time(start_text, false) else {
            malformed(
                warnings,
                clndr_id,
                line,
                format!("bad start time {:?} in {scope}", short(start_text)),
            );
            continue;
        };
        let Some(finish) = parse_time(finish_text, true) else {
            malformed(
                warnings,
                clndr_id,
                line,
                format!("bad finish time {:?} in {scope}", short(finish_text)),
            );
            continue;
        };
        if finish < start {
            warnings.push(Warning::CalendarCrossesMidnight {
                clndr_id,
                line,
                scope: short(scope),
            });
            continue;
        }
        if finish == start {
            malformed(warnings, clndr_id, line, format!("empty period in {scope}"));
            continue;
        }
        periods.push(Period { start, finish });
    }

    let unparsed = nodes.saturating_sub(periods.len());
    periods.sort_by_key(|period| period.start);
    let mut non_overlapping: Vec<Period> = Vec::new();
    for period in periods {
        if non_overlapping
            .last()
            .is_some_and(|previous| period.start < previous.finish)
        {
            warnings.push(Warning::CalendarOverlap {
                clndr_id,
                line,
                scope: short(scope),
            });
        } else {
            non_overlapping.push(period);
        }
    }
    (non_overlapping, unparsed)
}

fn parse_time(text: &str, finish: bool) -> Option<u16> {
    let bytes = text.as_bytes();
    if bytes.len() != 5
        || bytes.get(2).copied() != Some(b':')
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 2 || byte.is_ascii_digit())
    {
        return None;
    }
    let hour = text.get(..2)?.parse::<u16>().ok()?;
    let minute = text.get(3..)?.parse::<u16>().ok()?;
    if minute > 59 {
        return None;
    }
    if hour == 24 {
        return (finish && minute == 0).then_some(1_440);
    }
    if hour > 23 {
        return None;
    }
    if finish && hour == 0 && minute == 0 {
        return Some(1_440);
    }
    hour.checked_mul(60)?.checked_add(minute)
}

fn parse_serial(text: &str) -> Option<i32> {
    let digits = match text.strip_prefix('-') {
        Some(digits) => digits,
        None => text,
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn is_numeric_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit())
}

fn malformed(warnings: &mut Vec<Warning>, clndr_id: u32, line: usize, detail: impl AsRef<str>) {
    warnings.push(Warning::CalendarMalformed {
        clndr_id,
        line,
        detail: short(detail.as_ref()),
    });
}

fn unknown(warnings: &mut Vec<Warning>, clndr_id: u32, line: usize, name: &str) {
    warnings.push(Warning::CalendarUnknownNode {
        clndr_id,
        line,
        name: short(name),
    });
}
