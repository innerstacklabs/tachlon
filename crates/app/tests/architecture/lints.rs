//! Check 16: every workspace member opts in to the strict lints (gantt#408). The root `Cargo.toml`
//! names the members; each member's own `Cargo.toml` must hold `workspace = true` in its `[lints]`
//! table, or the root's `[workspace.lints.clippy]` rules do not reach it.

use crate::architecture::support::*;

/// The paths in the root manifest's `members = [...]` array.
fn members(root: &str) -> Vec<String> {
    let list = root
        .split("members = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next());
    let list = list.unwrap_or_default();
    let paths = list.split(',').map(|m| m.trim().trim_matches('"'));
    paths
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether a manifest has the line `workspace = true` in its `[lints]` table.
fn opts_in(manifest: &str) -> bool {
    let mut in_lints = false;
    for line in manifest
        .lines()
        .map(|l| l.split('#').next().unwrap_or_default().trim())
    {
        if line.starts_with('[') {
            in_lints = line == "[lints]";
        } else if in_lints && line.replace(' ', "") == "workspace=true" {
            return true;
        }
    }
    false
}

/// A failure for each member that does not opt in, and for a root that names no member.
fn check_lints(root: &str, manifest_of: impl Fn(&str) -> String) -> Vec<String> {
    let members = members(root);
    let mut failures = Vec::new();
    if members.is_empty() {
        failures.push("Cargo.toml: found 0 workspace members; the scan reads nothing".to_string());
    }
    for member in members {
        if !opts_in(&manifest_of(&format!("{member}/Cargo.toml"))) {
            failures.push(format!(
                "{member}/Cargo.toml has no `workspace = true` under `[lints]`; the strict lints do not reach `{member}`"
            ));
        }
    }
    failures
}

#[test]
fn every_crate_opts_in_to_the_strict_lints() {
    let failures = check_lints(&read("Cargo.toml"), read);
    assert!(
        failures.is_empty(),
        "a crate is off the strict lints ({} problems; check 16 in main.rs):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn a_member_without_the_line_fails_by_name_and_an_empty_root_fails() {
    let root = "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n";
    let manifest = |path: &str| match path {
        "crates/a/Cargo.toml" => "[lints]\nworkspace = true # on\n".to_string(),
        _ => "[lints.clippy]\nworkspace = true\n[lints]\n".to_string(),
    };
    let failures = check_lints(root, manifest);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, "crates/b/Cargo.toml");
    one(
        &check_lints("[workspace]\n", manifest),
        "found 0 workspace members",
    );
}
