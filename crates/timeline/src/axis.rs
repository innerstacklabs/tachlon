//! Time axis: calendar dates for instants, tick levels chosen by zoom, and
//! tick labels.
//!
//! The engine has no calendar dates, only minutes from an origin that is a
//! Monday 00:00. A schedule gives that origin a date, an [`Origin`], so the
//! axis can label years and months: the file's own (the importer's
//! `origin_day`) or [`Origin::DEMO`] for a schedule with no file. Every
//! function that turns an instant into a date takes the origin; there is no
//! fixed one (gs#150).

use crate::names::{Names, first_letter};
use engine::{DAY, Instant, WEEK};

/// Smallest spacing, in CSS pixels, between two labelled ticks of a level.
pub const MIN_TICK_PX: f64 = 48.0;
/// Smallest width of a day column that still gets a tick of its own, in CSS pixels: wide enough
/// for a one-letter weekday. Below it the days run together and the week is the minor level.
pub const MIN_DAY_TICK_PX: f64 = 14.0;
/// A day column narrower than this is labelled with its weekday's first letter, CSS pixels.
const LETTER_DAY_PX: f64 = 28.0;

/// The calendar date of instant 0 of a schedule: a Monday, held as its day
/// count since 1970-01-01. Being a Monday, weekdays count the same from any
/// origin. Weeks start on Monday unless [`Origin::with_week_start`] says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Origin {
    day: i64,
    week_start: u8,
}

impl Origin {
    /// The origin of a schedule with no file (the demo and the stress
    /// fixture): Monday 5 January 2026.
    pub const DEMO: Origin = Origin::from_day(20_458);

    /// The origin on the day `days_since_1970` (a Monday, as the importer's
    /// `origin_day`).
    #[must_use]
    pub const fn from_day(days_since_1970: i64) -> Origin {
        Origin {
            day: days_since_1970,
            week_start: 0,
        }
    }

    /// This origin, with weeks that start `week_start` days after a Monday (0 to 6, wrapping).
    #[must_use]
    pub const fn with_week_start(self, week_start: u8) -> Origin {
        Origin {
            week_start: week_start % 7,
            ..self
        }
    }

    /// Days from 1970-01-01 to this origin.
    #[must_use]
    pub const fn day(self) -> i64 {
        self.day
    }

    /// Date of the day that contains instant `t`.
    #[must_use]
    pub fn date_of(self, t: Instant) -> Date {
        civil_from_days(self.day.saturating_add(t.div_euclid(DAY)))
    }

    /// Instant of 00:00 on date `d`.
    #[must_use]
    pub fn instant_of(self, d: Date) -> Instant {
        days_from_civil(d)
            .saturating_sub(self.day)
            .saturating_mul(DAY)
    }
}

impl Default for Origin {
    fn default() -> Origin {
        Origin::DEMO
    }
}

/// A calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    /// Year.
    pub year: i64,
    /// Month, 1 to 12.
    pub month: u32,
    /// Day of the month, 1 to 31.
    pub day: u32,
}

/// Days from 1970-01-01 to `d` (proleptic Gregorian; Hinnant's algorithm).
#[expect(
    clippy::cast_lossless,
    reason = "month and day are u32 fields (1 to 12, 1 to 31); the casts widen them to i64 without loss and keep the arithmetic as Hinnant published it"
)]
fn days_from_civil(d: Date) -> i64 {
    let y = if d.month <= 2 { d.year - 1 } else { d.year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = d.month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d.day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date `z` days from 1970-01-01.
fn civil_from_days(z: i64) -> Date {
    let z = z.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "in Hinnant's algorithm this is the day of the month, 1 to 31"
    )]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "mp is 0 to 11, so the month is 1 to 12"
    )]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    Date { year, month, day }
}

/// A tick level, finest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Every hour.
    Hour,
    /// Every day at 00:00.
    Day,
    /// Every first day of the week at 00:00: Monday unless the origin says otherwise.
    Week,
    /// The first of every month.
    Month,
    /// Every 1 January.
    Year,
}

impl Level {
    /// Every level, finest first.
    pub const ALL: [Level; 5] = [
        Level::Hour,
        Level::Day,
        Level::Week,
        Level::Month,
        Level::Year,
    ];

    /// Typical length in minutes (months and years vary).
    #[expect(
        clippy::cast_precision_loss,
        reason = "DAY and WEEK are the constants 1440 and 10080, exact in f64"
    )]
    #[must_use]
    pub fn typical_minutes(self) -> f64 {
        match self {
            Level::Hour => 60.0,
            Level::Day => DAY as f64,
            Level::Week => WEEK as f64,
            Level::Month => 30.44 * DAY as f64,
            Level::Year => 365.25 * DAY as f64,
        }
    }

    /// Next coarser level.
    #[must_use]
    pub fn coarser(self) -> Option<Level> {
        let i = Level::ALL.iter().position(|&l| l == self)?;
        Level::ALL.get(i + 1).copied()
    }

    /// Lower-case name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Level::Hour => "hour",
            Level::Day => "day",
            Level::Week => "week",
            Level::Month => "month",
            Level::Year => "year",
        }
    }

    /// Latest tick at or before `t`.
    #[must_use]
    pub fn floor(self, o: Origin, t: Instant) -> Instant {
        match self {
            Level::Hour => t.div_euclid(60) * 60,
            Level::Day => t.div_euclid(DAY) * DAY,
            // The origin is a Monday: a Monday week starts at a multiple of WEEK, a later one that many days on.
            Level::Week => {
                let shift = DAY * i64::from(o.week_start);
                (t - shift).div_euclid(WEEK) * WEEK + shift
            }
            Level::Month => {
                let d = o.date_of(t);
                o.instant_of(Date { day: 1, ..d })
            }
            Level::Year => {
                let d = o.date_of(t);
                o.instant_of(Date {
                    year: d.year,
                    month: 1,
                    day: 1,
                })
            }
        }
    }

    /// The tick after tick `t`.
    #[must_use]
    pub fn next(self, o: Origin, t: Instant) -> Instant {
        match self {
            Level::Hour => t + 60,
            Level::Day => t + DAY,
            Level::Week => t + WEEK,
            Level::Month => {
                let d = o.date_of(t);
                let (year, month) = if d.month == 12 {
                    (d.year + 1, 1)
                } else {
                    (d.year, d.month + 1)
                };
                o.instant_of(Date {
                    year,
                    month,
                    day: 1,
                })
            }
            Level::Year => o.instant_of(Date {
                year: o.date_of(t).year + 1,
                month: 1,
                day: 1,
            }),
        }
    }

    /// Narrowest spacing, in CSS pixels, at which this level's ticks are the minor level.
    #[must_use]
    pub fn min_tick_px(self) -> f64 {
        match self {
            Level::Day => MIN_DAY_TICK_PX,
            _ => MIN_TICK_PX,
        }
    }

    /// The label for a minor tick of this level when its ticks are `tick_px` apart. A day
    /// under 28 px wide gets its weekday's first letter, one under [`MIN_TICK_PX`] the weekday's
    /// three; every other case is [`Level::short_label`]. The names come from `names`.
    #[must_use]
    pub fn minor_label(self, o: Origin, t: Instant, tick_px: f64, names: &Names) -> String {
        match self {
            Level::Day if tick_px < LETTER_DAY_PX => first_letter(weekday(names, t)).to_string(),
            Level::Day if tick_px < MIN_TICK_PX => weekday(names, t).to_string(),
            _ => self.short_label(o, t, names),
        }
    }

    /// Short label for a minor tick.
    #[must_use]
    pub fn short_label(self, o: Origin, t: Instant, names: &Names) -> String {
        let d = o.date_of(t);
        let month = &names.months[d.month as usize - 1];
        match self {
            Level::Hour => format!("{:02}:00", t.rem_euclid(DAY) / 60),
            Level::Day => format!("{} {}", weekday(names, t), d.day),
            Level::Week => format!("{month} {}", d.day),
            Level::Month => month.clone(),
            Level::Year => d.year.to_string(),
        }
    }

    /// Long label for a major tick.
    #[must_use]
    pub fn long_label(self, o: Origin, t: Instant, names: &Names) -> String {
        let d = o.date_of(t);
        let month = &names.months[d.month as usize - 1];
        match self {
            Level::Hour => format!(
                "{} {} {month} {}, {:02}:00",
                weekday(names, t),
                d.day,
                d.year,
                t.rem_euclid(DAY) / 60
            ),
            Level::Day => format!("{} {} {month} {}", weekday(names, t), d.day, d.year),
            Level::Week => format!("{} {month} {}", d.day, d.year),
            Level::Month => format!("{month} {}", d.year),
            Level::Year => d.year.to_string(),
        }
    }

    /// Ticks from the last one at or before `from` up to `to` inclusive.
    #[must_use]
    pub fn ticks(self, o: Origin, from: Instant, to: Instant) -> Vec<Instant> {
        let mut out = Vec::new();
        let mut t = self.floor(o, from);
        while t <= to {
            out.push(t);
            t = self.next(o, t);
        }
        out
    }
}

/// Weekday of the day holding `t`. The origin is a Monday, so no origin is needed.
#[expect(clippy::cast_possible_truncation, reason = "rem_euclid(7) is 0 to 6")]
fn weekday(names: &Names, t: Instant) -> &str {
    &names.weekdays[t.div_euclid(DAY).rem_euclid(7) as usize]
}

/// Minor and major levels for a scale: minor is the finest level whose ticks
/// are at least [`Level::min_tick_px`] apart; major is the next coarser one, if any.
#[must_use]
pub fn levels_for(px_per_minute: f64) -> (Level, Option<Level>) {
    let minor = Level::ALL
        .into_iter()
        .find(|l| l.typical_minutes() * px_per_minute >= l.min_tick_px())
        .unwrap_or(Level::Year);
    (minor, minor.coarser())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Days from 1970-01-01 to Monday 2024-01-01.
    const MONDAY_2024: i64 = 19_723;

    fn date(year: i64, month: u32, day: u32) -> Date {
        Date { year, month, day }
    }

    #[test]
    fn origin_and_known_dates() {
        let o = Origin::DEMO;
        assert_eq!(o.date_of(0), date(2026, 1, 5));
        assert_eq!(days_from_civil(date(1970, 1, 1)), 0);
        assert_eq!(days_from_civil(date(2000, 3, 1)), 11_017);
        assert_eq!(o.day(), days_from_civil(date(2026, 1, 5)));
        assert_eq!(o.date_of(-1), date(2026, 1, 4));
        assert_eq!(o.date_of(55 * DAY), date(2026, 3, 1)); // 2026 is not a leap year
        assert_eq!(
            o.date_of(o.instant_of(date(2028, 2, 29))),
            date(2028, 2, 29)
        );
        assert_eq!(weekday(&Names::default(), 0), "Mon");
        assert_eq!(
            weekday(&Names::default(), o.instant_of(date(2026, 10, 20))),
            "Tue"
        );
        for z in -1000..3000 {
            assert_eq!(days_from_civil(civil_from_days(z * 37)), z * 37);
        }
    }

    #[test]
    fn a_file_origin_gives_its_own_dates() {
        let o = Origin::from_day(MONDAY_2024);
        assert_eq!(o.date_of(0), date(2024, 1, 1));
        assert_eq!(o.date_of(-1), date(2023, 12, 31));
        assert_eq!(o.date_of(59 * DAY), date(2024, 2, 29)); // 2024 is a leap year
        assert_eq!(o.instant_of(date(2024, 1, 10)), 9 * DAY);
        assert_eq!(
            weekday(&Names::default(), o.instant_of(date(2024, 1, 1))),
            "Mon"
        );
        assert_eq!(Origin::default(), Origin::DEMO);
    }

    #[test]
    fn ticks_land_on_boundaries() {
        let o = Origin::DEMO;
        let from = 3 * DAY + 125;
        let to = from + 400 * DAY;
        for level in Level::ALL {
            let ticks = level.ticks(o, from, to);
            assert!(
                ticks[0] <= from && level.next(o, ticks[0]) > from,
                "{level:?}"
            );
            assert!(ticks.windows(2).all(|w| w[0] < w[1]));
            for &t in &ticks {
                assert_eq!(level.floor(o, t), t, "{level:?} {t}");
            }
        }
        let months = Level::Month.ticks(o, 0, o.instant_of(date(2027, 1, 1)));
        assert_eq!(months.len(), 13); // Jan 1 2026 (before the origin) .. Jan 1 2027
        assert_eq!(o.date_of(months[1]), date(2026, 2, 1));
        let weeks = Level::Week.ticks(o, 0, 3 * WEEK);
        assert!(
            weeks
                .iter()
                .all(|&t| weekday(&Names::default(), t) == "Mon")
        );
    }

    #[test]
    fn week_ticks_land_on_a_sunday_when_asked() {
        let o = Origin::DEMO.with_week_start(6);
        let weeks = Level::Week.ticks(o, 0, 3 * WEEK);
        assert_eq!(weeks[0], -DAY, "the Sunday before the Monday origin");
        assert!(
            weeks
                .iter()
                .all(|&t| weekday(&Names::default(), t) == "Sun")
        );
    }

    #[test]
    #[expect(
        clippy::cast_precision_loss,
        reason = "DAY is the constant 1440, exact in f64"
    )]
    fn levels_follow_zoom() {
        // 1200 px wide: two hours, a fortnight, a year, five years.
        assert_eq!(levels_for(1200.0 / 120.0), (Level::Hour, Some(Level::Day)));
        assert_eq!(levels_for(1200.0 / (14.0 * DAY as f64)).0, Level::Day);
        // Weeks zoom: 20 px a day is day letters under weeks; 10 px a day is weeks under months.
        let per_day = |px: f64| px / DAY as f64;
        assert_eq!(levels_for(per_day(20.0)), (Level::Day, Some(Level::Week)));
        assert_eq!(levels_for(per_day(13.9)), (Level::Week, Some(Level::Month)));
        assert_eq!(levels_for(per_day(14.0)), (Level::Day, Some(Level::Week)));
        assert_eq!(levels_for(1200.0 / (365.0 * DAY as f64)).0, Level::Month);
        assert_eq!(
            levels_for(1200.0 / (5.0 * 365.25 * DAY as f64)),
            (Level::Year, None)
        );
    }

    #[test]
    fn labels() {
        let o = Origin::DEMO;
        let t = o.instant_of(date(2026, 3, 16)) + 14 * 60;
        assert_eq!(Level::Hour.short_label(o, t, &Names::default()), "14:00");
        assert_eq!(Level::Day.short_label(o, t, &Names::default()), "Mon 16");
        assert_eq!(Level::Week.short_label(o, t, &Names::default()), "Mar 16");
        assert_eq!(Level::Month.long_label(o, t, &Names::default()), "Mar 2026");
        assert_eq!(
            Level::Day.long_label(o, t, &Names::default()),
            "Mon 16 Mar 2026"
        );
        assert_eq!(Level::Year.short_label(o, t, &Names::default()), "2026");
    }

    #[test]
    fn a_day_column_is_a_letter_then_three_letters_then_weekday_and_date() {
        let o = Origin::DEMO;
        let t = o.instant_of(date(2026, 3, 18)); // a Wednesday
        assert_eq!(Level::Day.minor_label(o, t, 14.0, &Names::default()), "W");
        assert_eq!(Level::Day.minor_label(o, t, 27.9, &Names::default()), "W");
        assert_eq!(Level::Day.minor_label(o, t, 28.0, &Names::default()), "Wed");
        assert_eq!(Level::Day.minor_label(o, t, 47.9, &Names::default()), "Wed");
        assert_eq!(
            Level::Day.minor_label(o, t, 48.0, &Names::default()),
            "Wed 18"
        );
        // Other levels do not shorten.
        assert_eq!(
            Level::Week.minor_label(o, t, 100.0, &Names::default()),
            "Mar 18"
        );
        assert_eq!(
            Level::Week.long_label(o, t, &Names::default()),
            "18 Mar 2026"
        );
    }

    #[test]
    fn a_day_label_cut_to_a_letter_keeps_a_whole_character() {
        let o = Origin::DEMO;
        let t = o.instant_of(date(2026, 3, 18)); // a Wednesday
        let mut names = Names::default();
        names.weekdays[2] = "Écu".to_string(); // É is two bytes: a cut at byte 1 is not a boundary
        names.months[2] = "Ärz".to_string();
        assert_eq!(Level::Day.minor_label(o, t, 14.0, &names), "É");
        assert_eq!(Level::Day.minor_label(o, t, 27.9, &names), "É");
        assert_eq!(Level::Day.minor_label(o, t, 28.0, &names), "Écu");
        assert_eq!(Level::Day.short_label(o, t, &names), "Écu 18");
        assert_eq!(Level::Month.short_label(o, t, &names), "Ärz");
        assert_eq!(Level::Day.long_label(o, t, &names), "Écu 18 Ärz 2026");
    }

    /// gs#150: a file planned in 2024 reads 2024 on the axis, in the labels and in where the
    /// month and year ticks fall.
    #[test]
    fn a_2024_file_reads_2024() {
        let o = Origin::from_day(MONDAY_2024);
        let t = 9 * DAY + 8 * 60; // 2024-01-10 08:00, the data date
        assert_eq!(Level::Year.short_label(o, t, &Names::default()), "2024");
        assert_eq!(Level::Year.long_label(o, t, &Names::default()), "2024");
        assert_eq!(Level::Month.long_label(o, t, &Names::default()), "Jan 2024");
        assert_eq!(Level::Month.short_label(o, t, &Names::default()), "Jan");
        assert_eq!(
            Level::Day.long_label(o, t, &Names::default()),
            "Wed 10 Jan 2024"
        );
        assert_eq!(
            Level::Week.long_label(o, t, &Names::default()),
            "10 Jan 2024"
        );
        assert_eq!(Level::Day.short_label(o, t, &Names::default()), "Wed 10");
        assert_eq!(Level::Week.short_label(o, t, &Names::default()), "Jan 10");
        // The demo origin reads the same instant as 2026.
        assert_eq!(
            Level::Year.short_label(Origin::DEMO, t, &Names::default()),
            "2026"
        );
        // Ticks fall on the file's month and year starts.
        let year = Level::Year.ticks(o, 0, 400 * DAY);
        assert_eq!(
            year.iter().map(|&t| o.date_of(t)).collect::<Vec<_>>(),
            [date(2024, 1, 1), date(2025, 1, 1)]
        );
        let months = Level::Month.ticks(o, 0, 70 * DAY);
        assert_eq!(
            months.iter().map(|&t| o.date_of(t)).collect::<Vec<_>>(),
            [date(2024, 1, 1), date(2024, 2, 1), date(2024, 3, 1)]
        );
        assert_eq!(months[1], 31 * DAY);
        assert_eq!(months[2], 60 * DAY);
    }
}
