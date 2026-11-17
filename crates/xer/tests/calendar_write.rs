use xer::{
    CalendarData, CalendarException, Period, Week, parse, parse_clndr_data, read, write_clndr_data,
};

fn node(name: &str, attrs: &str, children: &str) -> String {
    format!("(0||{name}({attrs})({children}))")
}

fn period(start: &str, finish: &str) -> String {
    node("0", &format!("s|{start}|f|{finish}"), "")
}

fn day(number: u8, periods: &[(&str, &str)]) -> String {
    let children: String = periods.iter().map(|(s, f)| period(s, f)).collect();
    node(&number.to_string(), "", &children)
}

fn calendar_blob(children: &str) -> String {
    node("CalendarData", "", children)
}

fn exception_blob(children: &str) -> String {
    let exception = node("0", "d|43101", children);
    calendar_blob(&node("Exceptions", "", &exception))
}

fn weekday_blob() -> String {
    let mut days = day(1, &[]);
    for number in 2..=6 {
        days.push_str(&day(number, &[("08:00", "12:00"), ("13:00", "17:00")]));
    }
    days.push_str(&day(7, &[]));
    calendar_blob(&format!(
        "{}{}",
        node("DaysOfWeek", "", &days),
        node("Exceptions", "", "")
    ))
}

/// The blobs the two existing calendar test files read, rebuilt here.
fn blobs() -> Vec<String> {
    let weekday = weekday_blob();
    let del = weekday
        .replace("(0||", "\u{7f}\u{7f}(0||")
        .replace(")))", ")\u{7f}\u{7f}))");
    let both = format!(
        "{}{}",
        node("DaysOfWeek", "", &day(2, &[("08:00", "12:00")])),
        node("Exceptions", "", &node("0", "d|-5", ""))
    );
    vec![
        weekday,
        del,
        calendar_blob(&both),
        exception_blob(""),
        exception_blob(&format!(
            "{}{}",
            period("09:00", "12:00"),
            period("13:00", "15:00")
        )),
        calendar_blob(&node("DaysOfWeek", "", &day(2, &[("22:00", "02:00")]))),
        calendar_blob(&node(
            "DaysOfWeek",
            "",
            &format!(
                "{}{}",
                day(1, &[("23:00", "00:00")]),
                day(2, &[("00:00", "00:00")])
            ),
        )),
        calendar_blob(&node(
            "DaysOfWeek",
            "",
            &day(2, &[("08:00", "12:00"), ("11:00", "13:00")]),
        )),
        calendar_blob(&node("Mystery", "", "")),
        exception_blob(&format!(
            "{}{}",
            period("9am", "12:00"),
            period("13:00", "99:99")
        )),
        exception_blob(&format!(
            "{}{}{}",
            period("09:00", "12:00"),
            period("nope", "13:00"),
            period("17:00", "08:00")
        )),
        exception_blob(&format!(
            "{}{}",
            period("09:00", "12:00"),
            period("10:00", "13:00")
        )),
        "(0||CalendarData()((0||Exceptions()())))".to_owned(),
    ]
}

#[test]
fn parse_of_the_encoded_text_equals_the_data() {
    for blob in blobs() {
        let data = parse_clndr_data(7, 3, &blob).unwrap().data.unwrap();
        let text = write_clndr_data(&data);
        let again = parse_clndr_data(7, 3, &text).unwrap();
        assert_eq!(again.data, Some(data), "{blob:?} -> {text:?}");
    }
}

#[test]
fn a_whole_day_exception_and_a_working_saturday_survive() {
    let mut week: Week = std::array::from_fn(|_| Vec::new());
    week[6] = vec![Period {
        start: 8 * 60,
        finish: 12 * 60,
    }];
    week[1] = vec![Period {
        start: 0,
        finish: 1_440,
    }];
    let data = CalendarData {
        week: Some(week),
        exceptions: vec![
            CalendarException {
                serial: 43_101,
                periods: Vec::new(),
                unparsed_periods: 0,
            },
            CalendarException {
                serial: 43_102,
                periods: vec![Period {
                    start: 9 * 60,
                    finish: 1_440,
                }],
                unparsed_periods: 0,
            },
        ],
    };
    let text = write_clndr_data(&data);
    let parsed = parse_clndr_data(1, 1, &text).unwrap();
    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
    let back = parsed.data.unwrap();
    assert_eq!(back, data);
    assert!(
        back.week.as_ref().unwrap()[0].is_empty(),
        "Sunday stays off"
    );
    assert_eq!(back.week.unwrap()[6].len(), 1, "Saturday stays working");
    assert_eq!(back.exceptions[0].periods, [] as [Period; 0]);
    assert_eq!(back.exceptions[0].unparsed_periods, 0);
}

#[test]
fn an_absent_week_and_no_exceptions_write_an_empty_calendar() {
    let data = CalendarData {
        week: None,
        exceptions: Vec::new(),
    };
    let text = write_clndr_data(&data);
    assert_eq!(parse_clndr_data(1, 1, &text).unwrap().data, Some(data));
}

#[test]
fn an_unreadable_exception_is_not_turned_into_a_holiday() {
    let data = CalendarData {
        week: None,
        exceptions: vec![
            CalendarException {
                serial: 43_101,
                periods: Vec::new(),
                unparsed_periods: 2,
            },
            CalendarException {
                serial: 43_102,
                periods: vec![Period {
                    start: 540,
                    finish: 720,
                }],
                unparsed_periods: 1,
            },
        ],
    };
    let text = write_clndr_data(&data);
    assert_eq!(parse_clndr_data(1, 1, &text).unwrap().data, Some(data));
}

#[test]
fn line_breaks_are_del_pairs_and_the_value_survives_an_xer_row() {
    let mut week: Week = std::array::from_fn(|_| Vec::new());
    week[1] = vec![Period {
        start: 480,
        finish: 1_020,
    }];
    let data = CalendarData {
        week: Some(week),
        exceptions: vec![CalendarException {
            serial: 43_101,
            periods: Vec::new(),
            unparsed_periods: 0,
        }],
    };
    let text = write_clndr_data(&data);
    assert!(!text.contains(['\r', '\n', '\t']), "{text:?}");
    assert!(text.contains("\u{7f}\u{7f}  (0||DaysOfWeek"), "{text:?}");
    assert_eq!(text.matches('\u{7f}').count() % 2, 0);
    assert!(!text.replace("\u{7f}\u{7f}", "").contains('\u{7f}'));

    let row = format!("%R\t7\tWeek\t\t\tCA_Base\t8\t{text}");
    for eol in ["\r\n", "\n"] {
        let file = [
            "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tProject Management\tUSD",
            "%T\tCALENDAR",
            "%F\tclndr_id\tclndr_name\tproj_id\tbase_clndr_id\tclndr_type\tday_hr_cnt\tclndr_data",
            &row,
            "%E",
        ]
        .join(eol);
        let xer = parse(file.as_bytes()).unwrap();
        let typed = read(&xer).unwrap();
        assert!(typed.warnings().is_empty(), "{:?}", typed.warnings());
        assert_eq!(typed.calendar(7).unwrap().clndr_data, Some(data.clone()));
    }
}
