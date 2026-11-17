//! `load_bytes` over bytes: the content selects the importer (gs#81 PR 6).

use app::{Loaded, is_xer, load_bytes};
use engine::TaskId;
use import::ProjectSelection;
use xer::Encoding;
use xer::synth::{LineEnding, Spec, write};

const SUMMARY: &str = include_str!("../../import/tests/fixtures/summary.xml");
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Open every project the file holds, as the page does today.
fn load_all(bytes: &[u8]) -> Result<Loaded, String> {
    load_bytes(bytes, &ProjectSelection::All, &|| 0.0).map_err(|error| error.to_string())
}

fn with_bom(bytes: &[u8]) -> Vec<u8> {
    let mut out = BOM.to_vec();
    out.extend_from_slice(bytes);
    out
}

#[test]
fn ermhdr_at_the_start_selects_xer_after_an_optional_bom() {
    assert!(is_xer(b"ERMHDR\t20.12"));
    assert!(is_xer(b"\xEF\xBB\xBFERMHDR\t20.12"));
    assert!(is_xer(b"ERMHDR"));
    assert!(!is_xer(b""));
    assert!(!is_xer(b"<?xml version=\"1.0\"?>"));
    assert!(!is_xer(b"\xEF\xBB\xBF<Project>"));
    assert!(!is_xer(b"\n ERMHDR\t20.12"));
    assert!(!is_xer(b"ermhdr"));
    assert!(!is_xer(b"ERMHD"));
}

#[test]
fn a_small_xer_loads_with_its_outline_and_a_solved_schedule() {
    let spec = Spec::small(7);
    let loaded = load_all(&write(&spec)).expect("a generated XER should load");
    let counts = spec.counts();

    assert_eq!(loaded.solution.dates.len(), loaded.schedule.tasks().len());
    assert_eq!(loaded.order.len(), loaded.schedule.tasks().len());
    assert_eq!(loaded.meta.len(), loaded.schedule.tasks().len());
    assert_eq!(loaded.schedule.dependencies().len(), counts.links);
    let leaves = (0..loaded.schedule.tasks().len())
        .filter(|index| !loaded.outline.is_summary(TaskId(*index as u32)))
        .count();
    assert_eq!(leaves, counts.activities);
    assert!(loaded.outline.is_summary(TaskId(0)));
    assert!(loaded.meta[0].parent.is_none());
}

#[test]
fn an_xer_loads_the_same_through_load_bytes_as_through_the_importer() {
    let bytes = write(&Spec::small(3));
    let direct = import::parse_xer(&bytes, &import::ProjectSelection::All).expect("importer");
    let loaded = load_all(&bytes).expect("a generated XER should load");

    assert_eq!(loaded.schedule.tasks().len(), direct.tasks.len());
    assert_eq!(
        loaded.schedule.dependencies().len(),
        direct.schedule.dependencies().len()
    );
    // `load_bytes` adds the file-dates line to the importer's notes and nothing else.
    let without_file_dates: Vec<_> = loaded
        .unsupported
        .iter()
        .filter(|line| !line.starts_with("in-progress activities:"))
        .cloned()
        .collect();
    assert_eq!(without_file_dates, direct.unsupported);
    assert_eq!(loaded.data_date, direct.data_date);
}

#[test]
fn a_utf8_byte_order_mark_does_not_change_an_xer_load() {
    let bytes = write(&Spec::small(5));
    let plain = load_all(&bytes).expect("plain XER");
    let marked = load_all(&with_bom(&bytes)).expect("XER with a BOM");

    assert_eq!(plain.schedule.tasks().len(), marked.schedule.tasks().len());
    assert_eq!(plain.solution.dates.len(), marked.solution.dates.len());
}

#[test]
fn a_windows_1252_crlf_xer_loads() {
    let spec = Spec {
        encoding: Encoding::Windows1252,
        line_ending: LineEnding::CrLf,
        ..Spec::small(11)
    };
    let loaded = load_all(&write(&spec)).expect("a Windows-1252 XER should load");
    assert!(
        loaded
            .unsupported
            .iter()
            .any(|line| line.contains("not saved as UTF-8"))
    );
}

#[test]
fn a_multi_project_xer_opens_every_project() {
    let spec = Spec {
        projects: 3,
        cross_project_links: true,
        ..Spec::small(9)
    };
    let loaded = load_all(&write(&spec)).expect("a multi-project XER opens with all");
    let roots = loaded
        .meta
        .iter()
        .filter(|meta| meta.parent.is_none())
        .count();
    assert_eq!(roots, 3);
    let leaves = (0..loaded.schedule.tasks().len())
        .filter(|index| !loaded.outline.is_summary(TaskId(*index as u32)))
        .count();
    assert_eq!(leaves, spec.counts().activities);
    assert_eq!(loaded.schedule.dependencies().len(), spec.counts().links);
}

#[test]
fn a_file_that_is_neither_format_is_a_readable_error() {
    for bytes in [
        &b""[..],
        b"hello",
        b"\n ERMHDR\t20.12\n",
        b"\xFF\xFE<\0P\0r\0o\0j\0e\0c\0t\0>\0",
        b"\x00\x01\x02\x03",
    ] {
        let Err(message) = load_all(bytes) else {
            panic!("{bytes:?} is neither XER nor MSPDI");
        };
        assert_ne!(message.trim(), "");
    }
}

#[test]
fn a_header_with_no_tables_is_the_xer_reader_error_not_an_xml_error() {
    let Err(message) = load_all(b"ERMHDR\t20.12\n") else {
        panic!("a header alone has no projects");
    };
    assert!(
        !message.starts_with("XML error"),
        "an XER-shaped file must not reach the XML parser: {message}"
    );
}

#[test]
fn every_truncation_of_an_xer_is_an_error_or_a_schedule_never_a_panic() {
    let bytes = write(&Spec::small(13));
    let step = (bytes.len() / 400).max(1);
    let mut cut = 0;
    while cut < bytes.len() {
        if let Err(message) = load_all(&bytes[..cut]) {
            assert!(!message.trim().is_empty(), "empty error at {cut} bytes");
        }
        cut += step;
    }
}

#[test]
fn mspdi_bytes_load_as_the_text_loaded_before() {
    let loaded = load_all(SUMMARY.as_bytes()).expect("the fixture should load");
    let marked = load_all(&with_bom(SUMMARY.as_bytes())).expect("MSPDI with a BOM");
    assert_eq!(loaded.schedule.tasks().len(), marked.schedule.tasks().len());
    assert_eq!(loaded.meta[1].name, marked.meta[1].name);
    assert_eq!(loaded.unsupported, marked.unsupported);
}

#[test]
fn an_invalid_utf8_byte_in_mspdi_becomes_a_replacement_character() {
    let patched = SUMMARY.replacen("<Name>First child</Name>", "<Name>First Xchild</Name>", 1);
    let mut bytes = patched.into_bytes();
    let at = bytes
        .windows(7)
        .position(|window| window == b"First X")
        .expect("the patched name is present");
    bytes[at + 6] = 0xFF;
    let loaded = load_all(&bytes).expect("an invalid byte is replaced, not rejected");
    assert!(loaded.meta[1].name.contains('\u{FFFD}'));
}

/// Parse, map and solve phases at 50,000 activities. Prints; asserts no timing.
///
/// Run: `cargo test --release -p app --test it -- --ignored --nocapture xer::timing`
#[test]
#[ignore = "prints timings at 50,000 activities; run by hand"]
fn timing_of_the_phases_at_50k_activities() {
    use std::time::Instant;

    let spec = Spec::with_activities(42, 50_000);
    let generated = Instant::now();
    let bytes = write(&spec);
    let generate = generated.elapsed();

    let started = Instant::now();
    let parsed = xer::parse(&bytes).expect("the generated XER parses");
    let data = xer::read(&parsed).expect("the generated XER reads");
    let parse = started.elapsed();
    drop((parsed, data));

    let started = Instant::now();
    let mut imported = import::parse_xer(&bytes, &import::ProjectSelection::All)
        .expect("the generated XER imports");
    let parse_and_map = started.elapsed();

    let started = Instant::now();
    let solution = imported.schedule.compute().expect("the schedule solves");
    let solve = started.elapsed();

    let started = Instant::now();
    let loaded = load_all(&bytes).expect("the generated XER loads");
    let load = started.elapsed();

    assert_eq!(solution.dates.len(), loaded.solution.dates.len());
    println!(
        "xer 50k: {} bytes, {} tasks (generate {:?})",
        bytes.len(),
        loaded.schedule.tasks().len(),
        generate
    );
    println!("  parse (xer::parse + xer::read): {parse:?}");
    println!(
        "  map (parse_xer minus parse):    {:?}",
        parse_and_map.saturating_sub(parse)
    );
    println!("  parse_xer total:                {parse_and_map:?}");
    println!("  solve (Schedule::compute):      {solve:?}");
    println!("  load_bytes (all, incl. outline and order): {load:?}");
}
