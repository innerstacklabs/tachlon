use xer::{CalendarException, Period, Warning, parse_clndr_data};

fn period(start: &str, finish: &str) -> String {
    format!("(0||0(s|{start}|f|{finish})())")
}

/// The one exception (day 43101, 2018-01-01) read from a blob whose single
/// exception holds `children`, with the warnings the read produced.
fn read(children: &str) -> (Vec<CalendarException>, Vec<Warning>) {
    let blob = format!("(0||CalendarData()((0||Exceptions()((0||0(d|43101)({children}))))))");
    let parsed = parse_clndr_data(7, 3, &blob).expect("the blob parses");
    (
        parsed
            .data
            .expect("the blob holds calendar data")
            .exceptions,
        parsed.warnings,
    )
}

#[test]
fn exception_with_only_unparsable_periods_is_counted_not_a_holiday() {
    let children = format!("{}{}", period("9am", "12:00"), period("13:00", "99:99"));
    let (exceptions, warnings) = read(&children);
    assert_eq!(exceptions.len(), 1);
    assert_eq!(exceptions[0].periods, []);
    assert_eq!(exceptions[0].unparsed_periods, 2);
    assert_eq!(exceptions[0].date(), "2018-01-01");
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}

#[test]
fn a_whole_day_exception_has_no_unparsed_periods() {
    let (exceptions, warnings) = read("");
    assert_eq!(exceptions[0].periods, []);
    assert_eq!(exceptions[0].unparsed_periods, 0);
    assert_eq!(warnings, []);
}

#[test]
fn partly_parsable_exception_keeps_its_parsable_periods() {
    let children = format!(
        "{}{}{}",
        period("09:00", "12:00"),
        period("nope", "13:00"),
        period("17:00", "08:00")
    );
    let (exceptions, warnings) = read(&children);
    assert_eq!(
        exceptions[0].periods,
        vec![Period {
            start: 9 * 60,
            finish: 12 * 60,
        }]
    );
    assert_eq!(exceptions[0].unparsed_periods, 2);
    assert_eq!(
        warnings,
        vec![
            Warning::CalendarMalformed {
                clndr_id: 7,
                line: 3,
                detail: "bad start time \"nope\" in exception 43101".to_string(),
            },
            Warning::CalendarCrossesMidnight {
                clndr_id: 7,
                line: 3,
                scope: "exception 43101".to_string(),
            },
        ]
    );
}

#[test]
fn overlapping_periods_are_not_counted_as_unparsed() {
    let children = format!("{}{}", period("09:00", "12:00"), period("10:00", "13:00"));
    let (exceptions, _) = read(&children);
    assert_eq!(exceptions[0].periods.len(), 1);
    assert_eq!(exceptions[0].unparsed_periods, 0);
}

#[test]
fn malformed_period_text_never_panics() {
    for (start, finish) in [
        ("", ""),
        ("é:é", "ñ:ñ"),
        ("9:0", "12:000"),
        ("-1:00", "+5:00"),
        ("24:01", "25:00"),
        ("09:60", "00:00"),
        ("\u{1F600}\u{1F600}", "\u{1F600}"),
    ] {
        let (exceptions, _) = read(&period(start, finish));
        assert_eq!(exceptions.len(), 1, "{start:?} {finish:?}");
        assert!(exceptions[0].periods.is_empty(), "{start:?} {finish:?}");
        assert_eq!(exceptions[0].unparsed_periods, 1, "{start:?} {finish:?}");
    }
}

#[test]
fn exception_date_matches_known_serials() {
    let on = |serial| CalendarException {
        serial,
        periods: Vec::new(),
        unparsed_periods: 0,
    };
    for (serial, date) in [
        (0, "1899-12-30"),
        (25_569, "1970-01-01"),
        (43_650, "2019-07-04"),
        (45_292, "2024-01-01"),
        (45_351, "2024-02-29"),
        (-1, "1899-12-29"),
    ] {
        assert_eq!(on(serial).date(), date);
    }
    for serial in [i32::MIN, i32::MAX] {
        assert_ne!(on(serial).date(), "");
    }
}
