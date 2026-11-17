use crate::{CalendarRow, Data, Error, Warning};

mod exceptions;
mod interpret;
mod scan;
mod text;
mod write;

use interpret::interpret;
use scan::{ScanError, scan};
use text::short;
pub use write::write_clndr_data;

/// Minutes since local midnight; `finish` may be 1440.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Period {
    /// The inclusive start minute, in the range 0 through 1439.
    pub start: u16,
    /// The exclusive finish minute, in the range 1 through 1440.
    pub finish: u16,
}

/// A P6 week: index 0 is Sunday and index 6 is Saturday.
///
/// An empty vector represents a non-working day.
pub type Week = [Vec<Period>; 7];

/// An exception day in a calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarException {
    /// The raw P6 day count from the 1899-12-30 epoch (public samples: 43650 is 2019-07-04).
    pub serial: i32,
    /// Working periods; empty is a whole-day exception only when `unparsed_periods` is 0.
    pub periods: Vec<Period>,
    /// Unreadable period nodes dropped; with empty `periods` this is not a holiday.
    pub unparsed_periods: usize,
}

/// The weekly schedule and exception days decoded from one calendar blob.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarData {
    /// The weekly schedule, absent when there is no `DaysOfWeek` node.
    pub week: Option<Week>,
    /// Exception days in file order.
    pub exceptions: Vec<CalendarException>,
}

/// Result of parsing one calendar blob.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedClndr {
    /// Decoded data, or `None` when the blob's tree syntax is malformed.
    pub data: Option<CalendarData>,
    /// Recoverable problems found while parsing or interpreting the blob.
    pub warnings: Vec<Warning>,
}

impl Data {
    /// Returns the first calendar row with the requested identifier.
    #[must_use]
    pub fn calendar(&self, clndr_id: u32) -> Option<&CalendarRow> {
        self.calendar_index
            .get(&clndr_id)
            .and_then(|index| self.calendar_rows.get(*index))
    }
}

/// Parses one `CALENDAR.clndr_data` blob.
///
/// `line` is the 1-based source line of the `CALENDAR` row and is copied into
/// every warning or error produced by this function.
///
/// # Errors
///
/// Returns [`Error::CalendarTooDeep`] when the blob nests more than 64 nodes.
pub fn parse_clndr_data(clndr_id: u32, line: usize, text: &str) -> Result<ParsedClndr, Error> {
    let arena = match scan(text) {
        Ok(arena) => arena,
        Err(ScanError::Syntax(detail)) => {
            return Ok(ParsedClndr {
                data: None,
                warnings: vec![Warning::CalendarMalformed {
                    clndr_id,
                    line,
                    detail: short(&detail),
                }],
            });
        }
        Err(ScanError::TooDeep) => return Err(Error::CalendarTooDeep { clndr_id, line }),
    };

    let mut warnings = Vec::new();
    let data = interpret(&arena, clndr_id, line, &mut warnings);
    Ok(ParsedClndr {
        data: Some(data),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::{CalendarData, CalendarException, ParsedClndr, Period, parse_clndr_data};
    use crate::{Error, Warning};

    fn data(blob: &str) -> ParsedClndr {
        parse_clndr_data(17, 9, blob).expect("calendar parser should return a value")
    }

    #[test]
    fn parses_monday_to_friday_week_with_lunch_split() {
        let blob = concat!(
            "(0||CalendarData()((0||DaysOfWeek()(",
            "(0||1()())",
            "(0||2()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))",
            "(0||3()((0||0(s|08:00|f|17:00)())))",
            "(0||4()((0||0(s|08:00|f|17:00)())))",
            "(0||5()((0||0(s|08:00|f|17:00)())))",
            "(0||6()((0||0(s|08:00|f|17:00)())))",
            "(0||7()())",
            "))))"
        );
        let parsed = data(blob);
        let mut week: [Vec<Period>; 7] = std::array::from_fn(|_| Vec::new());
        week[1] = vec![
            Period {
                start: 480,
                finish: 720,
            },
            Period {
                start: 780,
                finish: 1_020,
            },
        ];
        for day in week.iter_mut().take(6).skip(2) {
            *day = vec![Period {
                start: 480,
                finish: 1_020,
            }];
        }
        assert_eq!(
            parsed,
            ParsedClndr {
                data: Some(CalendarData {
                    week: Some(week),
                    exceptions: Vec::new(),
                }),
                warnings: Vec::new(),
            }
        );
    }

    #[test]
    fn keeps_holiday_exception() {
        let parsed = data("(0||CalendarData()((0||Exceptions()((0||0(d|43101)())))))");
        assert_eq!(
            parsed.data.expect("calendar data").exceptions,
            vec![CalendarException {
                serial: 43_101,
                periods: Vec::new(),
                unparsed_periods: 0,
            }]
        );
    }

    #[test]
    fn keeps_shortened_hours_exception() {
        let parsed = data(concat!(
            "(0||CalendarData()((0||Exceptions()(",
            "(0||0(d|43102)((0||0(s|08:00|f|12:00)())))",
            "))))"
        ));
        assert_eq!(
            parsed.data.expect("calendar data").exceptions,
            vec![CalendarException {
                serial: 43_102,
                periods: vec![Period {
                    start: 480,
                    finish: 720,
                }],
                unparsed_periods: 0,
            }]
        );
    }

    #[test]
    fn finish_at_midnight_means_end_of_day() {
        let parsed = data(concat!(
            "(0||CalendarData()((0||DaysOfWeek()(",
            "(0||1()((0||0(s|23:00|f|00:00)())))",
            "(0||2()((0||0(s|00:00|f|00:00)())))",
            "))))"
        ));
        let week = parsed.data.expect("calendar data").week.expect("week data");
        assert_eq!(
            week[0],
            vec![Period {
                start: 1_380,
                finish: 1_440,
            }]
        );
        assert_eq!(
            week[1],
            vec![Period {
                start: 0,
                finish: 1_440,
            }]
        );
    }

    #[test]
    fn drops_period_crossing_midnight() {
        let parsed = data(concat!(
            "(0||CalendarData()((0||DaysOfWeek()(",
            "(0||2()((0||0(s|22:00|f|02:00)())))",
            "))))"
        ));
        assert_eq!(
            parsed.warnings,
            vec![Warning::CalendarCrossesMidnight {
                clndr_id: 17,
                line: 9,
                scope: "day 2".to_owned(),
            }]
        );
        assert_eq!(
            parsed.data.expect("calendar data").week.expect("week")[1],
            [] as [Period; 0]
        );
    }

    #[test]
    fn drops_later_overlapping_period() {
        let parsed = data(concat!(
            "(0||CalendarData()((0||DaysOfWeek()(",
            "(0||2()((0||0(s|08:00|f|12:00)())(0||1(s|11:00|f|13:00)())))",
            "))))"
        ));
        assert_eq!(
            parsed.warnings,
            vec![Warning::CalendarOverlap {
                clndr_id: 17,
                line: 9,
                scope: "day 2".to_owned(),
            }]
        );
        assert_eq!(
            parsed.data.expect("calendar data").week.expect("week")[1],
            vec![Period {
                start: 480,
                finish: 720,
            }]
        );
    }

    #[test]
    fn enforces_node_depth_without_recursion() {
        fn nested(depth: usize) -> String {
            let mut blob = String::new();
            for index in 0..depth {
                blob.extend(["(0||N", &index.to_string(), "()("]);
            }
            blob.push_str(&")".repeat(depth));
            blob.push_str(&")".repeat(depth));
            blob
        }

        assert!(parse_clndr_data(3, 4, &nested(64)).is_ok());
        assert_eq!(
            parse_clndr_data(3, 4, &nested(65)),
            Err(Error::CalendarTooDeep {
                clndr_id: 3,
                line: 4,
            })
        );
    }

    #[test]
    fn crlf_and_lf_whitespace_parse_equally() {
        let lf = "(0||CalendarData()(\n(0||DaysOfWeek()(\n(0||1()())\n))\n))";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(data(lf), data(&crlf));
        assert_eq!(
            data(lf),
            data("(0||CalendarData()((0||DaysOfWeek()((0||1()())))))")
        );
    }

    #[test]
    fn warns_for_unknown_nodes_and_keeps_known_data() {
        let parsed = data(concat!(
            "(0||CalendarData()((0||Mystery()())(0||DaysOfWeek()((0||1()())))))",
            "(0||Other()())"
        ));
        assert_eq!(
            parsed.warnings,
            vec![
                Warning::CalendarUnknownNode {
                    clndr_id: 17,
                    line: 9,
                    name: "Mystery".to_owned(),
                },
                Warning::CalendarUnknownNode {
                    clndr_id: 17,
                    line: 9,
                    name: "Other".to_owned(),
                },
            ]
        );
        assert!(parsed.data.expect("calendar data").week.is_some());
    }

    #[test]
    fn malformed_inputs_warn_without_panicking() {
        let truncated = data("(0||CalendarData()(");
        assert!(truncated.data.is_none());
        assert!(matches!(
            truncated.warnings.as_slice(),
            [Warning::CalendarMalformed {
                clndr_id: 17,
                line: 9,
                ..
            }]
        ));

        for blob in [
            "(0||CalendarData()((0||DaysOfWeek()((0||2()((0||0(s|25:00|f|17:00)())))))))",
            "(0||CalendarData()((0||Exceptions()((0||0(d|oops)())))))",
            "(0||CalendarData()((0||DaysOfWeek()((0||8()())))))",
        ] {
            let parsed = data(blob);
            assert!(parsed.data.is_some());
            assert!(matches!(
                parsed.warnings.as_slice(),
                [Warning::CalendarMalformed { .. }]
            ));
        }
    }

    #[test]
    fn omits_week_when_days_of_week_is_absent() {
        assert_eq!(
            data("(0||CalendarData()((0||Exceptions()())))")
                .data
                .expect("calendar data")
                .week,
            None
        );
    }

    #[test]
    fn non_ascii_unknown_name_does_not_panic() {
        let parsed = data("(0||Calendár()())");
        assert_eq!(
            parsed.warnings,
            vec![Warning::CalendarUnknownNode {
                clndr_id: 17,
                line: 9,
                name: "Calendár".to_owned(),
            }]
        );
    }
}
