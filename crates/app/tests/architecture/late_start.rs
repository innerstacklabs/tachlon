//! Check 18: a late start is read in one place (decision 0018, rule 7). In every `crates/*/src`, the name
//! `ls` (a task's dates field) is allowed only as a field named in a struct literal, in the engine's own
//! computation (`cpm.rs`, `cpm/`), in the error's own `ls` field (`model/error.rs`) and inside
//! `Schedule::late_start_shown`, allowed by name. Any other use, even a destructuring or a local, fails with
//! file and line. Not seen: `ls: name` as a pattern inside a macro, `format!("{ls}")` inline captures, a read
//! outside `crates/*/src`, and a name built at run time. An empty scan fails.

use crate::architecture::support::*;
use proc_macro2::{Spacing, TokenStream, TokenTree};
use syn::{FieldValue, Ident, ImplItemFn, Macro, visit::Visit};

const ALLOWED: [&str; 2] = ["crates/engine/src/cpm", "crates/engine/src/model/error.rs"];
const ACCESSOR: &str = "crates/engine/src/model/options.rs";
const FAILURE: &str = "names `ls` outside `Schedule::late_start_shown` (check 18 in main.rs)";

struct Reads {
    in_options: bool,
    in_accessor: bool,
    lines: Vec<usize>,
}

impl<'ast> Visit<'ast> for Reads {
    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.in_accessor = self.in_options && item.sig.ident == "late_start_shown";
        syn::visit::visit_impl_item_fn(self, item);
        self.in_accessor = false;
    }

    /// The field named in a struct literal builds dates; its value is still read.
    fn visit_field_value(&mut self, field: &'ast FieldValue) {
        self.visit_expr(&field.expr);
    }

    fn visit_ident(&mut self, ident: &'ast Ident) {
        if ident == "ls" && !self.in_accessor {
            self.lines.push(ident.span().start().line);
        }
    }

    /// A macro's arguments are tokens: an `ls` not followed by `:` is a read.
    fn visit_macro(&mut self, mac: &'ast Macro) {
        if !self.in_accessor {
            macro_reads(mac.tokens.clone(), &mut self.lines);
        }
    }
}

/// Walks the token trees, groups included: `f(d.ls)` and `(d.es, d.ls)` hold the read inside a group.
fn macro_reads(tokens: TokenStream, lines: &mut Vec<usize>) {
    let mut stream = tokens.into_iter().peekable();
    while let Some(tree) = stream.next() {
        match tree {
            TokenTree::Group(group) => macro_reads(group.stream(), lines),
            TokenTree::Ident(ident) if ident == "ls" => {
                let named = matches!(stream.peek(), Some(TokenTree::Punct(p))
                    if p.as_char() == ':' && p.spacing() == Spacing::Alone);
                if !named {
                    lines.push(ident.span().start().line);
                }
            }
            _ => {}
        }
    }
}

fn check_late_start(files: &[SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    if files.is_empty() {
        failures.push("crates/*/src: found 0 Rust files; the scan reads nothing".into());
    }
    for file in files {
        if ALLOWED.iter().any(|p| file.path.starts_with(p)) {
            continue;
        }
        let mut reads = Reads {
            in_options: file.path == ACCESSOR,
            in_accessor: false,
            lines: vec![],
        };
        match syn::parse_file(&file.text) {
            Ok(tree) => reads.visit_file(&tree),
            Err(error) => failures.push(format!("{}: does not parse: {error}", file.path)),
        }
        reads.lines.dedup();
        let path = &file.path;
        failures.extend(reads.lines.iter().map(|n| format!("{path}:{n} {FAILURE}")));
    }
    failures
}

#[test]
fn a_late_start_is_read_only_through_the_accessor() {
    let failures = check_late_start(&rust_sources());
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_planted_read_and_an_empty_scan_fail_but_a_struct_literal_passes() {
    let planted = "fn a(d: &D) -> D {
    let x = d.ls;
    let TaskDates { ls, .. } = *d;
    let TaskDates { ls: late, .. } = *d;
    println!(\"{}\", d.ls); // d.ls in a comment
    println!(\"{}\", f(d.ls));
    assert_eq!((d.es, d.ls), x);
    D { ls: d.es, es: 1 }
}";
    let failures = check_late_start(&[file("crates/app/src/a.rs", planted)]);
    assert_eq!(failures.len(), 6, "{failures:#?}");
    for line in [2, 3, 4, 5, 6, 7] {
        one(&failures, &format!("a.rs:{line} names"));
    }
    one(&check_late_start(&[]), "found 0 Rust files");
}

#[test]
fn the_allowance_is_for_the_accessor_by_name() {
    let accessor = read(ACCESSOR);
    let real = check_late_start(&[file(ACCESSOR, &accessor)]);
    assert!(real.is_empty(), "{real:#?}");
    let renamed = accessor.replace("fn late_start_shown(", "fn late_start(");
    one(&check_late_start(&[file(ACCESSOR, &renamed)]), "options.rs");
}
