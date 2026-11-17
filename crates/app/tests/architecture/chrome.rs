//! Check 14: the chart does not import the app's chrome (gs#1087, decision 0012). The toolbar, the filters menu,
//! the find bar, the status line, Open, Export, the keys list, the import notes, the tab and the unsaved mark are an app that drives the chart through its public data API, commands and
//! events; they live in `apps/chrome/`. A component file (`packages/gantt/src/**` but the generated `packages/gantt/src/wasm/` and the
//! developer page) whose relative import, re-export or dynamic `import()` names one of `CHROME_MODULES` fails, from
//! whatever directory, so moving a file does not hide the import.
//!
//! The imports main had when the check began are in `EXEMPT`, one `path | import` line each, sorted bytewise.
//! The list only shrinks: an import that is not listed fails, a listed import that is gone fails until its line is
//! deleted, and an unsorted, repeated or malformed line fails.

use crate::architecture::support::*;

/// The chrome modules a component file may not import, by file name without its extension: the toolbar, the Filters menu, the find
/// box and the status line (gs#1087), then Open, Export, the keys list, the import notes, the tab and the unsaved mark (gs#1088).
pub(crate) const CHROME_MODULES: &[&str] = &[
    "toolbar",
    "filters-menu",
    "find",
    "status-line",
    "open",
    "export",
    "keys-help",
    "unsupported",
    "tab",
    "save-mark",
];

/// The developer page: it mounts the chart and the chrome beside it, as a host page does, so it is not a component file.
pub(crate) const PAGE_FILES: &[&str] = &[web!("src/main.ts")];

/// The chrome imports the chart had when check 14 began, as `path | import`, sorted bytewise. Only shrinks; empty since the toolbar moved (gs#1087).
pub(crate) const EXEMPT: &[&str] = &[];

const LIST: &str = "EXEMPT in crates/app/tests/architecture/chrome.rs";

/// Every relative module specifier in `src` (comments blanked) with its byte offset, from the real import forms only:
/// `from "x"`, `import "x"`, `import("x")` and `require("x")`. The quote may be `"`, `'` or a backtick with no `${` in it.
/// A call such as `Array.from('./find.js')` is not an import.
/// Only a backtick specifier is dropped for `${`; in `"` or `'` quotes it is literal text.
fn relative_specifiers(src: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    for (keyword, call) in [("from", false), ("import", true), ("require", true)] {
        let mut from = 0;
        while let Some(at) = src[from..].find(keyword) {
            let start = from + at;
            let end = start + keyword.len();
            from = end;
            let before = &src[..start];
            let member = before.ends_with('.')
                && !before.ends_with("...")
                && !(keyword == "require" && before.ends_with("module."));
            if member || before.chars().next_back().is_some_and(is_ident) {
                continue;
            }
            let mut rest = src[end..].trim_start();
            if call {
                let inner = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '(');
                if inner.len() == rest.len() && keyword == "require" {
                    continue;
                }
                rest = inner;
            }
            let Some(quote) = rest
                .chars()
                .next()
                .filter(|c| matches!(c, '"' | '\'' | '`'))
            else {
                continue;
            };
            let Some(close) = rest[1..].find(quote) else {
                continue;
            };
            let spec = &rest[1..=close];
            if (spec.starts_with("./") || spec.starts_with("../"))
                && !(quote == '`' && spec.contains("${"))
            {
                out.push((src.len() - rest.len(), spec));
            }
        }
    }
    out.sort_unstable();
    out
}

/// The file name of `spec` without its query and its extension: `../x/find.js?raw` gives `find`.
fn module_name(spec: &str) -> &str {
    let path = spec.split(['?', '#']).next().unwrap_or(spec);
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, ext))
            if ["js", "ts", "mjs", "mts", "cjs", "cts", "tsx", "jsx"].contains(&ext) =>
        {
            stem
        }
        _ => name,
    }
}

/// The list's own failures: a line that is not `path | import`, and a line not after the one above it (unsorted or repeated).
fn check_list(exempt: &[&str]) -> Vec<String> {
    let mut failures = Vec::new();
    for (i, entry) in exempt.iter().enumerate() {
        let well_formed = entry.split_once(" | ").is_some_and(|(path, spec)| {
            !path.is_empty() && !spec.is_empty() && !spec.contains(" | ")
        });
        if !well_formed {
            failures.push(format!("{LIST}: {entry:?} is not one `path | import` line"));
        }
        if i > 0 && exempt[i - 1].as_bytes() >= entry.as_bytes() {
            failures.push(format!(
                "{LIST}: {entry:?} is not after {:?}; the list is sorted bytewise with no repeats",
                exempt[i - 1]
            ));
        }
    }
    failures
}

pub(crate) fn check_chrome(files: &[SourceFile], exempt: &[&str]) -> Vec<String> {
    let component: Vec<_> = files
        .iter()
        .filter(|f| !PAGE_FILES.contains(&f.path.as_str()))
        .collect();
    if component.is_empty() {
        return vec![
            web!("src: found 0 component files; the chrome check reads nothing, so it would pass on an empty parse")
                .to_string(),
        ];
    }
    let mut failures = check_list(exempt);
    let mut used = vec![false; exempt.len()];
    for file in component {
        let text = strip_ts_comments(&file.text);
        for (offset, spec) in relative_specifiers(&text) {
            let name = module_name(spec);
            if !CHROME_MODULES.contains(&name) {
                continue;
            }
            if let Some(at) = exempt
                .iter()
                .position(|e| *e == format!("{} | {spec}", file.path))
            {
                used[at] = true;
            } else {
                failures.push(format!(
                    "{}:{} imports {spec}; the chart may not import the app's chrome ({}), which lives in apps/chrome \
                     and uses only the chart's public API (decision 0012)",
                    file.path,
                    line_of(&text, offset),
                    CHROME_MODULES.join(", ")
                ));
            }
        }
    }
    for (entry, used) in exempt.iter().zip(used) {
        if !used {
            failures.push(format!(
                "{LIST} lists {entry:?}, which is no longer imported: delete its line (the list only shrinks)"
            ));
        }
    }
    failures
}

#[test]
fn the_chart_does_not_import_the_chrome() {
    let failures = check_chrome(&web_sources(), EXEMPT);
    assert!(
        failures.is_empty(),
        "the chart imports the app's chrome ({} imports; check 14 in main.rs):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn a_component_file_importing_the_chrome_fails() {
    let files = [
        file(
            web!("src/index.ts"),
            "import { installToolbar } from \"./toolbar.js\";\nimport { installFind } from './find.js';\n",
        ),
        file(
            web!("src/keys.ts"),
            "\n\nexport { type FiltersMenu } from \"../../apps/chrome/filters-menu.js\";\n",
        ),
        file(
            web!("src/loop.ts"),
            "import {\n  statusText,\n} from \"./status-line.ts\";\n",
        ),
        file(
            web!("src/lazy.ts"),
            "const m = await import( \"./toolbar.js\");\nconst n = await import(('./toolbar.js'));\nconst r = require(( './toolbar.js' ));\nconst copy = {...import('./toolbar.js')};\nconst w = module.require('./toolbar.js');\n",
        ),
    ];
    let failures = check_chrome(&files, &[]);
    assert_eq!(failures.len(), 9, "{failures:#?}");
    one(&failures, web!("src/index.ts:1 imports ./toolbar.js;"));
    one(&failures, web!("src/index.ts:2 imports ./find.js;"));
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/keys.ts:3 imports ../../apps/chrome/filters-menu.js;"
        ),
    );
    one(&failures, web!("src/loop.ts:3 imports ./status-line.ts;"));
    one(&failures, web!("src/lazy.ts:1 imports ./toolbar.js;"));
    one(&failures, web!("src/lazy.ts:2 imports ./toolbar.js;"));
    one(&failures, web!("src/lazy.ts:3 imports ./toolbar.js;"));
    one(&failures, web!("src/lazy.ts:4 imports ./toolbar.js;"));
    one(&failures, web!("src/lazy.ts:5 imports ./toolbar.js;"));
}

#[test]
fn the_file_chrome_modules_fail_too() {
    let text: String = [
        "open",
        "export",
        "keys-help",
        "unsupported",
        "tab",
        "save-mark",
    ]
    .iter()
    .map(|m| format!("import {{ x }} from \"./{m}.js\";\n"))
    .collect::<Vec<_>>()
    .concat();
    let failures = check_chrome(&[file(web!("src/index.ts"), &text)], &[]);
    assert_eq!(failures.len(), 6, "{failures:#?}");
    one(&failures, web!("src/index.ts:1 imports ./open.js;"));
    one(&failures, web!("src/index.ts:6 imports ./save-mark.js;"));
}

#[test]
fn the_page_comments_and_other_names_pass() {
    let files = [
        file(
            web!("src/main.ts"),
            "import { installToolbar } from \"../../apps/chrome/toolbar.js\";\n",
        ),
        file(
            web!("src/a.ts"),
            "// import { x } from \"./toolbar.js\";\n/* from './find.js' */\nimport { y } from \"./finder.js\";\n\
             import z from \"lodash/find\";\nconst s = \"from ./find.js\";\nimport chart from \"./chart.html?raw\";\n\
             const found = list.find((x) => x);\nimport { reimport } from \"./status-line-old.js\";\n",
        ),
    ];
    assert_eq!(check_chrome(&files, &[]), Vec::<String>::new());
}

#[test]
fn a_template_literal_import_fails() {
    let files = [file(
        web!("src/a.ts"),
        "const m = await import(`./toolbar.js`);\nimport x from `./find.js`;\nimport(`./${dir}/toolbar.js`);\n\
         import './toolbar.js?label=${name}';\n",
    )];
    let failures = check_chrome(&files, &[]);
    assert_eq!(failures.len(), 3, "{failures:#?}");
    one(&failures, web!("src/a.ts:1 imports ./toolbar.js;"));
    one(&failures, web!("src/a.ts:2 imports ./find.js;"));
    one(
        &failures,
        web!("src/a.ts:4 imports ./toolbar.js?label=${name};"),
    );
}

#[test]
fn a_call_that_is_not_an_import_passes_and_require_fails() {
    let files = [
        file(
            web!("src/a.ts"),
            "const x = Array.from('./find.js');\nconst z = obj.import('./toolbar.js');\nconst y = Array.from ( \"./toolbar.js\" );\n",
        ),
        file(
            web!("src/b.cjs"),
            "const t = require('./toolbar.js');\nconst u = require('./toolbar.cjs');\nconst v = require('./toolbar.cts');\n",
        ),
    ];
    let failures = check_chrome(&files, &[]);
    assert_eq!(failures.len(), 3, "{failures:#?}");
    one(&failures, web!("src/b.cjs:1 imports ./toolbar.js;"));
    one(&failures, web!("src/b.cjs:2 imports ./toolbar.cjs;"));
    one(&failures, web!("src/b.cjs:3 imports ./toolbar.cts;"));
}

#[test]
fn every_script_extension_is_scanned() {
    let dir = std::env::temp_dir().join(format!("tl1212-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for ext in ["cts", "cjs", "jsx"] {
        std::fs::write(dir.join(format!("a.{ext}")), "import './toolbar.js';\n").unwrap();
    }
    let mut files = Vec::new();
    collect(dir.to_str().unwrap(), WEB_SCRIPT_EXTENSIONS, "", &mut files);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(check_chrome(&files, &[]).len(), 3);
}

#[test]
fn an_empty_parse_fails() {
    one(&check_chrome(&[], &[]), "found 0 component files");
    one(
        &check_chrome(&[file(web!("src/main.ts"), "")], &[]),
        "found 0 component files",
    );
}

#[test]
fn a_listed_import_passes_and_the_list_only_shrinks() {
    let files = [
        file(
            web!("src/index.ts"),
            "import { a } from \"./find.js\";\nimport { b } from \"./toolbar.js\";\n",
        ),
        file(
            web!("src/keys.ts"),
            "\n\nimport { c } from \"./find.js\";\n",
        ),
    ];
    let exempt = [
        web!("src/index.ts | ./find.js"),
        web!("src/index.ts | ./toolbar.js"),
        web!("src/keys.ts | ./find.js"),
    ];
    assert_eq!(check_chrome(&files, &exempt), Vec::<String>::new());
    // An import that is not listed fails, with its line.
    let failures = check_chrome(&files, &exempt[..2]);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, web!("src/keys.ts:3 imports ./find.js;"));
    // A listed import that is gone fails until its line is deleted.
    let failures = check_chrome(&files[..1], &exempt);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        concat!(
            "lists \"",
            web_root!(),
            "/src/keys.ts | ./find.js\", which is no longer imported"
        ),
    );
}

#[test]
fn an_unsorted_repeated_or_malformed_list_fails() {
    let files = [file(web!("src/a.ts"), "const a = 1;\n")];
    let failures = check_chrome(
        &files,
        &[web!("src/b.ts | ./find.js"), web!("src/a.ts | ./find.js")],
    );
    one(
        &failures,
        concat!(
            "\"",
            web_root!(),
            "/src/a.ts | ./find.js\" is not after \"",
            web_root!(),
            "/src/b.ts | ./find.js\""
        ),
    );
    let failures = check_chrome(
        &files,
        &[web!("src/a.ts | ./find.js"), web!("src/a.ts | ./find.js")],
    );
    one(&failures, "is not after");
    for bad in [
        web!("src/a.ts ./find.js"),
        " | ./find.js",
        web!("src/a.ts | "),
        "a | b | c",
    ] {
        one(
            &check_chrome(&files, &[bad]),
            "is not one `path | import` line",
        );
    }
}
