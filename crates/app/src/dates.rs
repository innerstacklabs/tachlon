//! Calendar dates for instants (gs#150).
//!
//! The engine counts minutes from an origin that is a Monday 00:00 and knows no dates. The
//! importer picks that Monday from the file: the Monday on or before the plan start
//! ([`import::Imported::origin_day`]). [`Origin`] carries it into the app, and every date the
//! app writes or reads, the table, the hover card, the toolbar's data date and the lookahead
//! start and its input, goes through [`Origin::date_of`] or its inverse [`Origin::parse_day`].
//! A schedule with no file (the demo and the stress fixture) uses [`Origin::DEMO`].
//!
//! The calendar arithmetic lives once, in [`timeline::axis::Origin`]. This `Origin` holds one
//! and adds the date texts; the canvas axis takes the same one ([`Origin::axis`]), so a table
//! and a chart never disagree on the year.

use crate::Session;
use engine::{DAY, Instant};
use timeline::axis::{self, Date};

/// The calendar date of instant 0 of a schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origin {
    axis: axis::Origin,
}

impl Origin {
    /// The origin of a schedule that has no file: Monday 2026-01-05.
    pub const DEMO: Origin = Origin {
        axis: axis::Origin::DEMO,
    };

    /// The origin on the day `days_since_1970` (as [`import::Imported::origin_day`]). It is a
    /// Monday, so weekdays count the same from any origin.
    #[must_use]
    pub fn from_day(days_since_1970: i64) -> Origin {
        Origin {
            axis: axis::Origin::from_day(days_since_1970),
        }
    }

    /// Days from 1970-01-01 to the origin: what [`Origin::from_day`] takes back.
    #[must_use]
    pub fn day(self) -> i64 {
        self.axis.day()
    }

    /// The same origin for the canvas axis (`timeline::scene::Backdrop::origin`).
    #[must_use]
    pub fn axis(self) -> axis::Origin {
        self.axis
    }

    /// The date of the day that holds `t`. Every date text goes through here.
    #[must_use]
    pub fn date_of(self, t: Instant) -> Date {
        self.axis.date_of(t)
    }

    /// `t` as `YYYY-MM-DD HH:MM`, the way the table writes dates.
    #[must_use]
    pub fn stamp(self, t: Instant) -> String {
        let d = self.date_of(t);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            d.year,
            d.month,
            d.day,
            t.rem_euclid(DAY) / 60,
            t.rem_euclid(60)
        )
    }

    /// The date of the day that holds `t`, written `YYYY-MM-DD`.
    #[must_use]
    pub fn day_text(self, t: Instant) -> String {
        let d = self.date_of(t);
        format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
    }

    /// Instant of 00:00 on the date `text` names, written `YYYY-MM-DD`. `None` for anything
    /// else, including a date that does not exist (2026-02-30). Any four-digit year is a date,
    /// so a file planned in 1850 shows a start the box accepts back.
    #[must_use]
    pub fn parse_day(self, text: &str) -> Option<Instant> {
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let number = |part: &str| -> Option<i64> {
            part.bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| part.parse().ok())
                .flatten()
        };
        let year = number(&text[0..4])?;
        let month = number(&text[5..7])?;
        let day = number(&text[8..10])?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "month is checked to 1 to 12 and day to 1 to 31 above, so both are positive and fit u32"
        )]
        let date = Date {
            year,
            month: month as u32,
            day: day as u32,
        };
        let at = self.axis.instant_of(date);
        // A day the month does not have rolls over into the next month: reject it.
        (self.axis.date_of(at) == date).then_some(at)
    }
}

/// The session reads and writes its dates here, from [`Session::origin`] (gs#150).
impl Session {
    /// The calendar date of instant 0.
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// `t` as `YYYY-MM-DD HH:MM`, the way the table and the hover card write dates.
    pub fn date_text(&self, t: engine::Instant) -> String {
        self.origin.stamp(t)
    }

    /// The file's data date as [`Session::date_text`] writes it, or `None` for a schedule with no
    /// data date.
    pub fn data_date_text(&self) -> Option<String> {
        self.data_date.map(|t| self.date_text(t))
    }

    /// The date, `YYYY-MM-DD`, the lookahead box shows: the window's start when one is set,
    /// else [`Session::default_lookahead_start`].
    pub fn lookahead_start_text(&self) -> String {
        let start = self
            .lookahead
            .map_or_else(|| self.default_lookahead_start(), |window| window.start);
        self.origin.day_text(start)
    }

    /// The instant of 00:00 on a date the lookahead box holds, written `YYYY-MM-DD`.
    pub fn parse_lookahead_day(&self, text: &str) -> Option<engine::Instant> {
        self.origin.parse_day(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::WEEK;

    /// Days from 1970-01-01 to Monday 2024-01-01.
    const MONDAY_2024: i64 = 19_723;
    /// Days from 1970-01-01 to Monday 2026-01-05, the demo origin.
    const DEMO_DAY: i64 = 20_458;

    #[test]
    fn the_demo_origin_is_2026_01_05() {
        assert_eq!(Origin::from_day(DEMO_DAY), Origin::DEMO);
        assert_eq!(Origin::DEMO.day_text(0), "2026-01-05");
        assert_eq!(Origin::DEMO.axis().day(), DEMO_DAY);
    }

    #[test]
    fn the_demo_origin_keeps_its_dates() {
        let o = Origin::DEMO;
        assert_eq!(o.parse_day("2026-01-05"), Some(0));
        assert_eq!(o.parse_day("2026-01-12"), Some(WEEK));
        assert_eq!(o.parse_day("2026-01-04"), Some(-DAY));
        assert_eq!(o.day_text(WEEK + 13 * 60), "2026-01-12");
        assert_eq!(o.day_text(-1), "2026-01-04");
        assert_eq!(o.stamp(WEEK + 13 * 60 + 5), "2026-01-12 13:05");
    }

    #[test]
    fn a_file_origin_moves_every_date() {
        let o = Origin::from_day(MONDAY_2024);
        assert_eq!(o.day_text(0), "2024-01-01");
        assert_eq!(o.stamp(9 * DAY + 8 * 60), "2024-01-10 08:00");
        assert_eq!(o.parse_day("2024-01-10"), Some(9 * DAY));
        assert_eq!(o.parse_day("2024-01-01"), Some(0));
        assert_eq!(o.day_text(-1), "2023-12-31");
        // 2024 is a leap year.
        assert_eq!(o.day_text(59 * DAY), "2024-02-29");
        assert_eq!(o.parse_day("2024-02-29"), Some(59 * DAY));
        // An origin after the axis's, and one far before it.
        let late = Origin::from_day(DEMO_DAY + 7 * 52);
        assert_eq!(late.parse_day(&late.day_text(3 * DAY)), Some(3 * DAY));
    }

    #[test]
    fn every_date_written_is_read_back() {
        for origin_day in [MONDAY_2024, DEMO_DAY, 10_959, -354_285, 2_900_000] {
            let o = Origin::from_day(origin_day);
            for t in [-WEEK, -1, 0, 1, DAY - 1, DAY, 40 * DAY + 13 * 60, 900 * DAY] {
                let text = o.day_text(t);
                let day = o
                    .parse_day(&text)
                    .unwrap_or_else(|| panic!("{text} reads back"));
                assert_eq!(day, t.div_euclid(DAY) * DAY, "{text}");
                assert_eq!(o.day_text(day), text);
                assert_eq!(&o.stamp(t)[..10], text);
            }
        }
    }

    #[test]
    fn bad_dates_are_refused() {
        for bad in [
            "",
            "2026-1-5",
            "2026-02-30",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "2025-02-29",
            "2026/01/05",
            "26-01-05",
            "+026-01-05",
            "2026-01-0x",
            "2026-01-05 ",
            "２０２６-01-05",
        ] {
            assert_eq!(Origin::DEMO.parse_day(bad), None, "{bad:?}");
            assert_eq!(
                Origin::from_day(MONDAY_2024).parse_day(bad),
                None,
                "{bad:?}"
            );
        }
        for far in ["1850-01-01", "2300-01-01", "0001-01-01", "9999-12-31"] {
            let o = Origin::DEMO;
            assert_eq!(o.day_text(o.parse_day(far).unwrap()), far);
        }
    }
}
