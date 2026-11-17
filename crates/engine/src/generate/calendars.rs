//! The four generator calendars and the holidays and working exceptions they draw.

use super::Rng;
use crate::calendar::{Calendar, DAY, DayPeriod};
use crate::model::{CalendarId, Schedule};

/// Holidays spread over 40 years, about ten a year, as day indices.
fn holidays(rng: &mut Rng) -> Vec<i64> {
    (0..40 * 10)
        .map(|i| i * 365 / 10 + rng.range(0, 30))
        .collect()
}

/// The holidays `holidays` plus about ten working exceptions a year over 40
/// years, drawn from `rng`: by turns a Saturday worked 08:00-12:00 and a
/// Wednesday shortened to 08:00-12:00. A day that is already a holiday stays
/// one.
fn working_exceptions(rng: &mut Rng, holidays: &[i64]) -> Vec<(i64, Vec<DayPeriod>)> {
    let mut days: Vec<i64> = holidays.to_vec();
    days.sort_unstable();
    days.dedup();
    let mut out: Vec<(i64, Vec<DayPeriod>)> = days.iter().map(|&d| (d, Vec::new())).collect();
    for i in 0..40 * 10 {
        let base = i * 365 / 10 + rng.range(0, 30);
        let day = base - base % 7 + if i % 2 == 0 { 5 } else { 2 };
        if days.binary_search(&day).is_err() {
            out.push((day, vec![(480, 720)]));
        }
    }
    out
}

/// The four generator calendars:
/// 0: Mon-Fri 08:00-12:00, 13:00-17:00, with holidays;
/// 1: Mon-Sat 07:00-17:00 (weekend-working);
/// 2: 24/7 (curing, monitoring);
/// 3: Mon-Thu 06:00-16:30, with holidays.
///
/// With `working`, calendars 0 and 3 also get the working exceptions of
/// [`working_exceptions`], drawn from that stream.
pub(super) fn add_calendars(
    s: &mut Schedule,
    rng: &mut Rng,
    mut working: Option<&mut Rng>,
    whole_hours: bool,
) -> [CalendarId; 4] {
    let std: Vec<DayPeriod> = vec![(480, 720), (780, 1020)];
    let six: Vec<DayPeriod> = vec![(420, 1020)];
    #[expect(
        clippy::cast_possible_truncation,
        reason = "DAY is the constant 1440, below u16::MAX"
    )]
    let all: Vec<DayPeriod> = vec![(0, DAY as u16)];
    let ten: Vec<DayPeriod> = vec![(360, if whole_hours { 960 } else { 990 })];
    let e = Vec::new;
    // Drawn in the order the calendars were always built, so `working`
    // leaves every other draw unchanged.
    let (h0, h3) = (holidays(rng), holidays(rng));
    let mut with = |week: [Vec<DayPeriod>; 7], h: Vec<i64>| match working.as_deref_mut() {
        None => Calendar::new(week, &h),
        Some(ex) => Calendar::with_exceptions(week, &working_exceptions(ex, &h)),
    };
    let cals = [
        with(
            [
                std.clone(),
                std.clone(),
                std.clone(),
                std.clone(),
                std,
                e(),
                e(),
            ],
            h0,
        ),
        Calendar::new(
            [
                six.clone(),
                six.clone(),
                six.clone(),
                six.clone(),
                six.clone(),
                six,
                e(),
            ],
            &[],
        ),
        Calendar::new(
            [
                all.clone(),
                all.clone(),
                all.clone(),
                all.clone(),
                all.clone(),
                all.clone(),
                all,
            ],
            &[],
        ),
        with(
            [ten.clone(), ten.clone(), ten.clone(), ten, e(), e(), e()],
            h3,
        ),
    ];
    cals.map(|c| s.add_calendar(c.expect("generator calendars are valid")))
}
