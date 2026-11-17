//! Check 10: the file-size budget (gs#160). Dave, 2026-10-24: "i prefer smaller files."
//!
//! | Kind | Limit | Counted as |
//! |---|---|---|
//! | Rust source file (`crates/*/src`, `benches`, `examples`) | 400 | lines above its `#[cfg(test)] mod tests` |
//! | In-file Rust test module | 300 | the `mod tests` block |
//! | TS/JS/HTML/CSS source (`packages/gantt/src`, `packages/gantt/index.html`, `demos/`, `scripts/`, `apps/site/`, `apps/chrome/src`, and in `apps/hosted/`: `src`, `samples`, `scripts`, `index.html`, `*.config.ts`) | 400 | whole file |
//! | Test file (`crates/*/tests`, `tests.rs`, `packages/gantt/tests`, `packages/gantt/scripts`, `apps/hosted/tests`) | 600 | whole file |
//!
//! A count is `wc -l`: the number of newline characters in the part.
//!
//! The files that were over a limit when the budget began are in `EXEMPT` at their exact size.
//! The list only shrinks:
//!
//! - a listed file that grows fails, and the message says to split it;
//! - a listed file that shrinks but is still over its limit fails until its row names the new
//!   size, so it cannot grow back;
//! - a listed file that is back under its limit fails until its row is deleted;
//! - a file that is not listed gets no allowance, however it came to be over its limit.
//!
//! A test module moved out of its source file, to `foo/tests.rs` or `crates/<c>/tests/`, is a test
//! file and gets the 600-line limit, not the 300 of an in-file module. That is the intended remedy
//! for an in-file module over 300 (karpathy, gs#160): the test file still cannot grow past 600, and
//! the source file above it shrinks.
//!
//! Threat model: a good-faith author who forgets. A file that is renamed or moved to a path
//! outside the roots below is not measured; each kind must find files, so a moved root fails.
//!
//! `split_code_test` is adapted from fleetwatch's `tests/architecture/source.rs`
//! (MIT OR Apache-2.0, <https://github.com/daveremy/fleetwatch>). Its policy is not: fleetwatch
//! sets ceilings at 1.5x to 2x the measured size; this budget uses flat limits.

use std::fs;

use crate::architecture::support::{WASM_DIR, root};

const RUST_SOURCE_LIMIT: usize = 400;
const TEST_MODULE_LIMIT: usize = 300;
const WEB_SOURCE_LIMIT: usize = 400;
const TEST_FILE_LIMIT: usize = 600;

/// Extensions that count as source in the TS/JS/HTML/CSS kinds.
const WEB_EXTENSIONS: [&str; 9] = ["ts", "tsx", "mts", "cts", "js", "mjs", "cjs", "html", "css"];

/// Where the walk starts, relative to the repository root.
const ROOTS: [&str; 9] = [
    "crates",
    concat!(web_root!(), "/src"),
    concat!(web_root!(), "/index.html"),
    concat!(web_root!(), "/tests"),
    "apps/hosted",
    "apps/chrome",
    "apps/site",
    "demos",
    "scripts",
];

/// Directory names never entered: build output and dependencies.
const SKIPPED_NAMES: [&str; 4] = ["target", "node_modules", ".git", "pkg"];

/// Directories never entered, by path: generated or recorded output.
const SKIPPED_PATHS: [&str; 5] = [
    WASM_DIR,
    "demos/screencast/out",
    "demos/screencast/draft",
    "apps/hosted/dist",
    "apps/hosted/public/samples",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    RustSource,
    WebSource,
    TestFile,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::RustSource => "Rust source",
            Kind::WebSource => "TS/JS/HTML/CSS source",
            Kind::TestFile => "test",
        }
    }
}

/// What is counted in a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// The lines above `#[cfg(test)] mod tests` (all lines when there is none).
    Code,
    /// The `mod tests` block of a Rust source file.
    TestModule,
    /// The whole file.
    File,
}

impl Part {
    fn name(self) -> &'static str {
        match self {
            Part::Code => "code above its test module",
            Part::TestModule => "in-file test module",
            Part::File => "file",
        }
    }
}

use Part::{Code, File, TestModule};

#[derive(Debug, PartialEq, Eq)]
struct Measure {
    path: String,
    part: Part,
    lines: usize,
    limit: usize,
}

/// The files that were over a limit when the budget began, at their size then. Only shrinks:
/// delete a row when its file is under the limit, and lower it when its file shrinks.
const EXEMPT: &[(&str, Part, usize)] = &[];

/// Whether `path` is source of the hosted app (gs#537): its `src`, `samples` and `scripts`, its
/// page and its `*.config.ts` files. Its `tests` are test files, handled in `kind`.
fn is_hosted_source(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("apps/hosted/") else {
        return false;
    };
    rest.starts_with("src/")
        || rest.starts_with("samples/")
        || rest.starts_with("scripts/")
        || rest == "index.html"
        || (!rest.contains('/') && rest.ends_with(".config.ts"))
}

/// The kind of budget a repository-relative path falls under, if any.
fn kind(path: &str) -> Option<Kind> {
    let (_, extension) = path.rsplit_once('.')?;
    if path.starts_with("crates/") {
        if extension != "rs" {
            return None;
        }
        let test_path = path.split('/').any(|part| part == "tests") || path.ends_with("/tests.rs");
        return Some(if test_path {
            Kind::TestFile
        } else {
            Kind::RustSource
        });
    }
    if !WEB_EXTENSIONS.contains(&extension) {
        return None;
    }
    if path.starts_with(concat!(web_root!(), "/tests/"))
        || path.starts_with(concat!(web_root!(), "/scripts/"))
        || path.starts_with("apps/hosted/tests/")
    {
        Some(Kind::TestFile)
    } else if path.starts_with(concat!(web_root!(), "/src/"))
        || path == concat!(web_root!(), "/index.html")
        || path.starts_with("demos/")
        || path.starts_with("scripts/")
        || path.starts_with("apps/site/")
        || path.starts_with("apps/chrome/src/")
        || is_hosted_source(path)
    {
        Some(Kind::WebSource)
    } else {
        None
    }
}

/// `wc -l`: the number of newline characters.
fn wc_l(text: &str) -> usize {
    text.matches('\n').count()
}

/// Whether `item` is the declaration of a module named exactly `tests` (`mod tests {`,
/// `mod tests{` or `mod tests;`), not `mod tests_foo`.
fn is_tests_module(item: &str) -> bool {
    item.strip_prefix("mod tests").is_some_and(|rest| {
        let rest = rest.trim_start();
        rest.starts_with('{') || rest.starts_with(';')
    })
}

/// Splits a file into its code and its trailing `mod tests`. The boundary is a top-level
/// (unindented) `#[cfg(test)]` whose item is `mod tests`; an indented one belongs to an inner
/// module and does not split the file. Production code after the test module would escape the
/// budget; clippy's `items_after_test_module` rejects that, so it is not checked again here.
/// (Adapted from fleetwatch, see the header.)
fn split_code_test(text: &str) -> (Vec<&str>, Vec<&str>) {
    let lines: Vec<_> = text.lines().collect();
    let boundary = lines.iter().enumerate().position(|(i, line)| {
        if let Some(item) = line.strip_prefix("#[cfg(test)] ") {
            return is_tests_module(item);
        }
        *line == "#[cfg(test)]"
            && lines[i + 1..]
                .iter()
                .map(|line| line.trim())
                .find(|line| !line.is_empty() && !line.starts_with("#["))
                .is_some_and(is_tests_module)
    });
    let at = boundary.unwrap_or(lines.len());
    (lines[..at].to_vec(), lines[at..].to_vec())
}

/// The counts of one file, or none when no budget applies to its path.
fn measure(path: &str, text: &str) -> Vec<Measure> {
    let Some(kind) = kind(path) else {
        return Vec::new();
    };
    let measure = |part, lines, limit| Measure {
        path: path.to_string(),
        part,
        lines,
        limit,
    };
    match kind {
        Kind::RustSource => {
            let (code, tests) = split_code_test(text);
            if tests.is_empty() {
                vec![measure(Code, wc_l(text), RUST_SOURCE_LIMIT)]
            } else {
                // `code` ends at the boundary line, so each of its lines ends in a newline.
                let above = code.len();
                vec![
                    measure(Code, above, RUST_SOURCE_LIMIT),
                    measure(TestModule, wc_l(text) - above, TEST_MODULE_LIMIT),
                ]
            }
        }
        Kind::WebSource => vec![measure(File, wc_l(text), WEB_SOURCE_LIMIT)],
        Kind::TestFile => vec![measure(File, wc_l(text), TEST_FILE_LIMIT)],
    }
}

/// Every failure of `measures` against the `exempt` list.
fn check_budget(measures: &[Measure], exempt: &[(&str, Part, usize)]) -> Vec<String> {
    let mut failures = Vec::new();
    for m in measures {
        let listed = exempt
            .iter()
            .find(|(path, part, _)| *path == m.path && *part == m.part)
            .map(|(_, _, lines)| *lines);
        let (path, part, lines, limit) = (&m.path, m.part.name(), m.lines, m.limit);
        match listed {
            None if lines > limit => failures.push(format!(
                "{path}: {part} is {lines} lines, over the {limit}-line limit. Split it into \
                 smaller modules. A file that is not on EXEMPT in \
                 crates/app/tests/architecture/size.rs gets no allowance."
            )),
            Some(_) if lines <= limit => failures.push(format!(
                "{path}: {part} is {lines} lines, within the {limit}-line limit. Delete its row \
                 from EXEMPT in crates/app/tests/architecture/size.rs: the list only shrinks."
            )),
            Some(listed) if lines > listed => failures.push(format!(
                "{path}: {part} grew from {listed} to {lines} lines (limit {limit}). It is on \
                 EXEMPT, so it may not grow: split it, or put the new code in a new file."
            )),
            Some(listed) if lines < listed => failures.push(format!(
                "{path}: {part} shrank from {listed} to {lines} lines (limit {limit}). Set its \
                 row in EXEMPT in crates/app/tests/architecture/size.rs to {lines}, so that it \
                 cannot grow back."
            )),
            None | Some(_) => {}
        }
    }
    for (path, part, _) in exempt {
        if !measures.iter().any(|m| m.path == *path && m.part == *part) {
            failures.push(format!(
                "{path}: EXEMPT lists its {}, but there is no such file or part any more. \
                 Delete the row from crates/app/tests/architecture/size.rs.",
                part.name()
            ));
        }
    }
    let mut seen: Vec<(&str, Part)> = Vec::new();
    for (path, part, _) in exempt {
        if seen.contains(&(path, *part)) {
            failures.push(format!(
                "{path}: EXEMPT lists its {} twice. Keep one row.",
                part.name()
            ));
        }
        seen.push((path, *part));
    }
    failures
}

/// Every file under `rel` (a file or a directory) as `(path, text)`, skipping the directories
/// in `SKIPPED_NAMES` and `SKIPPED_PATHS`.
fn walk(rel: &str, out: &mut Vec<(String, String)>) {
    let abs = root().join(rel);
    if abs.is_file() {
        let text = fs::read_to_string(&abs).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"));
        out.push((rel.to_string(), text));
        return;
    }
    let mut entries: Vec<_> = fs::read_dir(&abs)
        .unwrap_or_else(|e| panic!("cannot read directory {rel}: {e}"))
        .map(|e| e.expect("a directory entry is readable").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a UTF-8 file name");
        let child = format!("{rel}/{name}");
        if path.is_dir() {
            if !SKIPPED_NAMES.contains(&name) && !SKIPPED_PATHS.contains(&child.as_str()) {
                walk(&child, out);
            }
        } else if kind(&child).is_some() {
            walk(&child, out);
        }
    }
}

/// Every count over `files` (`(path, text)`), and a failure for each kind that found no file.
fn measure_all(files: &[(String, String)]) -> (Vec<Measure>, Vec<String>) {
    let mut measures = Vec::new();
    let mut found = [0usize; 3];
    for (path, text) in files {
        if let Some(kind) = kind(path) {
            found[kind as usize] += 1;
        }
        measures.extend(measure(path, text));
    }
    let mut failures = Vec::new();
    for kind in [Kind::RustSource, Kind::WebSource, Kind::TestFile] {
        if found[kind as usize] == 0 {
            failures.push(format!(
                "size budget: found 0 {} files; the scan reads nothing, so this test would \
                 pass on an empty tree",
                kind.name()
            ));
        }
    }
    (measures, failures)
}

/// Every failure of the budget over `files` (`(path, text)`) against the real `EXEMPT` list.
pub(crate) fn budget_failures(files: &[(String, String)]) -> Vec<String> {
    let (measures, mut failures) = measure_all(files);
    failures.extend(check_budget(&measures, EXEMPT));
    failures
}

/// Every failure of the budget over the real files.
fn check_size() -> Vec<String> {
    let mut files = Vec::new();
    for rel in ROOTS {
        walk(rel, &mut files);
    }
    budget_failures(&files)
}

#[test]
fn the_tree_is_inside_its_size_budget() {
    let failures = check_size();
    assert!(
        failures.is_empty(),
        "the size budget fails ({} problems; the limits are in crates/app/tests/architecture/\
         size.rs):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

mod tests;
