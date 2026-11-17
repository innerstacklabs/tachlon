//! Check 17: the undo and redo stacks change in one place (decision 0019, rule 1). In
//! `crates/app/src/history.rs` and `crates/app/src/history/`, any method call on either stack
//! other than a read (`back`, `last`, `len`, `is_empty`), or a call to `History::push`, is allowed only in
//! `impl History` and in `Session::commit`; anywhere else it fails and names the file and line. A write that
//! goes round the method call (`mem::take`, a macro, an assignment such as `self.history.redo = ...`, a
//! `&mut` borrow passed on, `with_history_cap`'s whole replacement) is not seen: the threat is a good-faith
//! author who forgets. An empty read fails.

use crate::architecture::support::*;
use syn::{Expr, ExprCall, ExprMethodCall, ImplItemFn, ItemImpl, Member, Type, visit::Visit};

const HISTORY: &str = "crates/app/src/history.rs";
const STACK_READS: [&str; 4] = ["back", "last", "len", "is_empty"];

struct Writes<'a> {
    path: &'a str,
    /// The type of the `impl` being read.
    owner: String,
    allowed: bool,
    found: Vec<String>,
}

impl Writes<'_> {
    fn flag(&mut self, line: usize, what: &str) {
        let (path, text) = (self.path, "outside Session::commit and impl History");
        self.found
            .push(format!("{path}:{line} {what} {text} (check 17 in main.rs)"));
    }
}

impl<'ast> Visit<'ast> for Writes<'_> {
    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        let name = match &*item.self_ty {
            Type::Path(path) => path.path.get_ident().map(ToString::to_string),
            _ => None,
        };
        let outer = std::mem::replace(&mut self.owner, name.unwrap_or_default());
        syn::visit::visit_item_impl(self, item);
        self.owner = outer;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        let outer = self.allowed;
        let commit = self.owner == "Session" && item.sig.ident == "commit";
        self.allowed = self.owner == "History" || commit;
        syn::visit::visit_impl_item_fn(self, item);
        self.allowed = outer;
    }

    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        let field = match &*call.receiver {
            Expr::Field(field) => match &field.member {
                Member::Named(name) => name.to_string(),
                Member::Unnamed(_) => String::new(),
            },
            _ => String::new(),
        };
        let method = call.method.to_string();
        let stack =
            matches!(field.as_str(), "undo" | "redo") && !STACK_READS.contains(&method.as_str());
        if !self.allowed && (stack || (field == "history" && method == "push")) {
            self.flag(
                call.method.span().start().line,
                &format!("`{field}.{method}` writes the history"),
            );
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if let Expr::Path(path) = &*call.func {
            let names: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if !self.allowed && names.ends_with(&["History".to_string(), "push".to_string()]) {
                self.flag(
                    path.path.segments[0].ident.span().start().line,
                    "`History::push` is called",
                );
            }
        }
        syn::visit::visit_expr_call(self, call);
    }
}

fn check_history_writes(files: &[SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    if files.is_empty() {
        failures.push(format!(
            "{HISTORY}: found 0 Rust files; the scan reads nothing"
        ));
    }
    for file in files {
        match syn::parse_file(&file.text) {
            Ok(tree) => {
                let mut writes = Writes {
                    path: &file.path,
                    owner: String::new(),
                    allowed: false,
                    found: Vec::new(),
                };
                writes.visit_file(&tree);
                failures.extend(writes.found);
            }
            Err(error) => failures.push(format!("{}: does not parse: {error}", file.path)),
        }
    }
    failures
}

#[test]
fn the_history_stacks_change_only_in_commit() {
    let mut files = vec![file(HISTORY, &read(HISTORY))];
    collect("crates/app/src/history", &["rs"], "", &mut files);
    let failures = check_history_writes(&files);
    assert!(
        failures.is_empty(),
        "the history stacks change outside commit ({} problems; check 17 in main.rs):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn a_planted_write_fails_and_the_allowed_writes_pass() {
    let text = "impl Session {
    fn undo(&mut self) { self.history.redo.push(x); self.history.undo.drain(..); self.history.undo.len(); }
    fn edit(&mut self) { self.history.push(x); History::push(h, x); }
    fn commit(&mut self) { self.history.undo.push_back(x); self.history.redo.pop(); }
}
impl History {
    fn push(&mut self) { self.undo.push_back(x); self.redo.clear(); }
}";
    let failures = check_history_writes(&[file(HISTORY, text)]);
    assert_eq!(failures.len(), 4, "{failures:#?}");
    one(&failures, "history.rs:2 `redo.push`");
    one(&failures, "history.rs:2 `undo.drain`");
    one(&failures, "history.rs:3 `history.push`");
    one(&failures, "history.rs:3 `History::push`");
}

#[test]
fn the_allowance_is_for_commit_by_name() {
    let path = "crates/app/src/history/commit.rs";
    let renamed = read(path).replace("fn commit(", "fn record(");
    let failures = check_history_writes(&[file(path, &renamed)]);
    assert!(!failures.is_empty(), "a renamed commit is still allowed");
    one(&failures, "`undo.pop_back` writes the history");
}

#[test]
fn an_empty_read_fails() {
    one(&check_history_writes(&[]), "found 0 Rust files");
}
