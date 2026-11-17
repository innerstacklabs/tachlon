//! Self-tests: what the reader counts as an export, and the `wasm_bindgen` rules of check 4.

use super::*;

// What the reader counts as an export, and what it does not.

#[test]
fn only_public_members_and_attributed_fns_are_exports() {
    let src = "\
#[wasm_bindgen]
pub struct S {
    pub(crate) inner: u8,
    private: u8,
    /// A tuple-like count.
    pub count: u8,
}

#[wasm_bindgen]
pub struct Pair(
    /// First.
    pub u8,
    u8,
);

#[wasm_bindgen]
impl S {
    /// Public.
    pub fn a(&self) {}
    pub(crate) fn b(&self) {}
    fn c(&self) {}
    const N: u8 = 1;
}

impl S {
    /// Its own attribute makes it an export.
    #[wasm_bindgen]
    pub fn d(&self) {}
    pub fn rust_only(&self) {}
}

fn body() {
    #[wasm_bindgen]
    /// In a fn body.
    pub fn nested() {}
}
";
    let parsed = parse_exports(src);
    assert!(parsed.problems.is_empty(), "{:#?}", parsed.problems);
    assert_eq!(names(src), ["S.count", "Pair.0", "S.a", "S.d", "nested"]);
    let by_name: BTreeMap<String, bool> = parsed
        .exports
        .into_iter()
        .map(|e| (e.name, e.documented))
        .collect();
    assert!(by_name.values().all(|d| *d), "{by_name:#?}");
}

#[test]
fn a_doc_comment_is_text_in_a_doc_attribute() {
    let src = "\
#[wasm_bindgen]
impl App {
    /** A block doc. */
    pub fn block(&self) {}
    #[doc = \"An attribute doc.\"]
    pub fn attr(&self) {}
    ///
    pub fn blank(&self) {}
    #[doc(hidden)]
    pub fn hidden(&self) {}
    // not a doc comment
    pub fn plain(&self) {}
    /// Doc.
    // then a note
    #[allow(dead_code)]
    pub fn mixed(&self) {}
}
";
    let documented: Vec<(String, bool)> = parse_exports(src)
        .exports
        .into_iter()
        .map(|e| (e.name, e.documented))
        .collect();
    let want = [
        ("App.block", true),
        ("App.attr", true),
        ("App.blank", false),
        ("App.hidden", false),
        ("App.plain", false),
        ("App.mixed", true),
    ];
    let want: Vec<(String, bool)> = want.iter().map(|(n, d)| (n.to_string(), *d)).collect();
    assert_eq!(documented, want);
}

#[test]
fn an_attribute_on_an_item_the_reader_does_not_know_fails() {
    let src = "\
#[wasm_bindgen]
pub enum Mode {
    A,
}

#[wasm_bindgen]
const LIMIT: u8 = 1;

#[wasm_bindgen]
pub trait Named {}

#[wasm_bindgen]
pub mod inner {}

#[wasm_bindgen]
extern \"C\" {
    fn imported();
}

#[wasm_bindgen]
pub struct Quiet;

#[wasm_bindgen]
impl Quiet {
    #[wasm_bindgen]
    pub fn fine(&self) {}
}
";
    let parsed = parse_exports(src);
    assert_eq!(names(src), ["Quiet.fine"]);
    assert_eq!(parsed.problems.len(), 5, "{:#?}", parsed.problems);
    for line in [1, 6, 9, 12, 15] {
        one(
            &parsed.problems,
            &format!(
                "crates/app/src/web.rs:{line} has a #[wasm_bindgen] on an item the test in \
                 crates/app/tests/architecture/exports does not read"
            ),
        );
    }
    for p in &parsed.problems {
        assert!(p.contains("extend the test"), "{p}");
    }
}

#[test]
fn a_hidden_attribute_or_macro_fails_closed() {
    let cfg_attr = "\
#[cfg_attr(target_arch = \"wasm32\", wasm_bindgen)]
pub struct S {
    pub count: u32,
}
";
    one(
        &parse_exports(cfg_attr).problems,
        "crates/app/src/web.rs:1 names wasm_bindgen inside another attribute",
    );
    let mac = "\
macro_rules! export {
    () => {
        #[wasm_bindgen]
        pub fn made() {}
    };
}
";
    one(
        &parse_exports(mac).problems,
        "crates/app/src/web.rs:3 names wasm_bindgen inside a macro body",
    );
    let call = "\
#[wasm_bindgen]
impl App {
    /// Fine.
    pub fn ok(&self) {}
    more_exports!();
}
";
    one(
        &parse_exports(call).problems,
        "crates/app/src/web.rs:5 has a macro inside an exported impl",
    );
    let ty = "\
#[wasm_bindgen]
impl<'a> &'a [u8] {
    /// Fine.
    pub fn ok(&self) {}
}
";
    one(
        &parse_exports(ty).problems,
        "crates/app/src/web.rs:4 exports `ok` from an impl whose type is not a plain path",
    );
    let field = "\
pub struct S {
    #[wasm_bindgen]
    pub count: u32,
}
";
    one(
        &parse_exports(field).problems,
        "crates/app/src/web.rs:2 has a #[wasm_bindgen] on an item",
    );
}

#[test]
fn a_file_that_does_not_parse_fails() {
    let parsed = parse_exports("#[wasm_bindgen]\nimpl App {\n    pub fn a(&self) {}\n");
    assert_eq!(parsed.problems.len(), 1, "{:#?}", parsed.problems);
    assert!(
        parsed.problems[0].starts_with("crates/app/src/web.rs:"),
        "{:#?}",
        parsed.problems
    );
    assert!(parsed.problems[0].contains("does not parse as Rust"));
    assert_eq!(parsed.exports, [] as [Export; 0]);
}

#[test]
fn a_hidden_export_fails() {
    let failures = hidden_exports(
        "crates/engine/src/lib.rs",
        "// wasm_bindgen is only mentioned here\nuse wasm_bindgen::prelude::*;\n#[wasm_bindgen]\npub fn f() {}\n",
    );
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(
        &failures,
        "crates/engine/src/lib.rs:2 uses wasm_bindgen; exports belong only in crates/app/src/web.rs",
    );
}

#[test]
fn a_wasm_bindgen_with_arguments_or_a_path_fails() {
    let src = "\
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
#[wasm_bindgen]
#[wasm_bindgen(js_name = findNext)]
pub fn f() {}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn g() {}
";
    let problems = parse_exports(src).problems;
    assert_eq!(problems.len(), 2, "{problems:#?}");
    one(
        &problems,
        "crates/app/src/web.rs:4 has a #[wasm_bindgen] that is not the bare attribute",
    );
    one(
        &problems,
        "crates/app/src/web.rs:7 has a #[wasm_bindgen] that is not the bare attribute",
    );
    assert!(
        problems.iter().all(|p| !p.contains("extend the parser")),
        "{problems:#?}"
    );
}

#[test]
fn a_raw_identifier_is_the_same_name() {
    let src = "\
use wasm_bindgen::prelude::*;

/// An exported function.
#[r#wasm_bindgen]
pub fn documented_export() {}

#[r#wasm_bindgen]
pub fn missed_export() {}

#[wasm_bindgen]
impl App {
    #[r#doc = \"Documented.\"]
    pub fn r#loop(&self) {}
}
";
    let parsed = parse_exports(src);
    assert!(parsed.problems.is_empty(), "{:#?}", parsed.problems);
    let got: Vec<(String, bool)> = parsed
        .exports
        .into_iter()
        .map(|e| (e.name, e.documented))
        .collect();
    let want = [
        ("documented_export", true),
        ("missed_export", false),
        ("App.loop", true),
    ];
    let want: Vec<(String, bool)> = want.iter().map(|(n, d)| (n.to_string(), *d)).collect();
    assert_eq!(got, want);

    let renamed = "\
use wasm_bindgen::prelude::r#wasm_bindgen as wb;
extern crate r#wasm_bindgen as wb2;
";
    let problems = parse_exports(renamed).problems;
    assert_eq!(problems.len(), 2, "{problems:#?}");
    one(
        &problems,
        "crates/app/src/web.rs:1 renames wasm_bindgen to `wb`",
    );
    one(
        &problems,
        "crates/app/src/web.rs:2 renames wasm_bindgen to `wb2`",
    );

    let hidden = "\
#[cfg_attr(target_arch = \"wasm32\", r#wasm_bindgen)]
pub struct S {
    pub count: u32,
}
";
    one(
        &parse_exports(hidden).problems,
        "crates/app/src/web.rs:1 names wasm_bindgen inside another attribute",
    );
}

#[test]
fn an_alias_of_wasm_bindgen_fails() {
    for (src, line) in [
        ("use wasm_bindgen::prelude::wasm_bindgen as wb;\n", 1),
        (
            "use wasm_bindgen::prelude::{wasm_bindgen as wb, JsValue};\n",
            1,
        ),
        (
            "use wasm_bindgen::{\n    JsValue,\n    prelude::wasm_bindgen as wb,\n};\n",
            3,
        ),
        ("extern crate wasm_bindgen as wb;\n", 1),
    ] {
        let problems = parse_exports(src).problems;
        one(
            &problems,
            &format!("crates/app/src/web.rs:{line} renames wasm_bindgen to `wb`"),
        );
    }
    // A rename of something else, and a plain import, are fine.
    let problems = parse_exports(
        "use wasm_bindgen::prelude::{JsValue as V, wasm_bindgen};\nuse wasm_bindgen::prelude as p;\n",
    )
    .problems;
    assert!(problems.is_empty(), "{problems:#?}");
}

// Two layouts the line scan of check 4 rejected although the syntax tree reads them
// as valid. Each exercises check 1 (the export is read) and check 4 (it adds nothing).

#[test]
fn a_doc_string_or_literal_naming_wasm_bindgen_is_not_a_use() {
    let src = "\
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    #[doc = \"Uses wasm_bindgen.\"]
    pub fn documented(&self) -> &'static str {
        \"wasm_bindgen\"
    }

    /// Calls `wasm_bindgen` internals.
    pub fn literal(&self) -> &'static str {
        // wasm_bindgen
        \"#[wasm_bindgen(js_name = x)]\"
    }
}
";
    assert_eq!(names(src), ["App.documented", "App.literal"]);
    let failures = surface(src, &["App.documented", "App.literal"], &[]);
    assert!(failures.is_empty(), "{failures:#?}");
    // Unlisted, check 1 still sees both and check 4 adds nothing: the failures are the
    // two exports and the empty page.
    let failures = surface(src, &[], &[]);
    assert_eq!(failures.len(), 3, "{failures:#?}");
    one(
        &failures,
        "App.documented is exported at crates/app/src/web.rs:6",
    );
    one(
        &failures,
        "App.literal is exported at crates/app/src/web.rs:11",
    );
}

#[test]
fn a_wrapped_wasm_bindgen_import_passes() {
    let src = "\
use wasm_bindgen::{
    JsCast, JsValue, closure::Closure, convert::FromWasmAbi, describe,
    prelude::wasm_bindgen,
};

#[wasm_bindgen]
impl App {
    /// Fine.
    pub fn ok(&self) {}
}
";
    assert_eq!(names(src), ["App.ok"]);
    let failures = surface(src, &["App.ok"], &[]);
    assert!(failures.is_empty(), "{failures:#?}");
    // Unlisted: the export and the empty page, and no check 4 failure.
    let failures = surface(src, &[], &[]);
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(&failures, "App.ok is exported at crates/app/src/web.rs:9");
}
