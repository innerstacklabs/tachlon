//! Check 13: retired project names stay out of the repository (gs#609, gs#1120).
//!
//! The project was called by other names until the renames in gs#524 and gs#1120. This check
//! reads `git ls-files -z` from the repository root (tracked files only, so `target/`,
//! `node_modules/` and the generated `web/src/wasm/` are not scanned) and searches every tracked
//! file for the retired names, in any case: the working code name in its hyphen and underscore
//! spellings, and the product name that followed it. A file whose path names one fails too. A
//! tracked symlink is read as its target path, so a dangling link cannot fail the read. A file
//! that is not valid UTF-8 is skipped.
//!
//! The needles are built at run time (`["gantt", "spike"].join("-")`), and so are the self-test
//! fixtures, so this file does not contain a retired name and cannot trip itself.
//!
//! Three things are exempt, and nothing else:
//!
//! 1. The decision records, `docs/decisions/`: history keeps its own words (decision 0013 item 4).
//! 2. An entry on the allowlist, `name/allowlist.txt`, sorted bytewise and without repeats (a line
//!    that starts with `#` is a comment). An entry is a whole file, one exact repository path (only
//!    the list itself, which names what it exempts), or one exact text of one
//!    file, `path | text # why` with the reason required: that text is taken out of each line of
//!    the file before the search, so the rest of the file is searched (the infra names another
//!    change renames, and a script that reads a decision record). It only
//!    shrinks: adding a line needs a reviewer's yes.
//! 3. A line of a `docs/**/*.md` file that ends with `<!-- verbatim -->`: a verbatim quote of a
//!    person or a source. The marker is a convention a reviewer checks (is the line really a
//!    quote?), valid in any `docs/**/*.md`, living docs included. The same marker in `CLAUDE.md`,
//!    in code, in config or in any file outside `docs/` exempts nothing.
//!
//! A listed file that no longer names a retired name fails (delete its line), a listed text that no
//! line of its file holds fails, and so does a listed path that is not tracked (the file was
//! renamed or deleted).
//!
//! The product's name is the other half (gs#1120): it is written only in the files a person reads
//! as the product, `BRAND_FILES`, and under `examples/` (`EXAMPLES`), and everything else reads it from
//! `packages/gantt/src/product.ts`. Any other tracked file or path that names it, in any case, fails. A line exempted by the marker does
//! not count as naming it. That is how the list shrinks.
//!
//! It fails closed: `git` missing or exiting non-zero, an empty file list, an allowlist that
//! cannot be read or is not sorted, and a listed file that cannot be read all fail. A hit is one
//! failure per line, and every hit is listed, not the first.
//!
//! Threat model: a good-faith author who forgets. A name built at run time, split over two lines
//! or held in a binary file is not seen, as with the other checks here. Other spellings (the name
//! with a space, or in camel case) have no hits today and are not searched.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::architecture::support::root;

/// The allowlist, relative to the repository root.
pub(crate) const ALLOWLIST: &str = "crates/app/tests/architecture/name/allowlist.txt";

/// The decision records: history, never searched for a retired name.
const HISTORY: &str = "docs/decisions/";

/// A line of a `docs/**/*.md` file that ends with this is a verbatim quote and is not searched.
const MARKER: &str = "<!-- verbatim -->";

const HINT: &str = "names a retired project name; see decision 0013 for the rule and \
                    crates/app/tests/architecture/name/allowlist.txt for the files that may \
                    still name it. A verbatim quote in a docs/**/*.md file may end its line \
                    with <!-- verbatim -->.";

/// The only files, or folders (ending in `/`), that may name the product: the edges a person reads, and the first five, which name what is registered outside the repository.
const BRAND_FILES: [&str; 15] = [
    // The first five hold the runner labels, the runner repository variable and the handoff directory, and the repository slug the fork guard and the npm publisher use. A neutral name would mean re-labeling the runners. The list otherwise only shrinks (decision 0013).
    ".github/workflows/ci.yml",
    ".github/workflows/npm-publish.yml",
    ".github/workflows/pr.yml",
    "scripts/lib/ci-measure.ts",
    "scripts/test-ci-measure.ts",
    "README.md",
    "apps/site/",
    "book/",
    "docs/PRODUCT.md",
    "docs/decisions/",
    concat!(web_root!(), "/src/product.ts"),
    concat!(web_root!(), "/index.html"),
    concat!(web_root!(), "/package.json"),
    concat!(web_root!(), "/README.md"),
    concat!(web_root!(), "/AGENTS.md"),
];

const BRAND_HINT: &str = concat!(
    "names the product outside the files that may (BRAND_FILES in ",
    "crates/app/tests/architecture/name.rs); read PRODUCT_NAME from ",
    web_root!(),
    "/src/product.ts, or use a neutral name inside (gs#1120)"
);

/// The product's name, in lower case. Built at run time so this file does not name it.
fn brand() -> String {
    ["tach", "lon"].concat()
}

/// The examples folder: a developer copies it, so it names the package as they will (gantt#1484). One rule, not a list.
const EXAMPLES: &str = "examples/";

fn may_name_brand(path: &str) -> bool {
    path.starts_with(EXAMPLES)
        || BRAND_FILES
            .iter()
            .any(|edge| path == *edge || (edge.ends_with('/') && path.starts_with(edge)))
}

/// A failure for each line, and the path, of a file outside `BRAND_FILES` that names the product.
fn check_brand(file: &Tracked, failures: &mut Vec<String>) {
    if may_name_brand(&file.path) {
        return;
    }
    let brand = brand();
    let names = |text: &str| text.to_ascii_lowercase().contains(&brand);
    if names(&file.path) {
        failures.push(format!("{}: {BRAND_HINT}", file.path));
    }
    for (i, line) in file.text.iter().flat_map(|text| text.lines()).enumerate() {
        if names(line) {
            failures.push(format!("{}:{}: {BRAND_HINT}", file.path, i + 1));
        }
    }
}

/// The retired names, in lower case. Built at run time so no source file contains them.
fn needles() -> [String; 3] {
    let words = ["gantt", "spike"];
    [words.join("-"), words.join("_"), ["man", "tis"].concat()]
}

fn names_old_project(text: &str, needles: &[String; 3]) -> bool {
    let lower = text.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle.as_str()))
}

/// Whether `path` (repository-relative, `/`-separated) is a Markdown file under `docs/`.
#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "the paths are repository file names, all lowercase"
)]
fn is_docs_markdown(path: &str) -> bool {
    path.starts_with("docs/") && path.ends_with(".md")
}

/// A tracked file. `text` is `None` for a file that is not valid UTF-8.
pub(crate) struct Tracked {
    pub(crate) path: String,
    pub(crate) text: Option<String>,
}

/// Every failure over `files`, the tracked files of the repository, with `allowlist` the paths
/// that may still name a retired name.
pub(crate) fn check_name(files: &[Tracked], allowlist: &[String]) -> Vec<String> {
    let mut failures = Vec::new();
    if files.is_empty() {
        failures.push(
            "git ls-files -z: listed 0 files; the scan reads nothing, so this check would pass \
             on an empty list"
                .to_string(),
        );
    }
    let needles = needles();
    let mut listed: HashSet<&str> = HashSet::new();
    let mut texts: Vec<(&str, &str, bool)> = Vec::new();
    for entry in allowlist {
        match entry.split_once(" | ") {
            Some((path, text)) => texts.push((path, text, false)),
            None => _ = listed.insert(entry.as_str()),
        }
    }
    for file in files {
        check_brand(file, &mut failures);
        if file.path.starts_with(HISTORY) {
            continue;
        }
        let mut hits = Vec::new();
        if names_old_project(&file.path, &needles) {
            hits.push(format!(
                "{}: the file name names the old project name; rename the file",
                file.path
            ));
        }
        if let Some(text) = &file.text {
            let quotable = is_docs_markdown(&file.path);
            for (i, line) in text.lines().enumerate() {
                let mut line = line.to_string();
                for (_, allowed, used) in texts.iter_mut().filter(|t| t.0 == file.path) {
                    *used |= line.contains(*allowed);
                    line = line.replace(*allowed, "");
                }
                if names_old_project(&line, &needles)
                    && !(quotable && line.trim_end().ends_with(MARKER))
                {
                    hits.push(format!("{}:{}: {HINT}", file.path, i + 1));
                }
            }
        }
        if !listed.contains(file.path.as_str()) {
            failures.extend(hits);
        } else if hits.is_empty() {
            failures.push(format!(
                "{}: on the allowlist but names no retired project name; delete the line from \
                 {ALLOWLIST}",
                file.path
            ));
        }
    }
    let tracked: HashSet<&str> = files.iter().map(|file| file.path.as_str()).collect();
    for (path, text, _) in texts.iter().filter(|t| !t.2 && tracked.contains(t.0)) {
        failures.push(format!(
            "{path} | {text}: on the allowlist but no line holds that text; delete the line from \
             {ALLOWLIST}"
        ));
    }
    let entry_path = |entry: &String| {
        entry
            .split_once(" | ")
            .map_or(entry.clone(), |e| e.0.into())
    };
    for path in allowlist
        .iter()
        .map(entry_path)
        .filter(|path| !tracked.contains(path.as_str()))
    {
        failures.push(format!(
            "{path}: on the allowlist but not tracked; delete or correct the line in {ALLOWLIST}"
        ));
    }
    failures
}

/// The entries in the allowlist `text`, each `path` or `path | text` (the reason dropped): one
/// per line, sorted bytewise (`LC_ALL=C sort -u`, comment lines skipped), so a repeat or a line
/// appended out of order after a merge is noticed.
fn parse_allowlist(text: &str) -> Result<Vec<String>, String> {
    let mut paths: Vec<String> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let at = format!("{ALLOWLIST}:{}", i + 1);
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() || line != line.trim() {
            return Err(format!(
                "{at}: a blank line or spaces around a path; write one exact path per line"
            ));
        }
        let line = match line.split_once(" | ") {
            None => line,
            Some((_, rest)) => match rest.split_once(" # ") {
                Some((text, why)) if !text.trim().is_empty() && !why.trim().is_empty() => {
                    &line[..line.len() - why.len() - 3]
                }
                _ => {
                    return Err(format!(
                        "{at}: a text entry is `path | text # why`, with a text and a reason"
                    ));
                }
            },
        };
        if paths.last().is_some_and(|prev| line <= prev.as_str()) {
            return Err(format!(
                "{at}: not sorted, or a repeat; keep the list in `LC_ALL=C sort -u` order"
            ));
        }
        paths.push(line.to_string());
    }
    Ok(paths)
}

/// The allowlist of the repository at `dir`. An unreadable file is an error, never an empty list.
fn load_allowlist(dir: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(dir.join(ALLOWLIST)).map_err(|e| {
        format!("{ALLOWLIST}: cannot read: {e}; it decides what may name the old name")
    })?;
    parse_allowlist(&text)
}

/// The stdout of `program args` run in `dir`. Any way of not getting it is an error.
fn listing(program: &str, args: &[&str], dir: &Path) -> Result<Vec<u8>, String> {
    let name = format!("{program} {}", args.join(" "));
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("{name}: cannot run: {e}; the check reads nothing"))?;
    if !out.status.success() {
        return Err(format!(
            "{name}: {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

/// The paths in a `git ls-files -z` listing, and a failure for each path that is not UTF-8.
fn parse_listing(bytes: &[u8]) -> (Vec<String>, Vec<String>) {
    let mut paths = Vec::new();
    let mut failures = Vec::new();
    for raw in bytes.split(|b| *b == 0).filter(|raw| !raw.is_empty()) {
        match std::str::from_utf8(raw) {
            Ok(path) => paths.push(path.to_string()),
            Err(_) => failures.push(format!(
                "{}: the path is not UTF-8; this check cannot read it",
                String::from_utf8_lossy(raw)
            )),
        }
    }
    (paths, failures)
}

/// Reads `paths` under `dir`, and a failure for each one that cannot be read.
fn load(dir: &Path, paths: &[String]) -> (Vec<Tracked>, Vec<String>) {
    let mut files = Vec::new();
    let mut failures = Vec::new();
    for path in paths {
        let abs = dir.join(path);
        let read = match fs::symlink_metadata(&abs) {
            Ok(meta) if meta.file_type().is_symlink() => {
                fs::read_link(&abs).map(|target| Some(target.to_string_lossy().into_owned()))
            }
            Ok(_) => fs::read(&abs).map(|bytes| String::from_utf8(bytes).ok()),
            Err(e) => Err(e),
        };
        match read {
            Ok(text) => files.push(Tracked {
                path: path.clone(),
                text,
            }),
            Err(e) => failures.push(format!("{path}: cannot read a file git lists: {e}")),
        }
    }
    (files, failures)
}

/// Every failure over the repository at `dir`.
fn check_repository(dir: &Path) -> Vec<String> {
    let bytes = match listing("git", &["ls-files", "-z"], dir) {
        Ok(bytes) => bytes,
        Err(failure) => return vec![failure],
    };
    let allowlist = match load_allowlist(dir) {
        Ok(allowlist) => allowlist,
        Err(failure) => return vec![failure],
    };
    let (paths, mut failures) = parse_listing(&bytes);
    let (files, unread) = load(dir, &paths);
    failures.extend(unread);
    failures.extend(check_name(&files, &allowlist));
    failures
}

#[test]
fn the_old_name_stays_out_of_the_repository() {
    let failures = check_repository(&root());
    assert!(
        failures.is_empty(),
        "a retired project name is in the repository ({} problems; decision 0013 says why it \
         went):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

mod tests;
