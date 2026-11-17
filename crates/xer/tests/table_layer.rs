use xer::{Encoding, Error, Row, Warning, parse};

fn xer(lines: &[&str], eol: &str) -> Vec<u8> {
    lines.join(eol).into_bytes()
}

fn lf(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

fn assert_cell(row: &Row<'_>, column: usize, expected: &str) {
    assert_eq!(row.cell(column).as_deref(), Some(expected));
}

fn assert_error(bytes: &[u8], expected: Error) {
    assert_eq!(parse(bytes).err(), Some(expected));
}

#[test]
fn rule_01_minimal_valid_file() {
    let bytes = xer(
        &[
            "ERMHDR\t19.12",
            "%T\tTASK",
            "%F\ttask_id\ttask_name",
            "%R\t1\tDesign",
            "%R\t2\tBuild",
            "%E",
        ],
        "\r\n",
    );
    let parsed = parse(&bytes).unwrap();

    assert_eq!(parsed.encoding(), Encoding::Utf8);
    assert_cell(parsed.header(), 0, "19.12");
    assert_eq!(parsed.tables().len(), 1);
    let table = parsed.table("TASK").unwrap();
    assert_eq!(table.column("task_id"), Some(0));
    assert_eq!(table.column("task_name"), Some(1));
    assert_eq!(table.rows().len(), 2);
    assert_cell(&table.rows()[0], 0, "1");
    assert_cell(&table.rows()[0], 1, "Design");
    assert_eq!(table.rows()[1].get(table, "task_id").as_deref(), Some("2"));
    assert_eq!(
        table.rows()[1].get(table, "task_name").as_deref(),
        Some("Build")
    );
    assert_eq!(parsed.warnings(), &[]);
}

#[test]
fn rule_02_crlf_and_lf_are_identical() {
    let lines = [
        "ERMHDR\t19.12",
        "%T\tTASK",
        "%F\tid\tname",
        "%R\t1\tAlpha",
        "%R\t2",
        "%E",
    ];
    let crlf_bytes = xer(&lines, "\r\n");
    let lf_bytes = xer(&lines, "\n");
    let crlf = parse(&crlf_bytes).unwrap();
    let lf = parse(&lf_bytes).unwrap();

    assert_eq!(crlf.warnings(), lf.warnings());
    assert_eq!(crlf.tables().len(), lf.tables().len());
    for (left, right) in crlf.tables().iter().zip(lf.tables()) {
        assert_eq!(left.name(), right.name());
        assert_eq!(left.line(), right.line());
        assert_eq!(
            left.fields().collect::<Vec<_>>(),
            right.fields().collect::<Vec<_>>()
        );
        assert_eq!(left.rows().len(), right.rows().len());
        for (left_row, right_row) in left.rows().iter().zip(right.rows()) {
            assert_eq!(left_row.line(), right_row.line());
            assert_eq!(left_row.width(), right_row.width());
            for column in 0..left_row.width() {
                assert_eq!(left_row.cell(column), right_row.cell(column));
            }
        }
    }
}

#[test]
fn rule_03_cr_only_splits_but_lone_cr_in_lf_file_does_not() {
    let cr_bytes = xer(
        &["ERMHDR\t19.12", "%T\tTASK", "%F\tname", "%R\tAlpha", "%E"],
        "\r",
    );
    let cr = parse(&cr_bytes).unwrap();
    assert_eq!(cr.table("TASK").unwrap().rows()[0].line(), 4);
    assert_cell(&cr.table("TASK").unwrap().rows()[0], 0, "Alpha");

    let lf_bytes = xer(
        &[
            "ERMHDR\t19.12",
            "%T\tTASK",
            "%F\tname",
            "%R\tleft\rright",
            "%E",
        ],
        "\n",
    );
    let lf = parse(&lf_bytes).unwrap();
    assert_cell(&lf.table("TASK").unwrap().rows()[0], 0, "left\rright");
}

#[test]
fn rule_04_blank_lines_count_and_trailing_newline_adds_no_line() {
    let mut bytes = lf("ERMHDR\t19.12\n\n\n%T\tTASK\n%F\tid\n\n\n%R\t7\n%E");
    bytes.push(b'\n');
    let parsed = parse(&bytes).unwrap();

    assert_eq!(parsed.table("TASK").unwrap().line(), 4);
    assert_eq!(parsed.table("TASK").unwrap().rows()[0].line(), 8);
    assert_eq!(parsed.warnings(), &[]);
}

#[test]
fn rule_05_utf8_bom_is_skipped_before_header() {
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend(xer(&["ERMHDR\t19.12", "%E"], "\n"));
    let parsed = parse(&bytes).unwrap();

    assert_eq!(parsed.encoding(), Encoding::Utf8);
    assert_cell(parsed.header(), 0, "19.12");
    assert_eq!(parsed.warnings(), &[]);
}

#[test]
fn rule_06_valid_utf8_without_bom_decodes_non_ascii() {
    let bytes = lf("ERMHDR\t19.12\n%T\tTEXT\n%F\tvalue\n%R\tcafé — menu\n%E");
    let parsed = parse(&bytes).unwrap();

    assert_eq!(parsed.encoding(), Encoding::Utf8);
    assert_cell(&parsed.table("TEXT").unwrap().rows()[0], 0, "café — menu");
}

#[test]
fn rule_07_windows_1252_uses_the_whatwg_mapping() {
    let mut selected = b"ERMHDR\t19.12\n%T\tTEXT\n%F\tvalue\n%R\t".to_vec();
    selected.extend([0x80, 0x92, 0x93, 0x94, 0x96, 0x99, 0x81]);
    selected.extend(b"\n%E");
    let parsed = parse(&selected).unwrap();

    assert_eq!(parsed.encoding(), Encoding::Windows1252);
    assert_eq!(parsed.table("TEXT").unwrap().name(), "TEXT");
    assert_eq!(parsed.table("TEXT").unwrap().column("value"), Some(0));
    assert_cell(
        &parsed.table("TEXT").unwrap().rows()[0],
        0,
        "\u{20ac}\u{2019}\u{201c}\u{201d}\u{2013}\u{2122}\u{81}",
    );

    let mut all = b"ERMHDR\t19.12\n%T\tTEXT\n%F\tvalue\n%R\t".to_vec();
    all.extend(0x80..=0x9f);
    all.extend(b"\n%E");
    let parsed = parse(&all).unwrap();
    let expected = "\u{20ac}\u{81}\u{201a}\u{192}\u{201e}\u{2026}\u{2020}\u{2021}\
                    \u{2c6}\u{2030}\u{160}\u{2039}\u{152}\u{8d}\u{17d}\u{8f}\
                    \u{90}\u{2018}\u{2019}\u{201c}\u{201d}\u{2022}\u{2013}\u{2014}\
                    \u{2dc}\u{2122}\u{161}\u{203a}\u{153}\u{9d}\u{17e}\u{178}";
    assert_cell(&parsed.table("TEXT").unwrap().rows()[0], 0, expected);
}

#[test]
fn rule_08_missing_header_and_bare_header() {
    for bytes in [
        b"%R\tdata".as_slice(),
        b"".as_slice(),
        b"\n\r\n\n".as_slice(),
        b"ERMHDRX\t19.12".as_slice(),
    ] {
        assert_eq!(parse(bytes).err(), Some(Error::MissingHeader));
    }

    let parsed = parse(b"ERMHDR\n%E").unwrap();
    assert_eq!(parsed.header().width(), 0);
    assert_eq!(parsed.header().cell(0), None);
}

#[test]
fn rule_09_fields_or_rows_before_table_are_errors() {
    assert_error(
        b"ERMHDR\n\n%F\tid",
        Error::LineBeforeTable {
            line: 3,
            marker: 'F',
        },
    );
    assert_error(
        b"ERMHDR\n%R\t1",
        Error::LineBeforeTable {
            line: 2,
            marker: 'R',
        },
    );
}

#[test]
fn rule_10_row_before_fields_is_an_error() {
    assert_error(
        b"ERMHDR\n%T\tTASK\n%R\t1",
        Error::RowBeforeFields {
            table: "TASK".into(),
            line: 3,
        },
    );
}

#[test]
fn rule_11_unknown_markers_and_data_are_errors_with_bounded_text() {
    for (line, expected) in [
        ("%X\tfoo", "%X\tfoo"),
        ("1\t2\t3", "1\t2\t3"),
        ("%Tfoo", "%Tfoo"),
    ] {
        let bytes = xer(&["ERMHDR", line], "\n");
        assert_error(
            &bytes,
            Error::UnknownLine {
                line: 2,
                text: expected.to_owned(),
            },
        );
    }

    let long = format!("%X\t{}", "z".repeat(80));
    let bytes = xer(&["ERMHDR", &long], "\n");
    let error = parse(&bytes).err().unwrap();
    let expected = Error::UnknownLine {
        line: 2,
        text: format!("%X\t{}", "z".repeat(37)),
    };
    assert_eq!(error, expected);
    if let Error::UnknownLine { text, .. } = error {
        assert_eq!(text.chars().count(), 40);
    }
}

#[test]
fn rule_12_ragged_rows_warn_and_obey_table_width() {
    let bytes = lf("ERMHDR\n%T\tTASK\n%F\tid\tname\n%R\t1\n%R\t2\tBuild\textra\n%E");
    let parsed = parse(&bytes).unwrap();
    let long = Warning::LongRow {
        table: "TASK".into(),
        line: 5,
        cells: 3,
        fields: 2,
    };
    assert_eq!(parsed.warnings(), &[long]);

    let rows = parsed.table("TASK").unwrap().rows();
    assert_cell(&rows[0], 1, "");
    assert_eq!(rows[0].cell(2), None);
    assert_cell(&rows[1], 0, "2");
    assert_cell(&rows[1], 1, "Build");
    assert_eq!(rows[1].cell(2), None);
}

#[test]
fn rule_13_missing_end_warns_last_without_losing_content() {
    let without_end = xer(&["ERMHDR", "%T\tTASK", "%F\tid", "%R\t1"], "\n");
    let parsed = parse(&without_end).unwrap();
    assert_cell(&parsed.table("TASK").unwrap().rows()[0], 0, "1");
    assert_eq!(parsed.warnings().last(), Some(&Warning::MissingEnd));

    let with_end = xer(&["ERMHDR", "%T\tTASK", "%F\tid", "%R\t1", "%E"], "\n");
    assert_eq!(parse(&with_end).unwrap().warnings(), &[]);
}

#[test]
fn rule_14_content_after_end_warns_once_and_is_not_parsed() {
    let bytes = lf("ERMHDR\n%T\tTASK\n%F\tid\n%R\t1\n%E\n\ngarbage that is not a marker\n%R\t2");
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.warnings(), &[Warning::AfterEnd { line: 7 }]);
    assert_eq!(parsed.table("TASK").unwrap().len(), 1);

    let blank_only = xer(&["ERMHDR", "%E", "", ""], "\n");
    assert_eq!(parse(&blank_only).unwrap().warnings(), &[]);
}

#[test]
fn rule_15_duplicate_tables_require_equal_field_lists() {
    let equal =
        lf("ERMHDR\n%T\tTASK\n%F\tid\tname\n%R\t1\tAlpha\n%T\tTASK\n%F\tid\tname\n%R\t2\tBeta\n%E");
    let parsed = parse(&equal).unwrap();
    assert_eq!(parsed.tables().len(), 1);
    let table = parsed.table("TASK").unwrap();
    assert_eq!(table.line(), 2);
    assert_eq!(table.rows().len(), 2);
    assert_cell(&table.rows()[0], 0, "1");
    assert_cell(&table.rows()[1], 0, "2");
    assert_eq!(parsed.warnings(), &[]);

    let different = lf("ERMHDR\n%T\tTASK\n%F\tid\tname\n%R\t1\tAlpha\n%T\tTASK\n%F\tname\tid");
    assert_error(
        &different,
        Error::FieldsDiffer {
            table: "TASK".into(),
            line: 6,
        },
    );

    let repeated = xer(&["ERMHDR", "%T\tTASK", "%F\tid", "%F\tid"], "\n");
    assert_error(
        &repeated,
        Error::FieldsRepeated {
            table: "TASK".into(),
            line: 4,
        },
    );
}

#[test]
fn rule_15b_row_before_fields_in_a_repeated_table_occurrence_is_an_error() {
    let bytes = lf("ERMHDR\n%T\tTASK\n%F\tid\tname\n%R\t1\tA\n%T\tTASK\n%R\tB\t2\n%E");
    assert_error(
        &bytes,
        Error::RowBeforeFields {
            table: "TASK".into(),
            line: 6,
        },
    );
}

#[test]
fn rule_16_column_lookup_survives_field_reordering() {
    let bytes = lf(
        "ERMHDR\n%T\tFIRST\n%F\ttask_id\ttask_name\n%R\t7\tExcavate\n\
         %T\tSECOND\n%F\ttask_name\ttask_id\n%R\tExcavate\t7\n%E",
    );
    let parsed = parse(&bytes).unwrap();
    let first = parsed.table("FIRST").unwrap();
    let second = parsed.table("SECOND").unwrap();

    let first_name = first.rows()[0].get(first, "task_name");
    let second_name = second.rows()[0].get(second, "task_name");
    assert_eq!(first_name, second_name);
    assert_eq!(first_name.as_deref(), Some("Excavate"));
    assert_eq!(first.column("nope"), None);
    assert_eq!(first.column("Task_Name"), None);
}

#[test]
fn rule_17_table_order_is_first_appearance_order() {
    let bytes = lf("ERMHDR\n%T\tB\n%F\tid\n%T\tA\n%F\tid\n%T\tB\n%F\tid\n%E");
    let parsed = parse(&bytes).unwrap();

    let names: Vec<_> = parsed.tables().iter().map(xer::Table::name).collect();
    assert_eq!(names, ["B", "A"]);
    assert_eq!(parsed.table("A").unwrap().name(), "A");
    assert!(parsed.table("UNKNOWN").is_none());
}

#[test]
fn rule_18_all_prefixes_and_garbage_are_panic_free() {
    fn exercise(bytes: &[u8]) {
        if let Ok(parsed) = parse(bytes) {
            for table in parsed.tables() {
                for row in table.rows() {
                    for column in 0..row.width() {
                        let _ = row.cell(column);
                        let _ = row.raw_cell(column);
                    }
                }
            }
        }
    }

    let valid = lf("ERMHDR\t19.12\n%T\tTASK\n%F\tid\tname\n%R\t1\tAlpha\n%R\t2\tBeta\n%E");
    for length in 0..=valid.len() {
        exercise(&valid[..length]);
    }
    for garbage in [
        vec![0xff; 64],
        b"%".to_vec(),
        b"%T".to_vec(),
        b"%F\t".to_vec(),
        b"%R".to_vec(),
        b"\t\t\t".to_vec(),
        vec![0; 32],
    ] {
        exercise(&garbage);
    }

    let cut = valid.iter().rposition(|byte| *byte == b'B').unwrap() + 1;
    let truncated = parse(&valid[..cut]).unwrap();
    let rows = truncated.table("TASK").unwrap().rows();
    assert_cell(&rows[0], 1, "Alpha");
    assert_eq!(rows[0].line(), 4);
}

#[test]
fn rule_19_zero_fields_and_tabbed_row_warns_long() {
    let bytes = xer(&["ERMHDR", "%T\tEMPTY", "%F", "%R\t", "%E"], "\n");
    let parsed = parse(&bytes).unwrap();
    let table = parsed.table("EMPTY").unwrap();

    assert_eq!(table.field_count(), 0);
    assert_eq!(table.rows()[0].width(), 0);
    assert_eq!(table.rows()[0].cell(0), None);
    let warning = Warning::LongRow {
        table: "EMPTY".into(),
        line: 4,
        cells: 1,
        fields: 0,
    };
    assert_eq!(parsed.warnings(), &[warning]);
}

#[test]
fn rule_20_header_cells_exclude_the_ermhdr_marker() {
    let bytes = xer(&["ERMHDR\t19.12\t2024-01-02\tProject", "%E"], "\n");
    let parsed = parse(&bytes).unwrap();

    assert_cell(parsed.header(), 0, "19.12");
    assert_cell(parsed.header(), 2, "Project");
}
