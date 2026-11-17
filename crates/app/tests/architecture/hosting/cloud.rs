//! The cloud rules of check 12 (decision 0011): the cloud API is server code in `apps/cloud`, and
//! the chart and the hosted page reach it by one door only.
//!
//! - The component (see `hosting.rs`) never names the API route `/api/docs`.
//! - In `apps/hosted/src`, only `store.ts` imports from `apps/cloud` or names `/api/docs`.
//! - Neither `apps/hosted/package.json` nor `packages/gantt/package.json` depends on `apps/cloud`, by a
//!   `workspace:`, `file:`, `link:` or `portal:` path or by the package name `@gantt/cloud`: a
//!   declared dependency would let any file import it.

use super::{dir_of, is_script, resolve};
use crate::architecture::support::*;

const CLOUD: &str = "apps/cloud";
const STORE: &str = "apps/hosted/src/store.ts";
const CLOUD_PACKAGE: &str = "@gantt/cloud";
const MANIFESTS: [&str; 2] = [
    "apps/hosted/package.json",
    concat!(web_root!(), "/package.json"),
];
const API: &str = "/api/docs";
const RECORD: &str = "docs/decisions/0011-cloud-api-in-this-repository.md";

/// Whether the specifier `spec`, imported by `importer`, names a path inside `apps/cloud`.
pub(super) fn names_cloud(importer: &str, spec: &str) -> bool {
    let bare = spec.split(['?', '#']).next().unwrap_or("");
    let path = if bare.starts_with('.') {
        resolve(dir_of(importer), bare)
    } else {
        Some(bare.to_string())
    };
    path.is_some_and(|p| p == CLOUD || p.starts_with("apps/cloud/"))
}

/// The lines of `files` that name the API route, as failures worded by `why`.
fn route_failures(files: &[&SourceFile], why: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for file in files {
        for (i, line) in file.text.lines().enumerate() {
            if line.to_ascii_lowercase().contains(API) {
                failures.push(format!(
                    "{}:{} names {API}; {why} ({RECORD})",
                    file.path,
                    i + 1
                ));
            }
        }
    }
    failures
}

/// Every file of the component that names the API route.
pub(super) fn component_failures(files: &[SourceFile]) -> Vec<String> {
    let all: Vec<&SourceFile> = files.iter().collect();
    route_failures(&all, "the component knows nothing about the cloud API")
}

/// The hosted source files other than `store.ts` that name the API route.
pub(super) fn hosted_route_failures(files: &[&SourceFile]) -> Vec<String> {
    let others: Vec<&SourceFile> = files.iter().copied().filter(|f| f.path != STORE).collect();
    route_failures(
        &others,
        "only apps/hosted/src/store.ts reaches the cloud API",
    )
}

/// The failure for `spec` imported by `importer` when it names `apps/cloud` and `importer` is not
/// the store; none when it does not name `apps/cloud`.
pub(super) fn import_failure(importer: &str, spec: &str) -> Option<String> {
    (is_script(importer) && names_cloud(importer, spec) && importer != STORE).then(|| {
        format!(
            "imports {spec}, which is in {CLOUD}; only {STORE} reaches the cloud API ({RECORD})"
        )
    })
}

/// Whether the dependency `name` with the version or path `spec`, declared in `manifest`, resolves
/// into `apps/cloud`: by name, by an `npm:` alias, or by a path-valued protocol.
fn depends_on_cloud(manifest: &str, name: &str, spec: &str) -> bool {
    let target = spec.strip_prefix("npm:").unwrap_or(spec);
    let by_name = name == CLOUD_PACKAGE
        || target
            .strip_prefix(CLOUD_PACKAGE)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('@'));
    let path = ["workspace:", "file:", "link:", "portal:"]
        .iter()
        .find_map(|p| spec.strip_prefix(p));
    by_name || path.is_some_and(|p| names_cloud(manifest, p))
}

/// The dependencies of the two manifests that resolve into `apps/cloud`.
pub(super) fn dependency_failures(files: &[&SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    for file in files
        .iter()
        .filter(|f| MANIFESTS.contains(&f.path.as_str()))
    {
        // A manifest that is not JSON is reported by the hosted checks; the web one is not read.
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&file.text) else {
            continue;
        };
        for key in ["dependencies", "devDependencies", "optionalDependencies"] {
            let table = value.get(key).and_then(|t| t.as_object());
            for (name, spec) in table.into_iter().flatten() {
                if depends_on_cloud(&file.path, name, spec.as_str().unwrap_or("")) {
                    failures.push(format!(
                        "{} {key} names {name}: {spec}, which is in {CLOUD}; no manifest \
                         declares it, so only {STORE} may import it ({RECORD})",
                        file.path
                    ));
                }
            }
        }
    }
    failures
}
