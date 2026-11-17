//! Self-tests of check 1, 2, 3 and 6: one failing case per message.

use std::collections::BTreeMap;

use crate::architecture::theming::check_theming;
use crate::architecture::web_sources::check_web_sources;

use super::*;

// ---------------------------------------------------------------------------
// The checks fire: one synthetic failing case per message

const SAMPLE_RS: &str = "\
use wasm_bindgen::prelude::*;

/// An app.
#[wasm_bindgen]
pub struct App {
    canvas: u8,
}

#[wasm_bindgen]
impl App {
    /// Set up.
    pub async fn create(
        canvas: u8,
    ) -> u8 {
        canvas
    }

    // a plain comment is skipped
    /// Find things.
    #[allow(clippy::needless_pass_by_value)]
    pub fn find(&self) {}

    pub fn undocumented(&self) {}

    fn private(&self) {}

    /// A test helper.
    pub fn test_seed(&self) -> u8 {
        let nested = || {
            pub fn not_an_export() {}
        };
        0
    }
}

impl App {
    /// Not exported: this impl is not #[wasm_bindgen].
    pub fn rust_only(&self) {}
}

/// The first panic.
#[wasm_bindgen]
pub fn last_panic() {}
";

#[test]
fn the_export_parser_reads_every_form() {
    let got: Vec<(String, usize, bool)> = parse_exports(SAMPLE_RS)
        .exports
        .into_iter()
        .map(|e| (e.name, e.line, e.documented))
        .collect();
    let want = [
        ("App.create", 12, true),
        ("App.find", 21, true),
        ("App.undocumented", 23, false),
        ("App.test_seed", 28, true),
        ("last_panic", 43, true),
    ];
    let want: Vec<(String, usize, bool)> = want
        .iter()
        .map(|(n, l, d)| (n.to_string(), *l, *d))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn a_clean_sample_passes() {
    let page = page(
        &["App.create", "App.find", "App.undocumented", "last_panic"],
        &["App.test_seed"],
    );
    let failures = check_surface(&web_rs(SAMPLE_RS), &page);
    // Only the missing doc comment is wrong with the sample.
    assert_eq!(failures.len(), 1, "{failures:#?}");
}

#[test]
fn an_export_missing_from_the_page_fails() {
    let page = page(
        &["App.create", "App.undocumented", "last_panic"],
        &["App.test_seed"],
    );
    let failures = check_surface(&web_rs(SAMPLE_RS), &page);
    one(
        &failures,
        "App.find is exported at crates/app/src/web.rs:21 but is not in docs/WASM_API.md; add a row \
         under \"Public\", or under \"Test-only\" with a test_ name",
    );
}

#[test]
fn a_page_row_with_no_export_fails() {
    let page = page(
        &[
            "App.create",
            "App.find",
            "App.undocumented",
            "last_panic",
            "App.row_cuont",
        ],
        &["App.test_seed"],
    );
    let failures = check_surface(&web_rs(SAMPLE_RS), &page);
    one(
        &failures,
        "docs/WASM_API.md:13 lists App.row_cuont but the wasm module (crates/app/src/web.rs, crates/app/src/web/) exports no such item",
    );
}

#[test]
fn an_export_without_a_doc_comment_fails() {
    let page = page(
        &["App.create", "App.find", "App.undocumented", "last_panic"],
        &["App.test_seed"],
    );
    let failures = check_surface(&web_rs(SAMPLE_RS), &page);
    one(
        &failures,
        "App.undocumented (crates/app/src/web.rs:23) has no doc comment",
    );
}

#[test]
fn a_test_only_row_must_be_named_test() {
    let page = page(
        &["App.create", "App.find", "App.undocumented", "last_panic"],
        &["App.test_seed", "App.calendar_add"],
    );
    let failures = check_surface(&web_rs(SAMPLE_RS), &page);
    one(
        &failures,
        "App.calendar_add is listed Test-only but is not named test_*",
    );
}

#[test]
fn a_public_row_must_not_be_named_test() {
    let page = page(
        &[
            "App.create",
            "App.find",
            "App.undocumented",
            "last_panic",
            "App.test_seed",
        ],
        &[],
    );
    let failures = check_surface(&web_rs(SAMPLE_RS), &page);
    one(
        &failures,
        "App.test_seed is listed Public but is named test_*",
    );
}

#[test]
fn a_row_listed_twice_fails() {
    let page = page(
        &[
            "App.create",
            "App.find",
            "App.undocumented",
            "last_panic",
            "App.find",
        ],
        &["App.test_seed"],
    );
    let failures = check_surface(&web_rs(SAMPLE_RS), &page);
    one(&failures, "lists App.find again");
}

#[test]
fn an_empty_parse_fails() {
    let failures = check_surface(&web_rs("// nothing here\n"), &page(&[], &[]));
    one(&failures, "found 0 #[wasm_bindgen] exports");
    one(&failures, "found 0 rows under \"Public\" and \"Test-only\"");
    let failures = check_web_sources(&[], &[]);
    one(&failures, "found 0 TypeScript files");
    let failures = check_theming(&[], &BTreeMap::new());
    one(&failures, "found 0 tokens in the \"Theming\" section");
    one(&failures, "found 0 --gantt- tokens");
}

#[test]
fn a_file_under_the_web_directory_is_part_of_the_wasm_module() {
    for path in [
        WEB_RS,
        "crates/app/src/web/mod.rs",
        "crates/app/src/web/schedule/rows.rs",
    ] {
        assert!(is_web_module(path), "{path}");
    }
    for path in [
        "crates/app/src/web",
        "crates/app/src/web/",
        "crates/app/src/webx.rs",
        "crates/app/src/web_extra/a.rs",
        "crates/engine/src/web.rs",
        "crates/app/src/lib.rs",
    ] {
        assert!(!is_web_module(path), "{path}");
    }
}

fn module_files(second: &str) -> Vec<SourceFile> {
    vec![
        file(
            WEB_RS,
            "#[wasm_bindgen]\nimpl App {\n    /// Find.\n    pub fn find(&self) {}\n}\n",
        ),
        file("crates/app/src/web/rows.rs", second),
    ]
}

const ROWS_RS: &str = "#[wasm_bindgen]\nimpl Rows {\n    /// Count.\n    pub fn count(&self) {}\n    pub fn bare(&self) {}\n}\n";

#[test]
fn the_exports_of_every_module_file_are_read_and_named_by_file() {
    let listed = page(&["App.find", "Rows.count", "Rows.bare"], &[]);
    let failures = check_surface(&module_files(ROWS_RS), &listed);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "Rows.bare (crates/app/src/web/rows.rs:5) has no doc comment",
    );
    let failures = check_surface(&module_files(ROWS_RS), &page(&["App.find"], &[]));
    one(
        &failures,
        "Rows.count is exported at crates/app/src/web/rows.rs:4 but is not in",
    );
    let failures = check_surface(
        &module_files(ROWS_RS)[..1],
        &page(&["App.find", "Rows.count"], &[]),
    );
    one(
        &failures,
        "lists Rows.count but the wasm module (crates/app/src/web.rs, crates/app/src/web/) \
         exports no such item",
    );
}

#[test]
fn a_name_exported_by_two_module_files_fails() {
    let second = "#[wasm_bindgen]\nimpl App {\n    /// Find.\n    pub fn find(&self) {}\n}\n";
    let failures = check_surface(&module_files(second), &page(&["App.find"], &[]));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "App.find is exported at crates/app/src/web.rs:4 and at crates/app/src/web/rows.rs:4",
    );
}

#[test]
fn a_module_file_that_does_not_parse_fails_and_names_itself() {
    let failures = check_surface(&module_files("impl {\n"), &page(&["App.find"], &[]));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "crates/app/src/web/rows.rs:1 does not parse as Rust",
    );
}
