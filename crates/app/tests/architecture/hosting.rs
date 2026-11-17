//! Check 12: the hosted app is a separate project (gs#537, decision 0007).
//!
//! `apps/hosted/` is a Vite project of its own that loads the chart as a customer does: the built
//! package, `gantt-core` (`packages/gantt`, linked in its own install, gantt#1090). Three rules keep it so:
//!
//! - *The component names nothing about hosting.* No file of the component (`crates/*/src/**`,
//!   `crates/*/Cargo.toml`, `scripts/build-web.sh`, and every file under `packages/gantt/` at any extension,
//!   the lockfile and `tests/fixtures/` included, but `node_modules/` and the paths
//!   `packages/gantt/.gitignore` names: `packages/gantt/src/wasm/`, `packages/gantt/dist/`, `packages/gantt/test-results/`,
//!   `packages/gantt/playwright-report/`) holds `apps/hosted`, `pages.dev`, `gantt-app`, `sample=` or
//!   `samples/`, and no module specifier in them holds `apps/`. Comments count: a comment that
//!   names the hosting is coupling too.
//! - *The hosted app imports only the public entry.* Every module specifier in
//!   `apps/hosted/src/**` and `apps/hosted/*.config.ts` (`import`, `import type`, `import "x"`,
//!   `export … from`, `export * from`, `import("x")`, `import.meta.glob("x")`, also with a type argument as in `import.meta.glob<string>("x")`,
//!   `require("x")`), every
//!   `@import` of a `.css` file there, and every `<script src>`
//!   and `<link href>` of `apps/hosted/index.html`, is `gantt-core` (only its `gantt.css` under it), a
//!   `node:` module, a relative path that stays inside `apps/hosted/`, or a package that
//!   `apps/hosted/package.json` declares, and that file declares `gantt-core`. A specifier that names `packages/gantt/`, `wasm/app`, `crates/` or
//!   `demos/` fails. `vite.config.ts` and `tsconfig.json` name neither `@gantt/chart` nor `gantt-core`: no alias
//!   sends the app to the source. The same holds for `apps/chrome/src`, the toolbar the hosted app and the developer page
//!   share, against `apps/chrome/package.json`. The `web_sources` rule holds in `apps/hosted/src` too.
//!   No selector there starts with `.gantt`.
//! - *The cloud API has one door* (decision 0011). No file of the component names `/api/docs`, and
//!   in `apps/hosted/src` only `store.ts` names it or imports a path inside `apps/cloud`
//!   (`hosting/cloud.rs`). Neither `package.json` declares a dependency that resolves into
//!   `apps/cloud`.
//!
//! An empty parse fails: no TypeScript file in `apps/hosted/src`, no alias line, or no file in one
//! of the component's groups. The size budget covers the hosted app in `size.rs`.
//!
//! Threat model: a good-faith author who forgets. A specifier built at run time
//! (`import("../" + name)`) is not caught; this is a text test. `node:` is allowed because
//! `vite.config.ts` and `playwright.config.ts` import it.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use alias::alias_failures;
use specifiers::{css_imports, html_refs, module_specifiers};

use crate::architecture::style_scope::{rule_preludes, split_selectors, style_blocks};
use crate::architecture::support::*;
use crate::architecture::web_sources::check_web_sources;

const HOSTED: &str = "apps/hosted";
/// The one way in to the chart: the built package, by the name the apps' manifests give it.
const PACKAGE_NAME: &str = "gantt-core";
/// The stylesheet that ships beside it.
const STYLES: &str = "gantt-core/gantt.css";
const CHROME: &str = "apps/chrome";
const CHROME_PACKAGE: &str = "apps/chrome/package.json";
const VITE: &str = "apps/hosted/vite.config.ts";
const TSCONFIG: &str = "apps/hosted/tsconfig.json";
const PACKAGE: &str = "apps/hosted/package.json";
const HOSTED_HTML: &str = "apps/hosted/index.html";

/// What the component may not name (compared in lower case).
const NEEDLES: [&str; 5] = [
    "apps/hosted",
    "pages.dev",
    "gantt-app",
    "sample=",
    "samples/",
];

/// Directory names never entered.
const SKIPPED: [&str; 1] = ["node_modules"];

/// What `packages/gantt/.gitignore` names (generated or installed, not the component's text); `node_modules`
/// is in `SKIPPED`. Everything else under `packages/gantt/` is read, at any extension: the lockfile and
/// `tests/fixtures/` too.
const WEB_SKIPPED: [&str; 4] = [
    WASM_DIR,
    concat!(web_root!(), "/dist"),
    concat!(web_root!(), "/test-results"),
    concat!(web_root!(), "/playwright-report"),
];

const SCRIPT_EXTENSIONS: [&str; 7] = ["ts", "tsx", "mts", "cts", "js", "mjs", "cjs"];

/// The files the check reads: the component's, and the hosted app's with the chrome's.
pub(crate) struct Tree {
    pub(crate) component: Vec<SourceFile>,
    pub(crate) hosted: Vec<SourceFile>,
}

// ---------------------------------------------------------------------------
// Reading the files

fn read_text(base: &Path, rel: &str) -> Option<String> {
    let path = base.join(rel);
    path.is_file().then(|| {
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"));
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Every file under `rel` in the directory `base`, at any extension, sorted; a missing directory
/// holds none, which the callers turn into a failure. `skip` lists the repository-relative
/// directories never entered, besides every `node_modules`.
fn walk(base: &Path, rel: &str, skip: &[&str], out: &mut Vec<SourceFile>) {
    let Ok(entries) = fs::read_dir(base.join(rel)) else {
        return;
    };
    let mut paths: Vec<_> = entries
        .map(|e| e.expect("a directory entry is readable").path())
        .collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().and_then(|n| n.to_str()).expect("a name");
        let child = format!("{rel}/{name}");
        if path.is_dir() {
            if !SKIPPED.contains(&name) && !skip.contains(&child.as_str()) {
                walk(base, &child, skip, out);
            }
        } else {
            out.extend(read_text(base, &child).map(|text| SourceFile { path: child, text }));
        }
    }
}

/// The files directly in `dir` whose name ends with `suffix`.
fn top_files(dir: &str, suffix: &str, out: &mut Vec<SourceFile>) {
    let Ok(entries) = fs::read_dir(root().join(dir)) else {
        return;
    };
    let mut names: Vec<String> = entries
        .map(|e| e.expect("a directory entry is readable"))
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.ends_with(suffix))
        .collect();
    names.sort();
    for name in names {
        push_file(&format!("{dir}/{name}"), out);
    }
}

fn push_file(rel: &str, out: &mut Vec<SourceFile>) {
    if let Some(text) = read_text(&root(), rel) {
        out.push(SourceFile {
            path: rel.to_string(),
            text,
        });
    }
}

fn read_tree() -> Tree {
    let mut component = Vec::new();
    let mut crates: Vec<_> = fs::read_dir(root().join("crates"))
        .expect("crates/ is readable")
        .map(|e| e.expect("a directory entry is readable").path())
        .collect();
    crates.sort();
    for krate in crates {
        let name = krate.file_name().and_then(|n| n.to_str()).expect("a name");
        walk(&root(), &format!("crates/{name}/src"), &[], &mut component);
        push_file(&format!("crates/{name}/Cargo.toml"), &mut component);
    }
    walk(&root(), web_root!(), &WEB_SKIPPED, &mut component);
    push_file("scripts/build-web.sh", &mut component);

    let mut hosted = Vec::new();
    walk(&root(), "apps/hosted/src", &[], &mut hosted);
    top_files(HOSTED, ".config.ts", &mut hosted);
    for rel in [HOSTED_HTML, PACKAGE, TSCONFIG] {
        push_file(rel, &mut hosted);
    }
    walk(&root(), "apps/chrome/src", &[], &mut hosted);
    push_file(CHROME_PACKAGE, &mut hosted);
    Tree { component, hosted }
}

// ---------------------------------------------------------------------------
// The component names nothing about hosting

fn is_script(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| SCRIPT_EXTENSIONS.contains(&ext))
}

/// The groups of files section 2 lists; each must find a file, so a moved root cannot empty the scan.
type Group = (&'static str, fn(&str) -> bool);
#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "the paths are repository file names, all lowercase"
)]
const GROUPS: [Group; 10] = [
    ("crates/*/src/**", |p| {
        p.starts_with("crates/") && p.contains("/src/")
    }),
    ("crates/*/Cargo.toml", |p| {
        p.starts_with("crates/") && p.ends_with("/Cargo.toml")
    }),
    (concat!(web_root!(), "/src/**"), |p| {
        p.starts_with(concat!(web_root!(), "/src/"))
    }),
    (concat!(web_root!(), "/index.html"), |p| {
        p == concat!(web_root!(), "/index.html")
    }),
    (concat!(web_root!(), "/*.config.ts"), |p| {
        p.strip_prefix(concat!(web_root!(), "/"))
            .is_some_and(|n| !n.contains('/') && n.ends_with(".config.ts"))
    }),
    (concat!(web_root!(), "/package.json"), |p| {
        p == concat!(web_root!(), "/package.json")
    }),
    ("scripts/build-web.sh", |p| p == "scripts/build-web.sh"),
    (concat!(web_root!(), "/tests/**/*.ts"), |p| {
        p.starts_with(concat!(web_root!(), "/tests/")) && p.ends_with(".ts")
    }),
    (concat!(web_root!(), "/tests/fixtures/**"), |p| {
        p.starts_with(concat!(web_root!(), "/tests/fixtures/"))
    }),
    (concat!(web_root!(), "/pnpm-lock.yaml"), |p| {
        p == concat!(web_root!(), "/pnpm-lock.yaml")
    }),
];

fn component_failures(files: &[SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    for (group, matches) in GROUPS {
        if !files.iter().any(|f| matches(&f.path)) {
            failures.push(format!(
                "the component scan found no file in {group}; it reads fewer files than the \
                 check lists, so a hosting name could hide in the files it misses"
            ));
        }
    }
    for file in files {
        for (i, line) in file.text.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            for needle in NEEDLES {
                if lower.contains(needle) {
                    failures.push(format!(
                        "{}:{} names {needle}; the component knows nothing about its hosting \
                         (docs/decisions/0007-hosted-app-separate-project.md)",
                        file.path,
                        i + 1
                    ));
                }
            }
        }
        if is_script(&file.path) {
            for (line, spec) in module_specifiers(&strip_ts_comments(&file.text)) {
                if spec.contains("apps/") && !spec.contains("apps/hosted") {
                    failures.push(format!(
                        "{}:{line} imports {spec}, which names apps/; the component knows \
                         nothing about its hosting",
                        file.path
                    ));
                }
            }
        }
    }
    failures
}

// ---------------------------------------------------------------------------
// The hosted app imports only the public entry

/// `rel` joined to the directory `dir` (both repository-relative, `/`-separated), with `.` and
/// `..` folded; none when it leaves the repository.
fn resolve(dir: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn names_component(spec: &str) -> bool {
    spec.contains("wasm/app")
        || spec
            .split('/')
            .any(|s| matches!(s, "packages" | "crates" | "demos"))
}

/// The failure for `spec` imported by `importer` in the project `home` (`apps/hosted` or
/// `apps/chrome`), if it is not an allowed import.
#[expect(
    clippy::similar_names,
    reason = "base is where the specifier resolves from, bare is the specifier"
)]
fn bad_specifier(
    home: &str,
    importer: &str,
    spec: &str,
    deps: &BTreeSet<String>,
) -> Option<String> {
    let bare = spec.split(['?', '#']).next().unwrap_or("");
    let outside = || {
        Some(format!(
            "imports {spec}; {home} imports only {PACKAGE_NAME}"
        ))
    };
    if bare.is_empty()
        || ["data:", "http:", "https:", "//", "mailto:", "node:"]
            .iter()
            .any(|p| bare.starts_with(p))
    {
        return None;
    }
    if names_component(bare) {
        return outside();
    }
    if bare == PACKAGE_NAME || bare == STYLES {
        return None;
    }
    if bare.starts_with("gantt-core/") {
        return outside();
    }
    if bare.starts_with('.') || bare.starts_with('/') {
        let base = if bare.starts_with('/') {
            home
        } else {
            dir_of(importer)
        };
        return match resolve(base, bare) {
            Some(p) if p == home || p.starts_with(&format!("{home}/")) => None,
            _ => outside(),
        };
    }
    let mut parts = bare.split('/');
    let first = parts.next().unwrap_or("");
    let name = if first.starts_with('@') {
        format!("{first}/{}", parts.next().unwrap_or(""))
    } else {
        first.to_string()
    };
    (!deps.contains(&name)).then(|| {
        format!(
            "imports {spec}, a package {home}/package.json does not declare; {home} imports only \
             {PACKAGE_NAME} and the packages it declares"
        )
    })
}

fn declared_packages(path: &str, package_json: &str) -> Result<BTreeSet<String>, String> {
    let value: serde_json::Value = serde_json::from_str(package_json).map_err(|e| {
        format!("{path} is not valid JSON ({e}); the check cannot read the declared packages")
    })?;
    let mut names = BTreeSet::new();
    for key in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(table) = value.get(key).and_then(|t| t.as_object()) {
            names.extend(table.keys().cloned());
        }
    }
    Ok(names)
}

/// Whether `selector` starts at the chart's root class (`.gantt…`, also inside `:where(` or `:is(`).
fn restyles_chart(selector: &str) -> bool {
    let mut s = selector.trim();
    while let Some(rest) = s.strip_prefix(":where(").or_else(|| s.strip_prefix(":is(")) {
        s = rest.trim_start();
    }
    s.starts_with(".gantt")
}

fn selector_failures(path: &str, css: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for (line, prelude) in rule_preludes(css) {
        for selector in split_selectors(&prelude) {
            if restyles_chart(selector) {
                failures.push(format!(
                    "{path}:{line} has the selector `{}`; no selector in {HOSTED} starts with \
                     .gantt, so the host page cannot restyle the chart",
                    selector.trim()
                ));
            }
        }
    }
    failures
}

/// The failures of the `@import`s of `css` (the file `path`, or the `<style>` blocks of the page).
fn css_import_failures(path: &str, css: &str, deps: &BTreeSet<String>) -> Vec<String> {
    css_imports(css)
        .into_iter()
        .filter_map(|(line, spec)| {
            bad_specifier(HOSTED, path, &spec, deps).map(|why| format!("{path}:{line} {why}"))
        })
        .collect()
}

/// The packages the manifest `path` declares. It must declare `gantt-core`: the app loads the
/// built package, so its own install holds it.
fn declared(files: &[SourceFile], path: &str, failures: &mut Vec<String>) -> BTreeSet<String> {
    let Some(file) = files.iter().find(|f| f.path == path) else {
        failures.push(format!(
            "{path}: not found; the declared packages cannot be read"
        ));
        return BTreeSet::new();
    };
    match declared_packages(path, &file.text) {
        Ok(names) => {
            if !names.contains(PACKAGE_NAME) {
                failures.push(format!(
                    "{path} does not declare {PACKAGE_NAME}; the built package is the one way in to the chart"
                ));
            }
            names
        }
        Err(e) => {
            failures.push(e);
            BTreeSet::new()
        }
    }
}

/// `apps/chrome/src`, the toolbar the hosted app and the developer page share, loads the chart
/// as the hosted app does: `gantt-core`, and what it declares.
fn chrome_failures(files: &[SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    let deps = declared(files, CHROME_PACKAGE, &mut failures);
    let sources: Vec<&SourceFile> = files
        .iter()
        .filter(|f| f.path.starts_with("apps/chrome/src/") && is_script(&f.path))
        .collect();
    if sources.is_empty() {
        failures.push(format!(
            "{CHROME}/src: found 0 TypeScript files; the scan reads nothing, so this test would \
             pass on an empty parse"
        ));
    }
    for file in sources {
        for (line, spec) in module_specifiers(&strip_ts_comments(&file.text)) {
            if let Some(why) = bad_specifier(CHROME, &file.path, &spec, &deps) {
                failures.push(format!("{}:{line} {why}", file.path));
            }
        }
    }
    failures
}

#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "the paths are repository file names, all lowercase"
)]
fn hosted_failures(files: &[SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    let in_src = |f: &&SourceFile| f.path.starts_with("apps/hosted/src/");
    let sources: Vec<&SourceFile> = files
        .iter()
        .filter(in_src)
        .filter(|f| is_script(&f.path))
        .collect();
    if sources.is_empty() {
        failures.push(format!(
            "{HOSTED}/src: found 0 TypeScript files; the scan reads nothing, so this test would \
             pass on an empty parse"
        ));
    }
    let deps = declared(files, PACKAGE, &mut failures);
    let configs = files.iter().filter(|f| {
        f.path.ends_with(".config.ts")
            && f.path.matches('/').count() == HOSTED.matches('/').count() + 1
    });
    for file in sources.iter().copied().chain(configs) {
        for (line, spec) in module_specifiers(&strip_ts_comments(&file.text)) {
            if cloud::names_cloud(&file.path, &spec) {
                // The store is the one door; any other file gets the cloud failure.
                if let Some(why) = cloud::import_failure(&file.path, &spec) {
                    failures.push(format!("{}:{line} {why}", file.path));
                }
                continue;
            }
            if let Some(why) = bad_specifier(HOSTED, &file.path, &spec, &deps) {
                failures.push(format!("{}:{line} {why}", file.path));
            }
        }
    }
    failures.extend(cloud::hosted_route_failures(&sources));
    match files.iter().find(|f| f.path == HOSTED_HTML) {
        Some(page) => {
            for (line, spec) in html_refs(&page.text) {
                if let Some(why) = bad_specifier(HOSTED, HOSTED_HTML, &spec, &deps) {
                    failures.push(format!("{HOSTED_HTML}:{line} {why}"));
                }
            }
            let styles = style_blocks(&page.text);
            failures.extend(selector_failures(HOSTED_HTML, &styles));
            failures.extend(css_import_failures(HOSTED_HTML, &styles, &deps));
        }
        None => failures.push(format!(
            "{HOSTED_HTML}: not found; the host page cannot be read"
        )),
    }
    for file in files
        .iter()
        .filter(in_src)
        .filter(|f| f.path.ends_with(".css"))
    {
        failures.extend(selector_failures(&file.path, &file.text));
        failures.extend(css_import_failures(&file.path, &file.text, &deps));
    }
    if !sources.is_empty() {
        let copies: Vec<SourceFile> = sources
            .iter()
            .map(|f| SourceFile {
                path: f.path.clone(),
                text: f.text.clone(),
            })
            .collect();
        failures.extend(check_web_sources(&copies, &[]));
    }
    failures.extend(alias_failures(files));
    failures
}

/// Every failure over `tree`.
pub(crate) fn check_tree(tree: &Tree) -> Vec<String> {
    let mut failures = component_failures(&tree.component);
    failures.extend(cloud::component_failures(&tree.component));
    failures.extend(hosted_failures(&tree.hosted));
    failures.extend(chrome_failures(&tree.hosted));
    let manifests: Vec<&SourceFile> = tree.component.iter().chain(&tree.hosted).collect();
    failures.extend(cloud::dependency_failures(&manifests));

    failures
}

#[test]
fn the_component_and_the_hosted_app_stay_apart() {
    let failures = check_tree(&read_tree());
    assert!(
        failures.is_empty(),
        "the hosting boundary fails ({} problems; docs/decisions/0007-hosted-app-separate-project.md \
         says why):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

mod alias;
mod cloud;
mod specifiers;
mod tests;
