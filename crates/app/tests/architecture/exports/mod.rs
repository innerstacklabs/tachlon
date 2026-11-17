//! Checks 1, 2, 3 and 6 (the Rust side) and the `wasm_bindgen` half of check 4: the exports of the
//! wasm module against the page.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{Ident, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprLit, ImplItem, ItemExternCrate, ItemFn, ItemImpl, ItemStruct, ItemUse,
    Lit, Macro, Meta, Type, UseTree, Visibility,
};

use crate::architecture::support::*;

mod layout_tests;
mod reader_tests;
mod surface_tests;

/// The failures of `src` read as `web.rs` against a page listing `public` and `test_only`.
fn surface(src: &str, public: &[&str], test_only: &[&str]) -> Vec<String> {
    check_surface(&web_rs(src), &page(public, test_only))
}

fn names(src: &str) -> Vec<String> {
    parse_exports(src)
        .exports
        .into_iter()
        .map(|e| e.name)
        .collect()
}

// ---------------------------------------------------------------------------
// Reading `web.rs`

#[derive(Debug, PartialEq)]
struct Export {
    /// `App.find`, `LoadedSchedule.parse`, `Bars.count` (a `pub` field) or `last_panic`.
    name: String,
    /// 1-based line of the `fn` name or the field name.
    line: usize,
    documented: bool,
}

/// What `parse_exports` read.
struct Parsed {
    exports: Vec<Export>,
    /// A `#[wasm_bindgen]` the reader cannot turn into an export, or a file it
    /// cannot parse. Each one fails the test, so a form the reader does not know
    /// is never a silent pass.
    problems: Vec<String>,
}

/// The exports of `src`, read from its syntax tree with `syn`, so the layout of the
/// text (comments, wrapped lines, multi-line strings, `where` clauses) cannot matter.
/// An export is:
///
/// - a `pub fn` (also `pub async fn`) inside a `#[wasm_bindgen] impl X` block, or any
///   fn carrying its own `#[wasm_bindgen]`, named `X.name`;
/// - a free fn carrying `#[wasm_bindgen]`, named `name`;
/// - a `pub` field of a `#[wasm_bindgen]` struct (wasm-bindgen exports it as an
///   accessor), named `Struct.field`.
///
/// Items are found anywhere in the file: inside an inline `mod`, or in a fn body.
///
/// Fail closed: a `#[wasm_bindgen]` on anything else (an enum, a const, a trait), a
/// `wasm_bindgen` name inside a macro body or another attribute (a `cfg_attr`), a
/// `pub fn` in an impl whose type is not a plain path, a macro in an exported impl,
/// or a file that does not parse, is a problem.
///
/// `path` is the file's repository-relative path; it starts every message.
fn parse_exports_in(path: &str, src: &str) -> Parsed {
    let mut reader = Reader {
        file: path.to_string(),
        ..Reader::default()
    };
    match syn::parse_file(src) {
        Ok(file) => reader.visit_file(&file),
        Err(e) => reader.problems.push(format!(
            "{path}:{} does not parse as Rust ({e}); the test reads the syntax tree, so it \
             cannot check this file",
            e.span().start().line
        )),
    }
    let unclaimed: Vec<(usize, usize)> = reader.seen.difference(&reader.claimed).copied().collect();
    for (line, _) in unclaimed {
        reader.problems.push(format!(
            "{path}:{line} has a #[wasm_bindgen] on an item the test in \
             crates/app/tests/architecture/exports does not read (it reads impl blocks, fns and \
             structs); extend the test, then this item"
        ));
    }
    reader.exports.sort_by_key(|e| e.line);
    reader.problems.sort();
    Parsed {
        exports: reader.exports,
        problems: reader.problems,
    }
}

/// `src` read as `crates/app/src/web.rs`: the form the self-tests use.
fn parse_exports(src: &str) -> Parsed {
    parse_exports_in(WEB_RS, src)
}

#[derive(Default)]
struct Reader {
    /// Repository-relative path of the file being read, for messages.
    file: String,
    exports: Vec<Export>,
    problems: Vec<String>,
    /// (line, column) of every `#[wasm_bindgen]` the walk met.
    seen: BTreeSet<(usize, usize)>,
    /// The ones an item the reader knows took.
    claimed: BTreeSet<(usize, usize)>,
}

/// `#[wasm_bindgen]` or a path ending in `wasm_bindgen`, with or without arguments
/// (the arguments are check 4's business).
fn is_wasm_bindgen(attr: &Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|s| named(&s.ident, "wasm_bindgen"))
}

/// An identifier's name without the `r#` of a raw identifier: `r#wasm_bindgen` is
/// `wasm_bindgen` to the compiler, so every name comparison goes through here.
fn bare(ident: &Ident) -> String {
    ident.unraw().to_string()
}

fn named(ident: &Ident, name: &str) -> bool {
    bare(ident) == name
}

fn path_is(path: &syn::Path, name: &str) -> bool {
    path.get_ident().is_some_and(|i| named(i, name))
}

fn position(attr: &Attribute) -> (usize, usize) {
    let start = attr.pound_token.span().start();
    (start.line, start.column)
}

/// A doc comment with text: `///` or `/** */` (both become `#[doc = "..."]`).
fn documented(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| match &a.meta {
        Meta::NameValue(nv) if path_is(&nv.path, "doc") => matches!(
            &nv.value,
            Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) if !s.value().trim().is_empty()
        ),
        _ => false,
    })
}

/// The line of the first `wasm_bindgen` name in `tokens`, groups included.
fn find_wasm_bindgen(tokens: TokenStream) -> Option<usize> {
    for tree in tokens {
        match tree {
            TokenTree::Ident(i) if named(&i, "wasm_bindgen") => return Some(i.span().start().line),
            TokenTree::Group(g) => {
                if let Some(line) = find_wasm_bindgen(g.stream()) {
                    return Some(line);
                }
            }
            _ => {}
        }
    }
    None
}

fn alias_problem(file: &str, alias: &Ident) -> String {
    format!(
        "{file}:{} renames wasm_bindgen to `{alias}`, which hides exports behind \
         `#[{alias}]`; the test reads the bare #[wasm_bindgen] only (check 4); drop the \
         `as {alias}`, or extend the test",
        alias.span().start().line
    )
}

/// Check 4: a `use` that renames `wasm_bindgen` (`use wasm_bindgen::prelude::wasm_bindgen
/// as wb;`, also inside braces) is a problem. Other `use wasm_bindgen::..` items are fine.
fn alias_of_wasm_bindgen(file: &str, tree: &UseTree, problems: &mut Vec<String>) {
    match tree {
        UseTree::Rename(r) if named(&r.ident, "wasm_bindgen") => {
            problems.push(alias_problem(file, &r.rename));
        }
        UseTree::Path(p) => alias_of_wasm_bindgen(file, &p.tree, problems),
        UseTree::Group(g) => {
            for t in &g.items {
                alias_of_wasm_bindgen(file, t, problems);
            }
        }
        _ => {}
    }
}

impl Reader {
    /// True when `attrs` carries a `#[wasm_bindgen]`; marks each one read.
    fn claim(&mut self, attrs: &[Attribute]) -> bool {
        let mut any = false;
        for a in attrs.iter().filter(|a| is_wasm_bindgen(a)) {
            self.claimed.insert(position(a));
            any = true;
        }
        any
    }

    fn export(&mut self, name: String, ident: &Ident, attrs: &[Attribute]) {
        self.exports.push(Export {
            name,
            line: ident.span().start().line,
            documented: documented(attrs),
        });
    }
}

impl<'ast> Visit<'ast> for Reader {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        if is_wasm_bindgen(attr) {
            self.seen.insert(position(attr));
            // Check 4: the bare form only. Arguments (`js_name = ..`) or a path
            // (`#[wasm_bindgen::prelude::wasm_bindgen]`) are not read.
            if !matches!(&attr.meta, Meta::Path(p) if path_is(p, "wasm_bindgen")) {
                self.problems.push(format!(
                    "{}:{} has a #[wasm_bindgen] that is not the bare attribute; the test \
                     reads the bare #[wasm_bindgen] only (check 4); write it bare, or extend \
                     the test",
                    self.file,
                    position(attr).0
                ));
            }
        } else if let Meta::List(list) = &attr.meta
            && let Some(line) = find_wasm_bindgen(list.tokens.clone())
        {
            self.problems.push(format!(
                "{file}:{line} names wasm_bindgen inside another attribute (a cfg_attr?); \
                 the test reads the bare #[wasm_bindgen] only; extend the test, then this line",
                file = self.file
            ));
        }
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        alias_of_wasm_bindgen(&self.file, &item.tree, &mut self.problems);
        visit::visit_item_use(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast ItemExternCrate) {
        if named(&item.ident, "wasm_bindgen")
            && let Some((_, alias)) = &item.rename
        {
            self.problems.push(alias_problem(&self.file, alias));
        }
        visit::visit_item_extern_crate(self, item);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        if let Some(line) = find_wasm_bindgen(mac.tokens.clone()) {
            self.problems.push(format!(
                "{file}:{line} names wasm_bindgen inside a macro body, which the test does \
                 not expand; write the item out, or extend the test",
                file = self.file
            ));
        }
        visit::visit_macro(self, mac);
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        let bound = self.claim(&item.attrs);
        let ty = match &*item.self_ty {
            Type::Path(p) if p.qself.is_none() => p.path.segments.last().map(|s| bare(&s.ident)),
            _ => None,
        };
        for member in &item.items {
            match member {
                ImplItem::Fn(f) => {
                    let own = self.claim(&f.attrs);
                    if own || (bound && matches!(f.vis, Visibility::Public(_))) {
                        if let Some(ty) = &ty {
                            self.export(
                                format!("{ty}.{}", bare(&f.sig.ident)),
                                &f.sig.ident,
                                &f.attrs,
                            );
                        } else {
                            self.problems.push(format!(
                                "{}:{} exports `{}` from an impl whose type is not a \
                                 plain path; the test cannot name it; extend the test",
                                self.file,
                                f.sig.ident.span().start().line,
                                f.sig.ident
                            ));
                        }
                    }
                }
                ImplItem::Macro(m) if bound => {
                    self.problems.push(format!(
                        "{}:{} has a macro inside an exported impl, which the test does \
                         not expand; write the fns out, or extend the test",
                        self.file,
                        m.mac.path.span().start().line
                    ));
                }
                _ => {}
            }
        }
        visit::visit_item_impl(self, item);
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        if self.claim(&item.attrs) {
            self.export(bare(&item.sig.ident), &item.sig.ident, &item.attrs);
        }
        visit::visit_item_fn(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast ItemStruct) {
        if self.claim(&item.attrs) {
            for (i, field) in item.fields.iter().enumerate() {
                if !matches!(field.vis, Visibility::Public(_)) {
                    continue;
                }
                // A tuple field has no name of its own: its index, and the line of its type.
                let (member, line) = match &field.ident {
                    Some(id) => (bare(id), id.span().start().line),
                    None => (i.to_string(), field.ty.span().start().line),
                };
                self.exports.push(Export {
                    name: format!("{}.{member}", bare(&item.ident)),
                    line,
                    documented: documented(&field.attrs),
                });
            }
        }
        visit::visit_item_struct(self, item);
    }
}

/// `line` without a trailing `//` comment. A `//` inside a string literal stays;
/// a line the scan cannot split (a `'"'` char literal) keeps its comment, so an
/// item header on it fails closed instead of passing.
fn without_line_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if c == '"' {
            in_string = true;
        } else if line[i..].starts_with("//") {
            return &line[..i];
        }
    }
    line
}

/// Check 4 outside `web.rs`: any `wasm_bindgen` in code (a `//` comment is skipped).
/// `web.rs` itself is checked on its syntax tree, in `parse_exports`.
pub(crate) fn hidden_exports(file: &str, src: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let t = without_line_comment(raw).trim();
        if t.starts_with("//") || find_bounded(t, "wasm_bindgen").is_none() {
            continue;
        }
        found.push(format!(
            "{file}:{} uses wasm_bindgen; exports belong only in {WEB_RS} or under {WEB_DIR}/",
            i + 1
        ));
    }
    found
}

// ---------------------------------------------------------------------------
// Checks 1, 2, 3 and 6 (the Rust side)

/// `web` is the wasm module: `crates/app/src/web.rs` and every file under
/// `crates/app/src/web/` (`is_web_module`), so the module can split into files.
#[expect(
    clippy::too_many_lines,
    reason = "the checks run in one pass and read in order"
)]
pub(crate) fn check_surface(web: &[SourceFile], page: &Page) -> Vec<String> {
    let mut failures = Vec::new();
    let mut exports: Vec<(&str, Export)> = Vec::new();
    for file in web {
        let Parsed {
            exports: found,
            problems,
        } = parse_exports_in(&file.path, &file.text);
        failures.extend(problems);
        exports.extend(found.into_iter().map(|e| (file.path.as_str(), e)));
    }
    if exports.is_empty() {
        failures.push(format!(
            "{WEB_RS} (whole module): found 0 #[wasm_bindgen] exports; the parser reads \
             nothing, so this test would pass on an empty parse"
        ));
    }
    let mut first: BTreeMap<&str, (&str, usize)> = BTreeMap::new();
    for (file, export) in &exports {
        if let Some((there, line)) = first.insert(&export.name, (file, export.line))
            && there != *file
        {
            failures.push(format!(
                "{} is exported at {there}:{line} and at {file}:{}; the page has one row per name",
                export.name, export.line
            ));
        }
    }
    if page.rows.is_empty() {
        failures.push(format!(
            "{PAGE} (whole file): found 0 rows under \"Public\" and \"Test-only\"; \
             the parser reads nothing, so this test would pass on an empty parse"
        ));
    }
    let mut listed: BTreeMap<&str, &Row> = BTreeMap::new();
    for row in &page.rows {
        if let Some(first) = listed.insert(&row.name, row) {
            failures.push(format!(
                "{PAGE}:{} lists {} again; it is already listed at {PAGE}:{}",
                row.line, row.name, first.line
            ));
        }
    }
    let exported: BTreeSet<&str> = exports.iter().map(|(_, e)| e.name.as_str()).collect();
    for (file, export) in &exports {
        if !listed.contains_key(export.name.as_str()) {
            failures.push(format!(
                "{} is exported at {file}:{} but is not in {PAGE}; add a row under \
                 \"Public\", or under \"Test-only\" with a test_ name",
                export.name, export.line
            ));
        }
        if !export.documented {
            failures.push(format!(
                "{} ({file}:{}) has no doc comment",
                export.name, export.line
            ));
        }
    }
    for row in &page.rows {
        if !exported.contains(row.name.as_str()) {
            failures.push(format!(
                "{PAGE}:{} lists {} but the wasm module ({WEB_RS}, {WEB_DIR}/) exports no such \
                 item",
                row.line, row.name
            ));
        }
        let member = row.name.rsplit('.').next().unwrap_or(&row.name);
        let prefixed = member.starts_with("test_");
        if row.test_only && !prefixed {
            failures.push(format!(
                "{} is listed Test-only but is not named test_* ({PAGE}:{})",
                row.name, row.line
            ));
        } else if !row.test_only && prefixed {
            failures.push(format!(
                "{} is listed Public but is named test_*; move it under \"Test-only\" \
                 ({PAGE}:{})",
                row.name, row.line
            ));
        }
    }
    failures
}
