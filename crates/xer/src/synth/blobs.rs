//! Calendar blobs, identifiers and dates for the table writers.

use super::Output;

// Exception serials count days from 1899-12-30 (confirmed by public samples: 43650 is 2019-07-04); this is 2024-01-01.
const EXCEPTION_SERIAL_2024_01_01: u32 = 45_292;
const TASKS_PER_ID_BLOCK: u32 = 10_000_000;

/// Identifiers wrap modulo 2^32 on purpose (see the doc on `write`): a specification past ten
/// million activities per project is allowed to repeat identifiers, and never panics.
#[cfg_attr(
    target_pointer_width = "64",
    expect(
        clippy::cast_possible_truncation,
        reason = "the wrap modulo 2^32 is the documented behaviour past ten million activities"
    )
)]
pub(super) fn wrap_u32(value: usize) -> u32 {
    value as u32
}

pub(super) fn weekday_blob(del: bool) -> String {
    let gap = if del { "\u{7f}\u{7f}" } else { "" };
    let days = "(0||1()())(0||2()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))(0||3()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))(0||4()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))(0||5()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))(0||6()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))(0||7()())";
    format!("(0||CalendarData()((0||DaysOfWeek()({days})){gap}(0||Exceptions()())))")
}

pub(super) fn full_day_blob() -> String {
    let day = "((0||0(s|00:00|f|24:00)()))";
    format!(
        "(0||CalendarData()((0||DaysOfWeek()((0||1(){day})(0||2(){day})(0||3(){day})(0||4(){day})(0||5(){day})(0||6(){day})(0||7(){day})))(0||Exceptions()())))"
    )
}

pub(super) fn project_blob(project: usize) -> String {
    let base = EXCEPTION_SERIAL_2024_01_01.wrapping_add(wrap_u32(project).wrapping_mul(30));
    // DEL pairs stand for a P6 line break; public samples show them in other values, not in a clndr_data.
    let week = weekday_blob(true);
    let exceptions = format!(
        "(0||Exceptions()((0||0(d|{base})())(0||1(d|{})((0||0(s|09:00|f|12:00)())))(0||2(d|{})((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|15:00)())))))",
        base.wrapping_add(1),
        base.wrapping_add(2)
    );
    week.replacen("(0||Exceptions()())", &exceptions, 1)
}

pub(super) fn project_id(project: usize) -> u32 {
    1_000u32.wrapping_add(wrap_u32(project))
}
pub(super) fn project_calendar_id(project: usize) -> u32 {
    id(102, project, 1, 0)
}
pub(super) fn wbs_id(project: usize, child: usize) -> u32 {
    id(100_000, project, 1_000, child)
}
pub(super) fn task_id(project: usize, activity: usize) -> u32 {
    id(1_000_000, project, TASKS_PER_ID_BLOCK, activity)
}
pub(super) fn id(base: u32, project: usize, stride: u32, offset: usize) -> u32 {
    base.wrapping_add(wrap_u32(project).wrapping_mul(stride))
        .wrapping_add(wrap_u32(offset))
}

pub(super) fn date(out: &mut Output, offset: usize, hour: u8) {
    let month = offset / 28 % 12 + 1;
    let day = offset % 28 + 1;
    out.formatted(format_args!("2024-{month:02}-{day:02} {hour:02}:00"));
}
