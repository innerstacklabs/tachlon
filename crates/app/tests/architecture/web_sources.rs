//! Check 5 and the TypeScript side of check 6: where `test_*` names and the test hook may appear.

use std::collections::BTreeSet;

use crate::architecture::support::*;
use crate::architecture::web_root::WEB_ROOT;

pub(crate) const HOOK_FILE: &str = web!("src/test-hook.ts");

pub(crate) fn check_web_sources(
    files: &[SourceFile],
    side_files: &[(String, usize)],
) -> Vec<String> {
    let mut failures = Vec::new();
    if files.is_empty() {
        failures.push(format!(
            "{WEB_ROOT}/src: found 0 TypeScript files; the scan reads nothing, \
                 so this test would pass on an empty parse"
        ));
    }
    for (path, line) in side_files {
        if !files.iter().any(|f| &f.path == path) {
            failures.push(format!(
                "{PAGE}:{line} lists the Test-side file {path} but it does not exist"
            ));
        }
    }
    for file in files {
        let is_side = side_files.iter().any(|(p, _)| *p == file.path);
        let is_hook = file.path == HOOK_FILE;
        for (i, line) in strip_ts_comments(&file.text).lines().enumerate() {
            let n = i + 1;
            if !is_side {
                let mut seen = BTreeSet::new();
                for id in identifiers(line) {
                    if id.starts_with("test_") && seen.insert(id) {
                        failures.push(format!(
                            "{}:{n} uses {id}; test-only exports may be used only in the \
                             Test-side files of {PAGE}",
                            file.path
                        ));
                    }
                }
            }
            if !is_hook {
                for hook in ["window.gantt", "GanttHook", "gantt.app"] {
                    if find_bounded(line, hook).is_some() {
                        failures.push(format!(
                            "{}:{n} uses {hook}; only {HOOK_FILE} may",
                            file.path
                        ));
                    }
                }
            }
        }
    }
    failures
}

#[test]
fn a_test_name_outside_the_test_side_files_fails() {
    let side = [(web!("src/measure.ts").to_string(), 3)];
    let files = [
        file(web!("src/measure.ts"), "const row = app.test_head_row();\n"),
        file(
            web!("src/main.ts"),
            "\nconst x = 1;\nconst y = app.test_calendar_add(1, 2, 3);\n",
        ),
    ];
    let failures = check_web_sources(&files, &side);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert_eq!(
        failures[0],
        web!(
            "src/main.ts:3 uses test_calendar_add; test-only exports may be used only in the \
         Test-side files of docs/WASM_API.md"
        )
    );
}

#[test]
fn a_destructured_test_name_fails_but_a_comment_does_not() {
    let files = [
        file(web!("src/a.ts"), "const { test_x } = app;\n"),
        file(
            web!("src/b.ts"),
            "// app.test_y() is test-only\n/* test_z */ const ok = 1;\nconst s = \"http://x\"; // test_w\n",
        ),
        file(web!("src/c.ts"), "const s = \"http://x\"; app.test_v();\n"),
        file(
            web!("src/d.ts"),
            "const latest_x = 1; const contest_y = 2;\n",
        ),
    ];
    let failures = check_web_sources(&files, &[]);
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(&failures, web!("src/a.ts:1 uses test_x;"));
    one(&failures, web!("src/c.ts:1 uses test_v;"));
}

#[test]
fn the_hook_outside_test_hook_fails() {
    let files = [
        file(web!("src/foo.ts"), "\n\n\n\nwindow.gantt = {};\n"),
        file(
            web!("src/bar.ts"),
            "let h: GanttHook;\nconst a = gantt.app;\n",
        ),
        file(
            web!("src/test-hook.ts"),
            "window.gantt = {};\nlet h: GanttHook;\nconst a = gantt.app;\n",
        ),
        file(
            web!("src/ok.ts"),
            "const ganttHook = 1; const gantt_app = 2; // window.gantt\n",
        ),
    ];
    let failures = check_web_sources(&files, &[]);
    assert_eq!(failures.len(), 3, "{failures:#?}");
    one(
        &failures,
        &format!("{WEB_ROOT}/src/foo.ts:5 uses window.gantt; only {HOOK_FILE} may"),
    );
    one(
        &failures,
        &format!("{WEB_ROOT}/src/bar.ts:1 uses GanttHook; only {HOOK_FILE} may"),
    );
    one(
        &failures,
        &format!("{WEB_ROOT}/src/bar.ts:2 uses gantt.app; only {HOOK_FILE} may"),
    );
}

#[test]
fn a_listed_test_side_file_that_does_not_exist_fails() {
    let side = [(web!("src/gone.ts").to_string(), 7)];
    let failures = check_web_sources(&[file(web!("src/a.ts"), "const a = 1;\n")], &side);
    one(
        &failures,
        &format!(
            "docs/WASM_API.md:7 lists the Test-side file {WEB_ROOT}/src/gone.ts but it does not exist"
        ),
    );
}
