//! Check 19: a host key is looked up in one place (decision 0016, rule 2). In `crates/app/src`, outside
//! `crates/app/src/host/keys.rs`, code fails and names the file and line when it reads a field of the key map
//! (`.keys.<field>`, `host_keys().<field>`; the methods are the way in and the fields are private, so the compiler
//! refuses this too), or, in a file that names `meta`, reads a record's `key` field inside a comparison (`==`, `!=`,
//! `eq`, `ne`) or a search or index call (`contains`, `contains_key`, `get`, `insert`, `entry`): a scan of the task
//! list for a key, or a second map keyed by it. A plain read of a key to write it out is allowed. Not seen: a key
//! held in a local or moved through a helper first, a lookup written as a `match` or `is_some_and`, a second map
//! keyed by a variable. The threat is a good-faith author who forgets. An empty read fails.

use crate::architecture::support::*;
use syn::{BinOp, Expr, ExprBinary, ExprField, ExprMethodCall, Member, visit::Visit};

const KEYS: &str = "crates/app/src/host/keys.rs";

fn is_search(method: &str) -> bool {
    matches!(
        method,
        "eq" | "ne" | "contains" | "contains_key" | "get" | "insert" | "entry"
    )
}

/// The key map: a field named `keys`, or a call of `host_keys()`.
fn is_map(expr: &Expr) -> bool {
    matches!(expr, Expr::Field(f) if named(&f.member) == "keys")
        || matches!(expr, Expr::MethodCall(c) if c.method == "host_keys")
}

#[derive(Default)]
struct Lookups<'a> {
    path: &'a str,
    meta: bool,
    scan: usize,
    found: Vec<String>,
}

fn named(member: &Member) -> String {
    match member {
        Member::Named(name) => name.to_string(),
        Member::Unnamed(_) => String::new(),
    }
}

impl<'ast> Visit<'ast> for Lookups<'_> {
    fn visit_expr_field(&mut self, field: &'ast ExprField) {
        let (name, line) = (named(&field.member), field.dot_token.spans[0].start().line);
        let what = if is_map(&field.base) {
            Some("reads a field of the key map")
        } else if name == "key" && self.meta && self.scan > 0 {
            Some("searches or indexes by a record's `key`")
        } else {
            None
        };
        if let Some(what) = what {
            self.found.push(format!(
                "{}:{line} {what} outside {KEYS} (check 19 in main.rs)",
                self.path
            ));
        }
        syn::visit::visit_expr_field(self, field);
    }

    fn visit_expr_binary(&mut self, binary: &'ast ExprBinary) {
        let compare = matches!(binary.op, BinOp::Eq(_) | BinOp::Ne(_));
        self.scan += usize::from(compare);
        syn::visit::visit_expr_binary(self, binary);
        self.scan -= usize::from(compare);
    }

    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        let search = is_search(&call.method.to_string());
        self.scan += usize::from(search);
        syn::visit::visit_expr_method_call(self, call);
        self.scan -= usize::from(search);
    }
}

fn check_key_lookups(files: &[SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    if files.is_empty() {
        failures.push("crates/app/src: found 0 Rust files; the scan reads nothing".to_string());
    }
    for file in files.iter().filter(|file| file.path != KEYS) {
        match syn::parse_file(&file.text) {
            Ok(tree) => {
                let meta = file.text.contains("meta");
                let mut lookups = Lookups {
                    path: &file.path,
                    meta,
                    ..Lookups::default()
                };
                lookups.visit_file(&tree);
                failures.extend(lookups.found);
            }
            Err(error) => failures.push(format!("{}: does not parse: {error}", file.path)),
        }
    }
    failures
}

#[test]
fn a_key_is_looked_up_only_in_keys_rs() {
    let mut files = Vec::new();
    collect("crates/app/src", &["rs"], "", &mut files);
    let failures = check_key_lookups(&files);
    assert!(
        failures.is_empty(),
        "a host key is looked up outside {KEYS} ({} problems; check 19 in main.rs):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn a_planted_lookup_fails_and_the_allowed_reads_pass() {
    let text = "fn a(s: &Session) { s.host.keys.slots.get(k); }
fn b(meta: &[TaskMeta]) { meta.iter().position(|m| m.key.as_deref() == Some(k)); }
fn c(meta: &[TaskMeta]) { by.insert(meta[0].key.clone(), 1); }
fn d(s: &Session) { s.host_keys().links.len(); }
fn e(meta: &[TaskMeta]) { out(meta[0].key.as_deref()); meta.iter().filter(|m| m.key.is_some()).count(); s.host_keys().link(k); s.host.keys.link_key(d); }";
    let failures = check_key_lookups(&[file("crates/app/src/other.rs", text), file(KEYS, text)]);
    assert_eq!(failures.len(), 4, "{failures:#?}");
    one(&failures, "other.rs:1 reads a field");
    one(&failures, "other.rs:2 searches");
    one(&failures, "other.rs:3 searches");
    one(&failures, "other.rs:4 reads a field");
}

#[test]
fn an_empty_read_fails() {
    one(&check_key_lookups(&[]), "found 0 Rust files");
}
