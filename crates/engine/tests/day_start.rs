//! `Calendar::day_start_at`: where a start asked for at an instant opens its
//! working day.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{Calendar, DAY};

const OPEN: i64 = 8 * 60;

/// Monday to Friday, 08:00-12:00 and 13:00-17:00, with Wednesday (day 2) off.
fn calendar() -> Calendar {
    let day = vec![(OPEN as u16, 12 * 60), (13 * 60, 17 * 60)];
    let week = std::array::from_fn(|d| if d < 5 { day.clone() } else { Vec::new() });
    Calendar::new(week, &[2]).expect("valid calendar")
}

#[test]
fn a_working_minute_gives_its_days_first_working_minute() {
    let cal = calendar();
    assert_eq!(cal.day_start_at(OPEN), OPEN);
    assert_eq!(cal.day_start_at(DAY + 14 * 60), DAY + OPEN);
}

#[test]
fn time_off_gives_the_next_working_days_start() {
    let cal = calendar();
    // Monday 18:00, Tuesday 12:30 (the gap) and Tuesday 18:00.
    assert_eq!(cal.day_start_at(18 * 60), DAY + OPEN);
    assert_eq!(cal.day_start_at(DAY + 12 * 60 + 30), DAY + OPEN);
    // Tuesday 18:00 and the holiday Wednesday both give Thursday.
    assert_eq!(cal.day_start_at(DAY + 18 * 60), 3 * DAY + OPEN);
    assert_eq!(cal.day_start_at(2 * DAY + 9 * 60), 3 * DAY + OPEN);
    // Saturday noon gives next Monday.
    assert_eq!(cal.day_start_at(5 * DAY + 12 * 60), 7 * DAY + OPEN);
}

#[test]
fn before_the_origin_gives_the_first_working_start() {
    assert_eq!(calendar().day_start_at(-500), OPEN);
}
