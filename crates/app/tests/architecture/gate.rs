//! Check 20: the edit gate runs once per action a person takes (decision 0019, rules 3 and 4). The
//! functions in `crates/*/src` that call `gate` are exactly `GATED`, and none is in
//! `crates/app/src/edit.rs` or is `run`, `apply` or `run_group`, which a drag frame and a rollback
//! use. A read of no files fails, since no listed function is found. A call that goes round the method
//! call (a path call, a macro) is not seen. A gate call takes exactly one argument, the plan: with
//! none or two it is not counted, so its action reads as one that never gates.

use crate::architecture::support::*;
use syn::{ExprMethodCall, ImplItemFn, ItemFn, visit::Visit};

const GATED: &str = "discard_whatif edit edit_group insert_many link_release pointer_down pointer_move redo set_task_fields undo";

#[derive(Default)]
struct Calls(Vec<String>, Vec<String>);

impl<'ast> Visit<'ast> for Calls {
    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.0.push(item.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, item);
        self.0.pop();
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.0.push(item.sig.ident.to_string());
        syn::visit::visit_item_fn(self, item);
        self.0.pop();
    }

    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        if call.method == "gate" && call.args.len() == 1 {
            self.1.extend(self.0.last().cloned());
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}

fn check_gate_calls(files: &[SourceFile]) -> Vec<String> {
    let (mut failures, mut gated) = (Vec::new(), Vec::new());
    for file in files {
        let mut calls = Calls::default();
        calls.visit_file(&syn::parse_file(&file.text).expect("a source file parses"));
        for name in calls.1 {
            if file.path.ends_with("src/edit.rs") || !GATED.split(' ').any(|n| n == name) {
                failures.push(format!(
                    "{}::{name} calls gate, and is no action",
                    file.path
                ));
            }
            gated.push(name);
        }
    }
    for name in GATED
        .split(' ')
        .filter(|name| !gated.iter().any(|g| g == name))
    {
        failures.push(format!("{name} does not call gate (check 20 in main.rs)"));
    }
    failures
}

#[test]
fn the_gate_runs_once_per_action_and_never_in_the_edit_core() {
    let failures = check_gate_calls(&rust_sources());
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_gate_call_in_run_or_a_missing_one_fails() {
    let text =
        "impl S { fn run(&mut self) { self.gate(p)?; } fn edit(&mut self) { self.gate(p)?; } }";
    let failures = check_gate_calls(&[file("crates/app/src/history.rs", text)]);
    one(&failures, "history.rs::run calls gate, and is no action");
    one(&failures, "undo does not call gate");
    assert_eq!(failures.len(), GATED.split(' ').count());
}
