//! Check 15: the chart's folder is named in one place (tl#1089). The checks here read its name from
//! `WEB_ROOT`, so the folder can move by changing the one string in `web_root!` below. A line carrying
//! `ALLOW` is skipped.
//! Any other code line of this directory's `.rs` files that spells the folder out fails; a `//` line
//! is ignored.

use crate::architecture::support::*;

/// The one place the folder's name is written. `web_root!()` is for `concat!`, `WEB_ROOT` for the rest.
macro_rules! web_root {
    () => {
        "packages/gantt"
    };
}
/// A path under the folder, as a literal: `web!("src/a.ts")`.
macro_rules! web {
    ($rest:literal) => {
        concat!(web_root!(), "/", $rest)
    };
}
pub(crate) const WEB_ROOT: &str = web_root!();

const ALLOW: &str = "allow(web-root)";
const DIR: &str = "crates/app/tests/architecture";

/// Lines (1-based) of `text` that spell the folder out in code: not a `//` line, not the allowed line,
/// and the name not part of a longer path (`src/`, `app/`) or word.
fn spelled_out(text: &str) -> Vec<usize> {
    let needle = format!("{WEB_ROOT}/");
    let mut lines = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("//") || line.contains(ALLOW) {
            continue;
        }
        let hit = line.match_indices(&needle).any(|(at, _)| {
            let before = &line[..at];
            !before.ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                && !before.ends_with("src/")
                && !before.ends_with("app/")
        });
        if hit {
            lines.push(i + 1);
        }
    }
    lines
}

fn check_web_root(files: &[SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    if files.is_empty() {
        failures.push(format!("{DIR}: found 0 Rust files; the scan reads nothing"));
    }
    for file in files {
        for line in spelled_out(&file.text) {
            failures.push(format!(
                "{}:{line} writes the chart's folder by hand; use WEB_ROOT (web_root.rs) so the folder can move",
                file.path
            ));
        }
    }
    failures
}

fn sources() -> Vec<SourceFile> {
    let mut out = Vec::new();
    collect(DIR, &["rs"], "", &mut out);
    out
}

#[test]
fn the_chart_folder_is_named_in_one_place() {
    let failures = check_web_root(&sources());
    assert!(
        failures.is_empty(),
        "the chart's folder is written by hand ({} problems; check 15 in main.rs):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn a_hand_written_folder_fails_and_the_allowed_forms_pass() {
    let hand = format!("const A: &str = \"{WEB_ROOT}/src/x.ts\";\n");
    let file = |text: &str| vec![file(&format!("{DIR}/a.rs"), text)];
    assert_eq!(check_web_root(&file(&hand)).len(), 1);
    let ok = format!(
        "// {WEB_ROOT}/src/x.ts\n//! {WEB_ROOT}/a\nlet p = \"crates/app/src/{WEB_ROOT}/a\"; let q = \"app/{WEB_ROOT}/b\";\nlet r = \"my-{WEB_ROOT}/c\"; let s = \"x\"; // {ALLOW}\nconst T: &str = \"{WEB_ROOT}/y\"; // {ALLOW}\n"
    );
    assert!(check_web_root(&file(&ok)).is_empty(), "{ok}");
}
