//! The writer against the parser: what is written parses back, in the same encoding and line ending.

#![cfg(feature = "synth")]

use xer::synth::{Spec, write as synth_write};
use xer::{Cell, Encoding, LineEnding, WriteError, Writer, parse};

fn assert_same_cells(left: &xer::Xer<'_>, right: &xer::Xer<'_>) {
    assert_eq!(left.encoding(), right.encoding());
    assert_eq!(left.line_ending(), right.line_ending());
    assert_eq!(
        left.header().raw_cells().collect::<Vec<_>>(),
        right.header().raw_cells().collect::<Vec<_>>()
    );
    assert_eq!(left.tables().len(), right.tables().len());
    for (a, b) in left.tables().iter().zip(right.tables()) {
        assert_eq!(a.name(), b.name());
        assert_eq!(
            a.fields().collect::<Vec<_>>(),
            b.fields().collect::<Vec<_>>()
        );
        assert_eq!(a.len(), b.len(), "rows of {}", a.name());
        for (x, y) in a.rows().iter().zip(b.rows()) {
            assert_eq!(
                x.raw_cells().collect::<Vec<_>>(),
                y.raw_cells().collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn written_tables_parse_back_cell_for_cell() {
    let input = synth_write(&Spec::small(7));
    let parsed = parse(&input).unwrap();
    assert!(parsed.tables().iter().any(|table| table.len() > 1));

    let written = parsed.write().unwrap();
    let again = parse(&written).unwrap();

    assert_same_cells(&parsed, &again);
    assert_eq!(written, input);
}

#[test]
fn windows_1252_and_each_line_ending_come_back_as_they_went_in() {
    for ending in [LineEnding::Lf, LineEnding::CrLf, LineEnding::Cr] {
        let mut writer = Writer::new(Encoding::Windows1252, ending);
        writer.header(&[Cell::Text("19.12")]).unwrap();
        writer.table("TASK", &["task_name", "note"]).unwrap();
        writer
            .row(&[Cell::Text("caf\u{e9} \u{20ac}5"), Cell::Raw(b"\xe9\x80")])
            .unwrap();
        let bytes = writer.finish();

        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.encoding(), Encoding::Windows1252);
        assert_eq!(parsed.line_ending(), ending);
        assert_eq!(parsed.warnings(), []);
        let row = &parsed.table("TASK").unwrap().rows()[0];
        assert_eq!(row.cell(0).as_deref(), Some("caf\u{e9} \u{20ac}5"));
        assert_eq!(row.raw_cell(0), Some(&b"caf\xe9 \x805"[..]));
        assert_eq!(row.raw_cell(1), Some(&b"\xe9\x80"[..]));

        assert_eq!(parsed.write().unwrap(), bytes);
    }
}

#[test]
fn a_character_the_encoding_cannot_hold_is_refused() {
    let mut writer = Writer::new(Encoding::Windows1252, LineEnding::Lf);
    writer.header(&[Cell::Text("19.12")]).unwrap();
    writer.table("TASK", &["task_name"]).unwrap();
    let refused = writer.row(&[Cell::Text("a \u{2192} b")]);
    assert_eq!(
        refused,
        Err(WriteError::Unencodable {
            character: '\u{2192}',
            encoding: Encoding::Windows1252
        })
    );
    // A refused row leaves nothing behind.
    let parsed_bytes = writer.finish();
    let parsed = parse(&parsed_bytes).unwrap();
    assert_eq!(parsed.table("TASK").unwrap().len(), 0);

    assert_eq!(Encoding::Windows1252.encode("\u{2192}"), Err('\u{2192}'));
    assert_eq!(
        Encoding::Utf8.encode("\u{2192}").unwrap().as_ref(),
        "\u{2192}".as_bytes()
    );
}

#[test]
fn the_parser_reports_the_line_ending() {
    let cases: [(&[u8], LineEnding); 4] = [
        (b"ERMHDR\t1\n%E\n", LineEnding::Lf),
        (b"ERMHDR\t1\r\n%E\r\n", LineEnding::CrLf),
        (b"ERMHDR\t1\r%E\r", LineEnding::Cr),
        (b"ERMHDR\t1", LineEnding::Lf),
    ];
    for (bytes, expected) in cases {
        assert_eq!(parse(bytes).unwrap().line_ending(), expected, "{bytes:?}");
    }
}

#[test]
fn a_cell_that_would_break_the_line_is_refused() {
    let mut writer = Writer::new(Encoding::Utf8, LineEnding::Lf);
    writer.table("T", &["a"]).unwrap();
    for cell in [Cell::Text("a\tb"), Cell::Text("a\nb"), Cell::Raw(b"a\rb")] {
        assert_eq!(writer.row(&[cell]), Err(WriteError::Delimiter));
    }
}

#[test]
fn a_byte_order_mark_is_kept_so_invalid_utf8_is_not_reread_as_windows_1252() {
    let input = b"\xef\xbb\xbfERMHDR\t1\n%T\tT\n%F\tv\n%R\tcaf\xc3\xa9\xff\n%E\n";
    let parsed = parse(input).unwrap();
    assert_eq!(parsed.encoding(), Encoding::Utf8);
    assert!(parsed.byte_order_mark());
    let before = parsed.table("T").unwrap().rows()[0]
        .cell(0)
        .unwrap()
        .into_owned();
    assert_eq!(before, "caf\u{e9}\u{fffd}");

    let written = parsed.write().unwrap();
    assert_eq!(written, input);
    let again = parse(&written).unwrap();
    assert_eq!(again.encoding(), Encoding::Utf8);
    assert_eq!(
        again.table("T").unwrap().rows()[0].cell(0).as_deref(),
        Some(before.as_str())
    );

    // Without a mark the reader reports none, and none is written.
    let plain = parse(b"ERMHDR\t1\n%E\n").unwrap();
    assert!(!plain.byte_order_mark());
    assert!(!plain.write().unwrap().starts_with(&[0xef]));
    // A mark means nothing in Windows-1252.
    let bytes = Writer::new(Encoding::Windows1252, LineEnding::Lf)
        .byte_order_mark()
        .finish();
    assert_eq!(bytes, b"%E\n");
}
