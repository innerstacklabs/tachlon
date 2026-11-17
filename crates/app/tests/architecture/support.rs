//! What the checks share: the repository's files, the page parser, the comment strippers and
//! the helpers the self-tests use.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::architecture::web_root::WEB_ROOT;

pub(crate) const WEB_RS: &str = "crates/app/src/web.rs";
/// The wasm module may also live in files under this directory (`src/web/mod.rs`-style splits).
pub(crate) const WEB_DIR: &str = "crates/app/src/web";
pub(crate) const PAGE: &str = "docs/WASM_API.md";
pub(crate) const INDEX_HTML: &str = concat!(web_root!(), "/index.html");
/// The chart's markup (gs#535): `mount` puts it in the page's root, so it is page HTML too.
pub(crate) const CHART_HTML: &str = concat!(web_root!(), "/src/chart.html");
/// The page entry: what a host page imports.
pub(crate) const INDEX_TS: &str = concat!(web_root!(), "/src/index.ts");
pub(crate) const WASM_DIR: &str = concat!(web_root!(), "/src/wasm");

/// Whether `path` (repository-relative, `/`-separated) belongs to the wasm module: the only
/// place `#[wasm_bindgen]` may appear.
pub(crate) fn is_web_module(path: &str) -> bool {
    path == WEB_RS
        || path
            .strip_prefix(WEB_DIR)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|rest| !rest.is_empty())
}

// ---------------------------------------------------------------------------
// Reading the page

#[derive(Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) name: String,
    /// 1-based line in the page.
    pub(crate) line: usize,
    pub(crate) test_only: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Page {
    pub(crate) rows: Vec<Row>,
    pub(crate) side_files: Vec<(String, usize)>,
    /// Token names (without `--gantt-`) from the "Theming" section's tables.
    pub(crate) tokens: Vec<(String, usize)>,
    /// Names from the "Page entry" section's table: one row per export of `packages/gantt/src/index.ts`.
    pub(crate) entry: Vec<(String, usize)>,
}

/// The first backticked cell of a table row (`prefix` is `"| "`) or bullet (`"- "`).
pub(crate) fn first_code(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?.strip_prefix('`')?;
    let (code, _) = rest.split_once('`')?;
    Some(code.to_string())
}

pub(crate) fn parse_page(page: &str) -> Page {
    enum Section {
        Other,
        Public,
        TestOnly,
        SideFiles,
        Theming,
        PageEntry,
    }
    let mut section = Section::Other;
    let mut fenced = false;
    let mut out = Page::default();
    for (i, line) in page.lines().enumerate() {
        let n = i + 1;
        if line.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = line.strip_prefix("## ") {
            section = match heading.trim() {
                "Public" => Section::Public,
                "Test-only" => Section::TestOnly,
                "Test-side files" => Section::SideFiles,
                "Theming" => Section::Theming,
                "Page entry" => Section::PageEntry,
                _ => Section::Other,
            };
            continue;
        }
        match section {
            Section::Public | Section::TestOnly => {
                if let Some(name) = first_code(line, "| ") {
                    out.rows.push(Row {
                        name,
                        line: n,
                        test_only: matches!(section, Section::TestOnly),
                    });
                }
            }
            Section::SideFiles => {
                if let Some(path) = first_code(line, "- ") {
                    out.side_files.push((path, n));
                }
            }
            Section::Theming => {
                if let Some(name) = first_code(line, "| ") {
                    let name = name.strip_prefix("--gantt-").unwrap_or(&name).to_string();
                    out.tokens.push((name, n));
                }
            }
            Section::PageEntry => {
                if let Some(name) = first_code(line, "| ") {
                    out.entry.push((name, n));
                }
            }
            Section::Other => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Check 5 and the TypeScript side of check 6

pub(crate) struct SourceFile {
    /// Path from the repository root, `/`-separated.
    pub(crate) path: String,
    pub(crate) text: String,
}

/// `src` with `//` and `/* */` comments blanked, newlines kept so line numbers
/// hold. Quotes are tracked so `"http://x"` is not read as a comment; string
/// contents stay, so `` `${app.test_x()}` `` is still seen.
pub(crate) fn strip_ts_comments(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut quote: Option<char> = None;
    let mut block = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if block {
            if c == '*' && next == Some('/') {
                block = false;
                i += 2;
                continue;
            }
            if c == '\n' {
                out.push('\n');
            }
            i += 1;
            continue;
        }
        if let Some(q) = quote {
            out.push(c);
            if c == '\\' {
                if let Some(n) = next {
                    out.push(n);
                    i += 2;
                    continue;
                }
            } else if c == q || (c == '\n' && q != '`') {
                quote = None;
            }
            i += 1;
            continue;
        }
        match (c, next) {
            ('/', Some('/')) => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                block = true;
                out.push(' ');
                i += 2;
            }
            ('\'' | '"' | '`', _) => {
                quote = Some(c);
                out.push(c);
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

pub(crate) fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Byte offset of the first `pat` in `line` that is not part of a longer identifier.
pub(crate) fn find_bounded(line: &str, pat: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(at) = line[from..].find(pat) {
        let start = from + at;
        let end = start + pat.len();
        let before_ok = !line[..start].chars().next_back().is_some_and(is_ident);
        let after_ok = !line[end..].chars().next().is_some_and(is_ident);
        if before_ok && after_ok {
            return Some(start);
        }
        from = start + 1;
    }
    None
}

/// Every identifier in `line`.
pub(crate) fn identifiers(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in line.char_indices() {
        match (is_ident(c), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push(&line[s..i]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push(&line[s..]);
    }
    out
}

/// Block comments blanked, newlines kept (CSS, and the CSS inside the page).
pub(crate) fn strip_block_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => {
                let comment = &rest[start..start + 2 + end + 2];
                out.extend(comment.chars().filter(|c| *c == '\n'));
                rest = &rest[start + 2 + end + 2..];
            }
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
}

// ---------------------------------------------------------------------------
// The files

pub(crate) fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(crate) fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// Files under `dir` (a repository-relative path) whose extension is in `exts`,
/// sorted, skipping the directory `skip`.
pub(crate) fn collect(dir: &str, exts: &[&str], skip: &str, out: &mut Vec<SourceFile>) {
    let abs = root().join(dir);
    let mut entries: Vec<_> = fs::read_dir(&abs)
        .unwrap_or_else(|e| panic!("cannot read directory {dir}: {e}"))
        .map(|e| e.expect("a directory entry is readable").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a UTF-8 file name");
        let rel = format!("{dir}/{name}");
        if path.is_dir() {
            if rel != skip {
                collect(&rel, exts, skip, out);
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.contains(&e))
        {
            out.push(SourceFile {
                text: read(&rel),
                path: rel,
            });
        }
    }
}

pub(crate) fn rust_sources() -> Vec<SourceFile> {
    let mut out = Vec::new();
    let mut crates: Vec<_> = fs::read_dir(root().join("crates"))
        .expect("crates/ is readable")
        .map(|e| e.expect("a directory entry is readable").path())
        .collect();
    crates.sort();
    for krate in crates {
        let name = krate.file_name().and_then(|n| n.to_str()).expect("a name");
        if krate.join("src").is_dir() {
            collect(&format!("crates/{name}/src"), &["rs"], "", &mut out);
        }
    }
    out
}

/// The script extensions `web_sources` reads: the ones `hosting.rs` and `size.rs` scan, and `jsx`.
pub(crate) const WEB_SCRIPT_EXTENSIONS: &[&str] =
    &["ts", "tsx", "mts", "cts", "js", "mjs", "cjs", "jsx"];

pub(crate) fn web_sources() -> Vec<SourceFile> {
    let mut out = Vec::new();
    collect(
        concat!(web_root!(), "/src"),
        WEB_SCRIPT_EXTENSIONS,
        WASM_DIR,
        &mut out,
    );
    out
}

/// The HTML the page shows: the page itself and the chart's markup, which `mount` inserts. A check
/// that reads the page reads both.
pub(crate) fn web_html() -> Vec<SourceFile> {
    [INDEX_HTML, CHART_HTML]
        .into_iter()
        .map(|path| SourceFile {
            path: path.to_string(),
            text: read(path),
        })
        .collect()
}

/// Standalone stylesheets under `packages/gantt/src` (the generated `packages/gantt/src/wasm/` aside).
pub(crate) fn web_css() -> Vec<SourceFile> {
    let mut out = Vec::new();
    collect(concat!(web_root!(), "/src"), &["css"], WASM_DIR, &mut out);
    out
}

pub(crate) fn page(public: &[&str], test_only: &[&str]) -> Page {
    let mut text = String::from(
        "# WASM API\n\n## Public\n\n### Group\n\n| Export | What | Notes |\n|---|---|---|\n",
    );
    for name in public {
        let _ = writeln!(text, "| `{name}` | does it | - |");
    }
    text.push_str("\n## Test-only\n\n| Export | What | Notes |\n|---|---|---|\n");
    for name in test_only {
        let _ = writeln!(text, "| `{name}` | does it | - |");
    }
    parse_page(&text)
}

pub(crate) fn one(failures: &[String], needle: &str) {
    let hits: Vec<_> = failures.iter().filter(|f| f.contains(needle)).collect();
    assert_eq!(
        hits.len(),
        1,
        "wanted one failure with {needle:?} in {failures:#?}"
    );
}

pub(crate) fn file(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: path.to_string(),
        text: text.to_string(),
    }
}

/// `text` as the whole of the page's HTML: one file, `packages/gantt/index.html`.
pub(crate) fn index_page(text: &str) -> Vec<SourceFile> {
    vec![file(INDEX_HTML, text)]
}

/// `text` as the whole wasm module: one file, `crates/app/src/web.rs`.
pub(crate) fn web_rs(text: &str) -> Vec<SourceFile> {
    vec![file(WEB_RS, text)]
}

#[test]
fn the_page_parser_reads_sections_and_skips_fences() {
    let text = &format!(
        "## Public\n| `App.a` | x | - |\n```\n## Test-only\n```\n| `App.b` | x | - |\n## Test-only\n| `App.test_c` | x | - |\n## Test-side files\n- `{WEB_ROOT}/src/m.ts` (note)\n## Theming\n| `--gantt-bg` | x |\n| `axis-h` | x |\n## Page entry\n| `mount` | x |\n## Other\n| `App.z` | x |\n"
    );
    let page = parse_page(text);
    let rows: Vec<_> = page
        .rows
        .iter()
        .map(|r| (r.name.as_str(), r.test_only))
        .collect();
    assert_eq!(
        rows,
        [("App.a", false), ("App.b", false), ("App.test_c", true)]
    );
    assert_eq!(page.side_files, [(format!("{WEB_ROOT}/src/m.ts"), 10)]);
    let tokens: Vec<_> = page.tokens.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(tokens, ["bg", "axis-h"]);
    assert_eq!(page.entry, [("mount".to_string(), 15)]);
}
