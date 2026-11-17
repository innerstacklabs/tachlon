//! Self-tests: source layouts the reader must get right.

use super::*;

// The four layouts review found that the line parser read wrongly. Each is a source
// on its own, held as a string, and each must fail with the export it hides.

#[test]
fn an_export_in_an_inline_mod_fails() {
    let src = "\
#[wasm_bindgen]
impl App {
    /// Fine.
    pub fn ok(&self) {}
}

mod extra {
    #[wasm_bindgen]
    pub struct Extra;

    #[wasm_bindgen]
    impl Extra {
        /// Hidden.
        pub fn hidden() {}
    }

    /// Free.
    #[wasm_bindgen]
    pub fn free() {}
}
";
    assert_eq!(names(src), ["App.ok", "Extra.hidden", "free"]);
    let failures = surface(src, &["App.ok"], &[]);
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(
        &failures,
        "Extra.hidden is exported at crates/app/src/web.rs:14 but is not in docs/WASM_API.md; \
         add a row under \"Public\", or under \"Test-only\" with a test_ name",
    );
    one(
        &failures,
        "free is exported at crates/app/src/web.rs:19 but is not in docs/WASM_API.md",
    );
    // Listed, they are the surface like any other.
    let failures = surface(src, &["App.ok", "Extra.hidden", "free"], &[]);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_pub_field_after_a_struct_header_comment_fails() {
    let src = "\
#[wasm_bindgen]
pub struct S { // app state
    /// Count.
    pub count: u32,
    quiet: u8, // not public
}
";
    assert_eq!(names(src), ["S.count"]);
    let failures = surface(src, &[], &[]);
    one(
        &failures,
        "S.count is exported at crates/app/src/web.rs:4 but is not in docs/WASM_API.md",
    );
    let failures = surface(src, &["S.count"], &[]);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_pub_field_after_a_block_comment_fails() {
    let src = "\
#[wasm_bindgen]
pub struct S {
    /* exposed count */ pub count: u32,
    /// The rest.
    pub rest: u8,
}
";
    assert_eq!(names(src), ["S.count", "S.rest"]);
    let failures = surface(src, &["S.rest"], &[]);
    one(
        &failures,
        "S.count is exported at crates/app/src/web.rs:3 but is not in docs/WASM_API.md",
    );
    one(
        &failures,
        "S.count (crates/app/src/web.rs:3) has no doc comment",
    );
}

#[test]
fn a_multi_line_string_at_column_zero_does_not_hide_a_later_export() {
    // The two r4 probes, devteam-1 261b receipts/gs100b/probes/13 and 14, and a
    // string that ends with `}` and one that starts a line with `type`.
    let cases = [
        (
            "a JSON template ending in a brace",
            "\
#[wasm_bindgen]
impl App {
    fn probe_json(&self) -> &'static str {
        r#\"{
  \"a\": 1
}\"#
    }

    pub fn probe_after_brace(&self) {}
}
",
            "App.probe_after_brace",
            9,
        ),
        (
            "a help text with a line starting `type`",
            "\
#[wasm_bindgen]
impl App {
    fn probe_help(&self) -> &'static str {
        \"Filter:
type a name to filter rows\"
    }

    pub fn probe_after_type(&self) {}
}
",
            "App.probe_after_type",
            8,
        ),
        (
            "a plain string with `}` alone on a line",
            "\
#[wasm_bindgen]
impl App {
    pub fn text(&self) -> &str {
        \"{
}\"
    }

    /// After.
    pub fn after(&self) {}
}
",
            "App.after",
            9,
        ),
    ];
    for (what, src, hidden, line) in cases {
        let failures = surface(src, &[], &[]);
        assert!(
            failures.iter().any(|f| f.contains(&format!(
                "{hidden} is exported at crates/app/src/web.rs:{line} but is not in docs/WASM_API.md"
            ))),
            "{what}: {failures:#?}"
        );
    }
    // The export after the string is read as an export, so its missing doc comment shows.
    let src = "\
#[wasm_bindgen]
impl App {
    /// Text.
    pub fn text(&self) -> &str {
        \"first
}
type x
pub fn fake() {}\"
    }

    pub fn after(&self) {}
}
";
    assert_eq!(names(src), ["App.text", "App.after"]);
    let failures = surface(src, &["App.text", "App.after"], &[]);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "App.after (crates/app/src/web.rs:11) has no doc comment",
    );
}

// The two layouts `cargo fmt` writes that the line parser failed on. Both must pass.

#[test]
fn a_where_clause_and_a_wrapped_field_type_pass() {
    let src = "\
#[wasm_bindgen]
pub struct S {
    cache: std::collections::BTreeMap<
        String,
        Vec<u8>,
    >,
    /// Wrapped and public.
    pub wrapped: std::collections::BTreeMap<
        String,
        Vec<u8>,
    >,
}

#[wasm_bindgen]
impl S {
    /// Maps a value.
    pub fn map<F>(&self, f: F) -> u8
    where
        F: Fn(u8) -> u8,
    {
        f(1)
    }

    /// Makes one.
    pub async fn create<T>(
        value: T,
    ) -> Result<S, JsError>
    where
        T: Into<u8>,
    {
        todo!()
    }

    fn private<T>(&self)
    where
        T: Copy,
    {
    }
}
";
    let parsed = parse_exports(src);
    assert!(parsed.problems.is_empty(), "{:#?}", parsed.problems);
    assert_eq!(names(src), ["S.wrapped", "S.map", "S.create"]);
    let failures = surface(src, &["S.wrapped", "S.map", "S.create"], &[]);
    assert!(failures.is_empty(), "{failures:#?}");
}
