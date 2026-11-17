//! Self-tests of check 12. Tests 1 to 7 are the red breaks of the plan on gs#511 (section 2): each
//! one plants the mistake in a copy of a good tree and wants the failure that names file and line.

use super::*;
use crate::architecture::size::budget_failures;

const MAIN_TS: &str = "import { mount } from \"gantt-core\";\nimport { helper } from \"./samples\";\nmount(helper);\n";
const VITE_TS: &str = "import { defineConfig } from \"vite\";\n// The chart comes from the app's own install.\nexport default defineConfig({\n  base: \"./\",\n});\n";
const TSCONFIG_JSON: &str = "{\n  \"compilerOptions\": {\n    \"strict\": true\n  }\n}\n";
const PACKAGE_JSON: &str = "{\n  \"devDependencies\": { \"@playwright/test\": \"1\", \"vite\": \"8\" },\n  \"dependencies\": { \"gantt-core\": \"link:../../packages/gantt\", \"left-pad\": \"1\" }\n}\n";
const CHROME_JSON: &str =
    "{\n  \"dependencies\": { \"gantt-core\": \"link:../../packages/gantt\" }\n}\n";
const CHROME_TS: &str =
    "import type { Gantt } from \"gantt-core\";\nimport page from \"./page.html?raw\";\n";
const PAGE_HTML: &str = "<!doctype html>\n<html>\n<head>\n<link rel=\"icon\" href=\"data:,\" />\n<style>\n  body { margin: 0; }\n  .host-bar { height: 32px; }\n</style>\n</head>\n<body>\n<div id=\"app\" class=\"gantt\"></div>\n<script type=\"module\" src=\"/src/main.ts\"></script>\n</body>\n</html>\n";

fn tree() -> Tree {
    Tree {
        component: vec![
            file("crates/app/src/lib.rs", "pub fn a() {}\n"),
            file("crates/app/Cargo.toml", "[package]\nname = \"app\"\n"),
            file(
                concat!(web_root!(), "/src/main.ts"),
                "import \"./other\";\nexport const a = 1;\n",
            ),
            file(
                concat!(web_root!(), "/src/open.ts"),
                "export const open = 1;\n",
            ),
            file(concat!(web_root!(), "/index.html"), "<html></html>\n"),
            file(
                concat!(web_root!(), "/vite.config.ts"),
                "export default {};\n",
            ),
            file(concat!(web_root!(), "/package.json"), "{}\n"),
            file("scripts/build-web.sh", "echo build\n"),
            file(concat!(web_root!(), "/tests/open.spec.ts"), "test();\n"),
            file(concat!(web_root!(), "/tests/fixtures/one.xer"), "ERMHDR\n"),
            file(
                concat!(web_root!(), "/pnpm-lock.yaml"),
                "lockfileVersion: '9.0'\n",
            ),
        ],
        hosted: vec![
            file("apps/hosted/src/main.ts", MAIN_TS),
            file("apps/hosted/src/samples.ts", "export const helper = 1;\n"),
            file("apps/hosted/index.html", PAGE_HTML),
            file("apps/hosted/package.json", PACKAGE_JSON),
            file("apps/hosted/tsconfig.json", TSCONFIG_JSON),
            file("apps/hosted/vite.config.ts", VITE_TS),
            file(
                "apps/hosted/playwright.config.ts",
                "import { join } from \"node:path\";\nimport { defineConfig } from \"@playwright/test\";\n",
            ),
            file("apps/chrome/src/index.ts", CHROME_TS),
            file("apps/chrome/package.json", CHROME_JSON),
        ],
    }
}

/// The tree with `text` appended to the file `path`, or replacing `from` with `to` in it.
fn append(tree: &mut Tree, path: &str, text: &str) {
    let f = tree
        .component
        .iter_mut()
        .chain(tree.hosted.iter_mut())
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("no file {path}"));
    f.text.push_str(text);
}

fn edit(tree: &mut Tree, path: &str, from: &str, to: &str) {
    let f = tree
        .component
        .iter_mut()
        .chain(tree.hosted.iter_mut())
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("no file {path}"));
    assert!(f.text.contains(from), "{path} has no {from:?}");
    f.text = f.text.replace(from, to);
}

#[test]
fn a_good_tree_passes() {
    let failures = check_tree(&tree());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Break 1.
#[test]
fn the_hosted_app_importing_the_components_internals_fails() {
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/src/main.ts",
        concat!(
            "import { App } from \"../../../",
            web_root!(),
            "/src/wasm/app.js\";\n"
        ),
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert_eq!(
        failures[0],
        concat!(
            "apps/hosted/src/main.ts:4 imports ../../../",
            web_root!(),
            "/src/wasm/app.js; apps/hosted \
         imports only gantt-core"
        )
    );
}

/// Break 2.
#[test]
fn the_component_importing_the_hosted_app_fails() {
    let mut t = tree();
    append(
        &mut t,
        concat!(web_root!(), "/src/main.ts"),
        "import \"../../apps/hosted/src/samples\";\n",
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:3 names apps/hosted; the component knows nothing"
        ),
    );
}

/// Break 3.
#[test]
fn the_component_naming_a_hosting_url_fails() {
    let mut t = tree();
    append(
        &mut t,
        concat!(web_root!(), "/src/open.ts"),
        "const u = \"https://gantt-app.pages.dev\";\n",
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(
        &failures,
        concat!(web_root!(), "/src/open.ts:2 names pages.dev;"),
    );
    one(
        &failures,
        concat!(web_root!(), "/src/open.ts:2 names gantt-app;"),
    );
}

/// Break 4: an alias for the chart sends the app to the source, not the package it declares.
#[test]
fn an_alias_for_the_chart_fails() {
    for name in ["@gantt/chart", "gantt-core"] {
        let mut t = tree();
        edit(
            &mut t,
            "apps/hosted/vite.config.ts",
            "base: \"./\",",
            &format!("resolve: {{ alias: {{ \"{name}\": \"../x\" }} }},"),
        );
        let failures = check_tree(&t);
        assert_eq!(failures.len(), 1, "{failures:#?}");
        one(
            &failures,
            &format!("apps/hosted/vite.config.ts:4 names {name};"),
        );
        let mut t = tree();
        edit(
            &mut t,
            "apps/hosted/tsconfig.json",
            "\"strict\": true",
            &format!("\"paths\": {{ '{name}': [\"x\"] }}"),
        );
        let failures = check_tree(&t);
        assert_eq!(failures.len(), 1, "{failures:#?}");
        one(
            &failures,
            &format!("apps/hosted/tsconfig.json:3 names {name};"),
        );
    }
    let mut t = tree();
    t.hosted.retain(|f| f.path != "apps/hosted/tsconfig.json");
    one(&check_tree(&t), "apps/hosted/tsconfig.json: not found");
}

/// Break 4b: an app that does not declare the package has nothing for its imports to find.
#[test]
fn a_manifest_that_does_not_declare_the_package_fails() {
    for (path, from) in [
        (
            "apps/hosted/package.json",
            "\"gantt-core\": \"link:../../packages/gantt\", ",
        ),
        (
            "apps/chrome/package.json",
            "\"gantt-core\": \"link:../../packages/gantt\"",
        ),
    ] {
        let mut t = tree();
        edit(&mut t, path, from, "");
        let failures = check_tree(&t);
        assert_eq!(failures.len(), 1, "{failures:#?}");
        one(&failures, &format!("{path} does not declare gantt-core;"));
    }
}

/// The chrome loads the chart as the hosted app does, and reads nothing else from outside its folder.
#[test]
fn the_chrome_imports_only_the_package() {
    for (spec, why) in [
        (
            "@gantt/chart",
            "a package apps/chrome/package.json does not declare",
        ),
        (
            "gantt-core/src/index",
            "apps/chrome imports only gantt-core",
        ),
        (
            "../../hosted/src/main",
            "apps/chrome imports only gantt-core",
        ),
        (
            "left-pad",
            "a package apps/chrome/package.json does not declare",
        ),
    ] {
        let mut t = tree();
        append(
            &mut t,
            "apps/chrome/src/index.ts",
            &format!("import \"{spec}\";\n"),
        );
        let failures = check_tree(&t);
        assert_eq!(failures.len(), 1, "{spec}: {failures:#?}");
        one(
            &failures,
            &format!("apps/chrome/src/index.ts:3 imports {spec}"),
        );
        one(&failures, why);
    }
    let mut t = tree();
    t.hosted.retain(|f| !f.path.starts_with("apps/chrome/src/"));
    one(&check_tree(&t), "apps/chrome/src: found 0 TypeScript files");
}

/// Break 5.
#[test]
fn an_empty_hosted_source_directory_fails_and_does_not_pass() {
    let mut t = tree();
    t.hosted.retain(|f| !f.path.starts_with("apps/hosted/src/"));
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "apps/hosted/src: found 0 TypeScript files; the scan reads nothing",
    );
}

/// Break 6.
#[test]
fn a_side_effect_import_of_the_component_fails() {
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/src/main.ts",
        concat!("import \"../../../", web_root!(), "/src/open.js\";\n"),
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        concat!(
            "apps/hosted/src/main.ts:4 imports ../../../",
            web_root!(),
            "/src/open.js; apps/hosted imports only gantt-core"
        ),
    );
}

/// Break 7: the hosted file over 400 lines fails the size budget through the new `kind()`, and
/// the same file outside the hosted roots is not measured, so the classification is what fails it.
#[test]
fn an_oversized_hosted_file_fails_the_size_budget() {
    let others = [
        ("crates/app/src/a.rs", "fn a() {}\n"),
        ("crates/app/tests/t.rs", "fn t() {}\n"),
        (concat!(web_root!(), "/src/a.ts"), "const a = 1;\n"),
    ];
    let with = |path: &str, lines: usize| {
        let mut files: Vec<(String, String)> = others
            .iter()
            .map(|(p, t)| (p.to_string(), t.to_string()))
            .collect();
        files.push((path.to_string(), "x\n".repeat(lines)));
        budget_failures(&files)
    };
    let failures = with("apps/hosted/src/samples.ts", 401);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "apps/hosted/src/samples.ts: file is 401 lines, over the 400-line limit",
    );
    assert_eq!(with("apps/hosted/src/samples.ts", 400), [] as [String; 0]);
    assert_eq!(
        with("apps/hosted/tests/long.spec.ts", 600),
        [] as [String; 0]
    );
    one(
        &with("apps/hosted/tests/long.spec.ts", 601),
        "apps/hosted/tests/long.spec.ts: file is 601 lines, over the 600-line limit",
    );
    assert_eq!(with("apps/hosted/other/samples.ts", 401), [] as [String; 0]);
}

#[test]
fn a_hosted_selector_that_starts_with_gantt_fails() {
    let mut t = tree();
    edit(
        &mut t,
        "apps/hosted/index.html",
        "  .host-bar { height: 32px; }",
        "  .gantt .row { x: 1 }\n  .host, :where(.gantt-bar) { x: 2 }\n  :is( .gantt ) a { x: 3 }",
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 3, "{failures:#?}");
    one(
        &failures,
        "apps/hosted/index.html:7 has the selector `.gantt .row`;",
    );
    one(
        &failures,
        "apps/hosted/index.html:8 has the selector `:where(.gantt-bar)`;",
    );
    one(
        &failures,
        "apps/hosted/index.html:9 has the selector `:is( .gantt ) a`;",
    );
    let mut t = tree();
    t.hosted
        .push(file("apps/hosted/src/host.css", ".a {}\n.gantt:focus {}\n"));
    one(
        &check_tree(&t),
        "apps/hosted/src/host.css:2 has the selector `.gantt:focus`;",
    );
}

#[test]
fn the_web_sources_rule_holds_in_the_hosted_app() {
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/src/samples.ts",
        "const r = app.test_head_row();\nwindow.gantt = 1;\nlet h: GanttHook;\nconst a = gantt.app;\n// app.test_ok()\n",
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 4, "{failures:#?}");
    one(
        &failures,
        "apps/hosted/src/samples.ts:2 uses test_head_row;",
    );
    one(
        &failures,
        concat!(
            "apps/hosted/src/samples.ts:3 uses window.gantt; only ",
            web_root!(),
            "/src/test-hook.ts may"
        ),
    );
    one(&failures, "apps/hosted/src/samples.ts:4 uses GanttHook;");
    one(&failures, "apps/hosted/src/samples.ts:5 uses gantt.app;");
}

#[test]
fn a_manifest_that_is_not_json_fails() {
    let mut t = tree();
    edit(&mut t, "apps/hosted/package.json", "{\n", "{ nope\n");
    one(
        &check_tree(&t),
        "apps/hosted/package.json is not valid JSON",
    );
}

mod cloud;
mod component;
mod imports;
