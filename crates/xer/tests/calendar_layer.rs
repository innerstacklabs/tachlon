use xer::{
    CalendarException, CalendarType, Data, Error, Period, Warning, parse, parse_clndr_data, read,
};

const HEADER: &str =
    "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tProject Management\tUSD";
const CALENDAR_FIELDS: &str =
    "%F\tclndr_id\tclndr_name\tproj_id\tbase_clndr_id\tclndr_type\tday_hr_cnt\tclndr_data";

fn bytes(lines: &[&str], eol: &str) -> Vec<u8> {
    lines.join(eol).into_bytes()
}

fn typed(lines: &[&str], eol: &str) -> Data {
    let input = bytes(lines, eol);
    let xer = parse(&input).expect("the fixture parses");
    read(&xer).expect("the fixture reads")
}

fn calendars(rows: &[String], eol: &str) -> Data {
    let mut lines = vec![
        HEADER.to_owned(),
        "%T\tCALENDAR".to_owned(),
        CALENDAR_FIELDS.to_owned(),
    ];
    lines.extend_from_slice(rows);
    lines.push("%E".to_owned());
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    typed(&refs, eol)
}

fn calendar_row(
    id: u32,
    name: &str,
    project: &str,
    base: &str,
    kind: &str,
    hours: &str,
    blob: &str,
) -> String {
    format!("%R\t{id}\t{name}\t{project}\t{base}\t{kind}\t{hours}\t{blob}")
}

fn node(name: &str, attrs: &str, children: &str) -> String {
    format!("(0||{name}({attrs})({children}))")
}

fn period(start: &str, finish: &str) -> String {
    node("0", &format!("s|{start}|f|{finish}"), "")
}

fn day(number: u8, periods: &[(&str, &str)]) -> String {
    let children: String = periods
        .iter()
        .map(|(start, finish)| period(start, finish))
        .collect();
    node(&number.to_string(), "", &children)
}

fn days_node(days: &str) -> String {
    node("DaysOfWeek", "", days)
}

fn exceptions_node(exceptions: &str) -> String {
    node("Exceptions", "", exceptions)
}

fn calendar_blob(children: &str) -> String {
    node("CalendarData", "", children)
}

fn weekday_blob() -> String {
    let mut days = day(1, &[]);
    for number in 2..=6 {
        days.push_str(&day(number, &[("08:00", "12:00"), ("13:00", "17:00")]));
    }
    days.push_str(&day(7, &[]));
    calendar_blob(&format!("{}{}", days_node(&days), exceptions_node("")))
}

#[test]
fn monday_to_friday_week_and_typed_fields() {
    let blob = weekday_blob();
    let data = calendars(
        &[calendar_row(
            7, "Standard", "42", "3", "CA_Base", "8", &blob,
        )],
        "\n",
    );

    assert_eq!(data.calendar_rows.len(), 1);
    let calendar = &data.calendar_rows[0];
    assert_eq!(calendar.clndr_id, 7);
    assert_eq!(calendar.clndr_name.as_deref(), Some("Standard"));
    assert_eq!(calendar.proj_id, Some(42));
    assert_eq!(calendar.base_clndr_id, Some(3));
    assert_eq!(calendar.clndr_type, Some(CalendarType::Base));
    assert_eq!(calendar.day_hr_cnt, Some(8.0));

    let week = calendar.clndr_data.as_ref().unwrap().week.as_ref().unwrap();
    assert_eq!(week[0], []);
    assert_eq!(
        week[1],
        vec![
            Period {
                start: 8 * 60,
                finish: 12 * 60,
            },
            Period {
                start: 13 * 60,
                finish: 17 * 60,
            },
        ]
    );
    assert_eq!(week[6], []);
    assert_eq!(data.warnings(), []);
    assert!(!data.not_read.iter().any(|table| table.name == "CALENDAR"));
}

#[test]
fn holiday_exception_keeps_its_serial() {
    let exception = node("0", "d|43101", "");
    let blob = calendar_blob(&exceptions_node(&exception));
    let data = calendars(
        &[calendar_row(7, "Holidays", "", "", "CA_Base", "8", &blob)],
        "\n",
    );

    assert_eq!(
        data.calendar(7)
            .unwrap()
            .clndr_data
            .as_ref()
            .unwrap()
            .exceptions,
        vec![CalendarException {
            serial: 43101,
            periods: Vec::new(),
            unparsed_periods: 0,
        }]
    );
    assert_eq!(data.warnings(), []);
}

#[test]
fn shortened_hours_exception_keeps_its_periods() {
    let exception = node(
        "0",
        "d|43102",
        &format!("{}{}", period("09:00", "12:00"), period("13:00", "15:00")),
    );
    let blob = calendar_blob(&exceptions_node(&exception));
    let data = calendars(
        &[calendar_row(7, "Short day", "", "", "CA_Base", "5", &blob)],
        "\n",
    );

    assert_eq!(
        data.calendar(7)
            .unwrap()
            .clndr_data
            .as_ref()
            .unwrap()
            .exceptions,
        vec![CalendarException {
            serial: 43102,
            periods: vec![
                Period {
                    start: 9 * 60,
                    finish: 12 * 60,
                },
                Period {
                    start: 13 * 60,
                    finish: 15 * 60,
                },
            ],
            unparsed_periods: 0,
        }]
    );
    assert_eq!(data.warnings(), []);
}

#[test]
fn midnight_crossing_period_warns_naming_the_calendar() {
    let blob = calendar_blob(&days_node(&day(2, &[("17:00", "08:00")])));
    let data = calendars(
        &[calendar_row(
            7,
            "Night shift",
            "",
            "",
            "CA_Base",
            "8",
            &blob,
        )],
        "\n",
    );

    assert_eq!(
        data.warnings(),
        &[Warning::CalendarCrossesMidnight {
            clndr_id: 7,
            line: 4,
            scope: "day 2".into(),
        }]
    );
    let week = data
        .calendar(7)
        .unwrap()
        .clndr_data
        .as_ref()
        .unwrap()
        .week
        .as_ref()
        .unwrap();
    assert_eq!(week[1], []);
}

#[test]
fn overlap_and_unknown_node_warn() {
    let overlapping_day = day(2, &[("08:00", "12:00"), ("11:00", "13:00")]);
    let children = format!(
        "{}{}",
        days_node(&overlapping_day),
        node("FutureCalendarNode", "", "")
    );
    let blob = calendar_blob(&children);
    let data = calendars(
        &[calendar_row(9, "Warnings", "", "", "CA_Base", "8", &blob)],
        "\n",
    );

    assert!(data.warnings().contains(&Warning::CalendarOverlap {
        clndr_id: 9,
        line: 4,
        scope: "day 2".into(),
    }));
    assert!(data.warnings().contains(&Warning::CalendarUnknownNode {
        clndr_id: 9,
        line: 4,
        name: "FutureCalendarNode".into(),
    }));
    assert_eq!(
        data.calendar(9)
            .unwrap()
            .clndr_data
            .as_ref()
            .unwrap()
            .week
            .as_ref()
            .unwrap()[1],
        vec![Period {
            start: 8 * 60,
            finish: 12 * 60,
        }]
    );
}

#[test]
fn depth_64_reads_and_depth_65_is_an_error_naming_the_calendar() {
    fn nested(depth: usize) -> String {
        format!("{}{}", "(0||x()(".repeat(depth), "))".repeat(depth))
    }

    let depth_64 = nested(64);
    let data = calendars(
        &[calendar_row(7, "Deep", "", "", "CA_Base", "8", &depth_64)],
        "\n",
    );
    assert!(data.warnings().contains(&Warning::CalendarUnknownNode {
        clndr_id: 7,
        line: 4,
        name: "x".into(),
    }));

    let depth_65 = nested(65);
    let row = calendar_row(7, "Too deep", "", "", "CA_Base", "8", &depth_65);
    let input = bytes(&[HEADER, "%T\tCALENDAR", CALENDAR_FIELDS, &row, "%E"], "\n");
    let xer = parse(&input).unwrap();
    assert_eq!(
        read(&xer).unwrap_err(),
        Error::CalendarTooDeep {
            clndr_id: 7,
            line: 4,
        }
    );
}

#[test]
fn crlf_file_with_del_line_breaks_inside_the_blob() {
    // P6 writes a line break inside a value as two DEL bytes (0x7F 0x7F, unverified); a raw CRLF
    // inside a row would split the row, so the DEL form is the one that occurs in a file.
    let blob = weekday_blob();
    let del_blob = blob
        .replace("(0||", "\u{7f}\u{7f}(0||")
        .replace(")))", ")\u{7f}\u{7f}))");
    assert_ne!(blob, del_blob);

    let plain_row = calendar_row(7, "Plain", "", "", "CA_Base", "8", &blob);
    let del_row = calendar_row(7, "Breaks", "", "", "CA_Base", "8", &del_blob);
    let expected = typed(
        &[HEADER, "%T\tCALENDAR", CALENDAR_FIELDS, &plain_row, "%E"],
        "\r\n",
    );
    let expected_week = expected
        .calendar(7)
        .unwrap()
        .clndr_data
        .as_ref()
        .unwrap()
        .week
        .clone()
        .unwrap();

    for eol in ["\r\n", "\n"] {
        let input = bytes(
            &[HEADER, "%T\tCALENDAR", CALENDAR_FIELDS, &del_row, "%E"],
            eol,
        );
        let xer = parse(&input).unwrap();
        let data = read(&xer).unwrap();
        let calendar = data.calendar(7).unwrap();
        assert_eq!(calendar.clndr_name.as_deref(), Some("Breaks"));
        let week = calendar.clndr_data.as_ref().unwrap().week.as_ref().unwrap();
        assert_eq!(week, &expected_week);
        assert_eq!(week[1].len(), 2);
        assert!(week[0].is_empty() && week[6].is_empty());
        assert!(data.warnings().is_empty(), "no CalendarMalformed warning");
    }

    assert_eq!(
        parse_clndr_data(7, 4, &blob).unwrap(),
        parse_clndr_data(7, 4, &del_blob).unwrap()
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test walks each key-field error and the non-key warning in turn, on shared helpers"
)]
fn key_and_non_key_fields() {
    let bad_id = bytes(
        &[
            HEADER,
            "%T\tCALENDAR",
            CALENDAR_FIELDS,
            "%R\tabc\tBad id\t\t\tCA_Base\t8\t",
            "%E",
        ],
        "\n",
    );
    let xer = parse(&bad_id).unwrap();
    assert_eq!(
        read(&xer).unwrap_err(),
        Error::BadValue {
            table: "CALENDAR".into(),
            field: "clndr_id".into(),
            line: 4,
            value: "abc".into(),
        }
    );

    let missing_id = bytes(
        &[
            HEADER,
            "%T\tCALENDAR",
            "%F\tclndr_name\tclndr_data",
            "%R\tNo id\t",
            "%E",
        ],
        "\n",
    );
    let xer = parse(&missing_id).unwrap();
    assert_eq!(
        read(&xer).unwrap_err(),
        Error::MissingColumn {
            table: "CALENDAR".into(),
            field: "clndr_id".into(),
            line: 2,
        }
    );

    let bad_project = calendars(
        &[calendar_row(
            1,
            "Bad project",
            "abc",
            "",
            "CA_Base",
            "8",
            "",
        )],
        "\n",
    );
    assert_eq!(bad_project.calendar(1).unwrap().proj_id, None);
    assert_eq!(
        bad_project.warnings(),
        &[Warning::BadValue {
            table: "CALENDAR".into(),
            field: "proj_id".into(),
            line: 4,
            value: "abc".into(),
        }]
    );

    let empty_data = calendars(
        &[calendar_row(2, "No data", "", "", "CA_Base", "8", "")],
        "\n",
    );
    assert_eq!(empty_data.calendar(2).unwrap().clndr_data, None);
    assert_eq!(empty_data.warnings(), []);

    let duplicates = calendars(
        &[
            calendar_row(3, "First", "", "", "CA_Base", "8", ""),
            calendar_row(3, "Second", "", "", "CA_Base", "8", ""),
        ],
        "\n",
    );
    assert_eq!(
        duplicates.calendar(3).unwrap().clndr_name.as_deref(),
        Some("First")
    );
    assert_eq!(
        duplicates.warnings(),
        &[Warning::CalendarDuplicate {
            clndr_id: 3,
            line: 5,
        }]
    );
}

#[test]
fn unknown_calendar_type_is_other() {
    let data = calendars(
        &[calendar_row(7, "Future", "", "", "CA_New", "8", "")],
        "\n",
    );
    assert_eq!(
        data.calendar(7).unwrap().clndr_type,
        Some(CalendarType::Other("CA_New".into()))
    );
    assert_eq!(data.warnings(), []);
}
