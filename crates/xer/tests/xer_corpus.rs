//! Round-trip tests for `xer::synth` and a corpus run over public sample files.
//!
//! `XER_CORPUS_DIR=<dir> cargo test -p xer --features synth --test xer_corpus -- --nocapture`
//! parses and reads every `.xer` file under `<dir>` and prints what it found. No sample file is
//! committed; the directory is whatever the person running the test points at.
//!
//! The corpus run fails on a panic, on a root directory that cannot be listed, and on a root with
//! no `.xer` file. It does not fail on a parse error or a read error: those are printed in the
//! report and are the findings the run exists to collect.
#![cfg(feature = "synth")]

use std::collections::BTreeMap;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Instant;

use xer::synth::{LineEnding, Spec, write};
use xer::{Data, Encoding, PredType, parse, read};

/// The largest corpus file the test reads, so a stray multi-gigabyte file cannot exhaust memory.
const CORPUS_FILE_LIMIT: u64 = 256 * 1024 * 1024;

fn round_trip(spec: &Spec) -> Data {
    let bytes = write(spec);
    let xer = parse(&bytes).expect("generated file parses");
    assert_eq!(xer.encoding(), spec.encoding);
    let data = read(&xer).expect("generated file reads");
    assert_eq!(data.warnings(), &[], "generated file has no warnings");

    let counts = spec.counts();
    assert_eq!(data.project_rows.len(), counts.projects);
    assert_eq!(data.task_rows.len(), counts.activities);
    assert_eq!(data.wbs_rows.len(), counts.wbs);
    assert_eq!(data.pred_rows.len(), counts.links);
    assert_eq!(data.calendar_rows.len(), counts.calendars);
    assert_eq!(data.sched_options_rows.len(), counts.projects);
    assert_eq!(data.header.version.as_deref(), Some("19.12"));
    assert_eq!(data.header.currency.as_deref(), Some("USD"));

    let projects = data.projects();
    assert_eq!(projects.len(), counts.projects);
    for summary in &projects {
        assert_eq!(summary.activity_count, spec.activities);
        assert!(summary.exported);
    }
    for project in &data.project_rows {
        let calendar = project.clndr_id.expect("project calendar");
        let own = data.calendar(calendar).expect("calendar row");
        let exceptions = own.clndr_data.as_ref().expect("blob").exceptions.len();
        assert_eq!(exceptions, 3);
    }
    for table in ["TASKRSRC", "CURRTYPE"] {
        assert!(data.not_read.iter().any(|not_read| not_read.name == table));
    }
    for table in ["ACTVTYPE", "ACTVCODE", "TASKACTV"] {
        assert!(!data.not_read.iter().any(|not_read| not_read.name == table));
    }
    // The synthetic file has one code type with six values, and one assignment per activity.
    assert_eq!(data.actv_type_rows.len(), 1);
    assert_eq!(data.actv_code_rows.len(), 6);
    assert_eq!(data.task_actv_rows.len(), counts.activities);
    data
}

#[test]
fn small_file_round_trips() {
    let data = round_trip(&Spec::small(1));
    assert_eq!(data.task_rows.len(), 40);
    assert!(
        data.pred_rows
            .iter()
            .any(|pred| pred.pred_type != Some(PredType::FinishToStart))
    );
    assert!(
        data.pred_rows
            .iter()
            .any(|pred| pred.lag_hr_cnt.is_some_and(|lag| lag < 0.0))
    );
    assert!(data.task_rows.iter().any(|task| task.cstr_date.is_some()));
}

#[test]
fn output_is_a_pure_function_of_the_spec() {
    assert_eq!(write(&Spec::small(7)), write(&Spec::small(7)));
    assert_ne!(write(&Spec::small(7)), write(&Spec::small(8)));
}

#[test]
fn windows_1252_and_crlf_round_trip() {
    let spec = Spec {
        encoding: Encoding::Windows1252,
        line_ending: LineEnding::CrLf,
        ..Spec::small(3)
    };
    let data = round_trip(&spec);
    assert_eq!(
        data.project_rows[0].proj_short_name.as_deref(),
        Some("Caf\u{e9} Project 1")
    );
}

#[test]
fn multi_project_file_with_cross_project_links_round_trips() {
    let spec = Spec {
        projects: 3,
        cross_project_links: true,
        ..Spec::with_activities(11, 60)
    };
    let data = round_trip(&spec);
    let crossing = data
        .pred_rows
        .iter()
        .filter(|pred| pred.proj_id != pred.pred_proj_id)
        .count();
    assert_eq!(crossing, 2);
}

#[test]
fn empty_and_tiny_specs_do_not_panic() {
    for activities in 0..4 {
        for projects in 0..3 {
            let spec = Spec {
                projects,
                cross_project_links: true,
                ..Spec::with_activities(5, activities)
            };
            round_trip(&spec);
        }
    }
}

#[test]
fn five_thousand_activities_round_trip() {
    round_trip(&Spec::with_activities(42, 5_000));
}

/// Run once with `cargo test -p xer --features synth --test xer_corpus -- --ignored --nocapture` (the test profile, not `--release`).
#[test]
#[ignore = "50,000 activities; run on request"]
fn fifty_thousand_activities_round_trip() {
    let started = Instant::now();
    let bytes = write(&Spec::with_activities(42, 50_000));
    let written = started.elapsed();
    let xer = parse(&bytes).expect("parses");
    let parsed = started.elapsed();
    let data = read(&xer).expect("reads");
    println!(
        "50,000 activities: {} bytes, write {written:?}, parse {:?}, read {:?}",
        bytes.len(),
        parsed.checked_sub(written).unwrap(),
        started.elapsed().checked_sub(parsed).unwrap()
    );
    assert_eq!(data.task_rows.len(), 50_000);
    assert_eq!(data.warnings(), &[]);
}

fn find_xer_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `DirEntry::file_type` does not follow symlinks, so a link to a device or FIFO is skipped.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            find_xer_files(&path, found);
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("xer"))
        {
            found.push(path);
        }
    }
}

/// Parses and reads one file and prints a report; returns false if the library panicked.
#[expect(
    clippy::print_stdout,
    reason = "the corpus run prints one report line per file for the person running it"
)]
fn report(path: &Path) -> bool {
    let size = fs::metadata(path).map_or(0, |meta| meta.len());
    if size > CORPUS_FILE_LIMIT {
        println!(
            "{}: skipped, {size} bytes is over the limit",
            path.display()
        );
        return true;
    }
    let Ok(bytes) = fs::read(path) else {
        println!("{}: unreadable", path.display());
        return true;
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let xer = match parse(&bytes) {
            Ok(xer) => xer,
            Err(error) => return format!("parse error: {error}"),
        };
        let tables: Vec<String> = xer
            .tables()
            .iter()
            .map(|table| format!("{}:{}", table.name(), table.len()))
            .collect();
        let header: Vec<String> = (0..xer.header().width())
            .map(|cell| xer.header().cell(cell).unwrap_or_default().into_owned())
            .collect();
        let mut text = format!(
            "{} bytes, {}, ERMHDR [{}], tables {}",
            bytes.len(),
            xer.encoding().name(),
            header.join(" | "),
            tables.join(" ")
        );
        match read(&xer) {
            Err(error) => text = format!("{text}\n    read error: {error}"),
            Ok(data) => {
                text = format!(
                    "{text}\n    {} projects, {} activities, {} links, {} calendars, {} warnings",
                    data.project_rows.len(),
                    data.task_rows.len(),
                    data.pred_rows.len(),
                    data.calendar_rows.len(),
                    data.warnings().len()
                );
                let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
                for warning in data.warnings() {
                    let line = warning.to_string();
                    let kind = line
                        .split_once(": ")
                        .map_or(line.as_str(), |(_, rest)| rest);
                    let kind: String = kind.chars().take(60).collect();
                    *kinds.entry(kind).or_default() += 1;
                }
                for (kind, count) in kinds.iter().take(5) {
                    text = format!("{text}\n    warning x{count}: {kind}");
                }
            }
        }
        text
    }));
    if let Ok(text) = outcome {
        println!("{}: {text}", path.display());
        true
    } else {
        println!("{}: PANIC", path.display());
        false
    }
}

#[test]
fn corpus_files_parse_and_read_without_a_panic() {
    let Some(dir) = std::env::var_os("XER_CORPUS_DIR") else {
        println!("XER_CORPUS_DIR is not set; the corpus run is skipped");
        return;
    };
    // A mistyped path must not go green having read nothing, so the root is checked here;
    // `find_xer_files` skips an unreadable subdirectory.
    if let Err(error) = fs::read_dir(&dir) {
        panic!(
            "XER_CORPUS_DIR {} cannot be listed: {error}",
            Path::new(&dir).display()
        );
    }
    let mut files = Vec::new();
    find_xer_files(Path::new(&dir), &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "XER_CORPUS_DIR {} holds no .xer file",
        Path::new(&dir).display()
    );
    println!(
        "{} .xer files under {}",
        files.len(),
        Path::new(&dir).display()
    );
    let mut panicked = 0;
    for path in &files {
        if !report(path) {
            panicked += 1;
        }
    }
    assert_eq!(
        panicked, 0,
        "the library panicked on {panicked} corpus files"
    );
}
