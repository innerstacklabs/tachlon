//! Self-tests of the size budget: one failing case per message.

use super::*;

// ---------------------------------------------------------------------------
// The check fires: one synthetic failing case per message

use crate::architecture::support::one;

fn lines(n: usize) -> String {
    "x\n".repeat(n)
}

/// A Rust file with `code` lines above a test module of `tests` lines (none when 0).
fn rust_file(code: usize, tests: usize) -> String {
    let mut text = lines(code);
    if tests > 0 {
        text.push_str("#[cfg(test)]\nmod tests {\n");
        text.push_str(&lines(tests - 3));
        text.push_str("}\n");
    }
    text
}

fn measured(path: &str, part: Part, lines: usize, limit: usize) -> Measure {
    Measure {
        path: path.to_string(),
        part,
        lines,
        limit,
    }
}

const SRC: &str = "crates/a/src/x.rs";

#[test]
fn a_file_may_hold_exactly_its_limit() {
    for (path, text) in [
        (SRC, rust_file(400, 0)),
        (web!("src/a.ts"), lines(400)),
        ("crates/a/tests/t.rs", lines(600)),
    ] {
        let failures = check_budget(&measure(path, &text), &[]);
        assert!(failures.is_empty(), "{path}: {failures:#?}");
    }
    let failures = check_budget(&measure(SRC, &rust_file(401, 0)), &[]);
    one(
        &failures,
        "crates/a/src/x.rs: code above its test module is 401 lines, over the 400-line limit",
    );
}

#[test]
fn a_new_file_over_its_limit_gets_no_allowance() {
    let listed = [("crates/a/src/other.rs", Code, 900)];
    for (path, text, limit) in [
        (web!("src/new.ts"), lines(401), 400),
        ("crates/a/tests/new.rs", lines(601), 600),
        ("demos/new.mjs", lines(401), 400),
    ] {
        let failures = check_budget(&measure(path, &text), &[]);
        one(
            &failures,
            &format!(
                "{path}: file is {} lines, over the {limit}-line limit",
                text.len() / 2
            ),
        );
        one(&failures, "gets no allowance");
        let failures = check_budget(&measure(path, &text), &listed);
        assert_eq!(failures.len(), 2, "{failures:#?}");
    }
}

#[test]
fn an_allowance_is_for_one_part_of_one_file() {
    let listed = [(SRC, Code, 500)];
    let failures = check_budget(&measure(SRC, &rust_file(500, 301)), &listed);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "crates/a/src/x.rs: in-file test module is 301 lines, over the 300-line limit",
    );
    let failures = check_budget(&measure("crates/a/src/y.rs", &rust_file(500, 0)), &listed);
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(
        &failures,
        "crates/a/src/y.rs: code above its test module is 500 lines",
    );
    one(
        &failures,
        "crates/a/src/x.rs: EXEMPT lists its code above its test module",
    );
}

#[test]
fn an_exempt_file_at_its_listed_size_passes() {
    let listed = [(SRC, Code, 500), (SRC, TestModule, 400)];
    let failures = check_budget(&measure(SRC, &rust_file(500, 400)), &listed);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn an_exempt_file_that_grows_fails() {
    let listed = [(SRC, Code, 500)];
    let failures = check_budget(&measure(SRC, &rust_file(501, 0)), &listed);
    one(
        &failures,
        "crates/a/src/x.rs: code above its test module grew from 500 to 501 lines",
    );
    one(&failures, "it may not grow: split it");
    let listed = [(web!("src/a.ts"), File, 900)];
    let failures = check_budget(&measure(web!("src/a.ts"), &lines(901)), &listed);
    one(&failures, web!("src/a.ts: file grew from 900 to 901 lines"));
}

#[test]
fn an_exempt_file_that_shrinks_must_lower_its_row() {
    let listed = [(SRC, Code, 500)];
    let failures = check_budget(&measure(SRC, &rust_file(499, 0)), &listed);
    one(
        &failures,
        "crates/a/src/x.rs: code above its test module shrank from 500 to 499 lines",
    );
    one(
        &failures,
        "row in EXEMPT in crates/app/tests/architecture/size.rs to 499",
    );
}

#[test]
fn an_exempt_file_back_under_its_limit_must_leave_the_list() {
    let listed = [(SRC, Code, 500)];
    let failures = check_budget(&measure(SRC, &rust_file(400, 0)), &listed);
    one(
        &failures,
        "crates/a/src/x.rs: code above its test module is 400 lines, within the 400-line limit. \
         Delete its row",
    );
}

#[test]
fn a_row_for_a_file_or_part_that_is_gone_fails() {
    let listed = [("crates/a/src/gone.rs", Code, 500), (SRC, TestModule, 400)];
    let failures = check_budget(&measure(SRC, &rust_file(10, 0)), &listed);
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(
        &failures,
        "crates/a/src/gone.rs: EXEMPT lists its code above its test module, but there is no \
         such file or part any more",
    );
    one(
        &failures,
        "crates/a/src/x.rs: EXEMPT lists its in-file test module, but there is no such file",
    );
}

#[test]
fn a_row_listed_twice_fails() {
    let listed = [(SRC, Code, 500), (SRC, Code, 500)];
    let failures = check_budget(&measure(SRC, &rust_file(500, 0)), &listed);
    one(
        &failures,
        "crates/a/src/x.rs: EXEMPT lists its code above its test module twice",
    );
}

#[test]
fn the_exempt_list_is_sorted_and_has_no_duplicates() {
    for pair in EXEMPT.windows(2) {
        let (a, b) = (
            (pair[0].0, pair[0].1 as usize),
            (pair[1].0, pair[1].1 as usize),
        );
        assert!(
            a < b,
            "EXEMPT must be sorted by path, then Code before TestModule: {a:?} {b:?}"
        );
    }
}

#[test]
fn a_count_is_wc_l() {
    assert_eq!(wc_l(""), 0);
    assert_eq!(wc_l("a\nb"), 1);
    assert_eq!(wc_l("a\nb\n"), 2);
    // 400 newlines and a last line with none: `wc -l` says 400.
    let unterminated = format!("{}x", lines(400));
    let failures = check_budget(&measure(web!("src/a.ts"), &unterminated), &[]);
    assert!(failures.is_empty(), "{failures:#?}");
    let failures = check_budget(&measure(web!("src/a.ts"), &lines(401)), &[]);
    assert_eq!(failures.len(), 1, "{failures:#?}");
}

#[test]
fn the_test_module_is_counted_apart_from_the_code() {
    assert_eq!(
        measure(SRC, &rust_file(399, 300)),
        [
            measured(SRC, Code, 399, RUST_SOURCE_LIMIT),
            measured(SRC, TestModule, 300, TEST_MODULE_LIMIT)
        ]
    );
    assert_eq!(
        measure(SRC, &rust_file(400, 0)),
        [measured(SRC, Code, 400, RUST_SOURCE_LIMIT)]
    );
    let failures = check_budget(&measure(SRC, &rust_file(399, 301)), &[]);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "in-file test module is 301 lines, over the 300-line limit",
    );
}

#[test]
fn the_test_module_starts_at_its_cfg_attribute() {
    let one_line = "a\nb\n#[cfg(test)] mod tests {\n}\n";
    assert_eq!(split_code_test(one_line).0, ["a", "b"]);
    let split = "a\n#[cfg(test)]\n#[allow(dead_code)]\n\nmod tests {\n}\n";
    assert_eq!(split_code_test(split).0, ["a"]);
    assert_eq!(split_code_test(split).1.len(), 5);
    // A `cfg(test)` on another item, or a `mod tests` without it, starts nothing.
    let other = "a\n#[cfg(test)]\nfn helper() {}\nmod tests {\n}\n";
    assert_eq!(split_code_test(other).1.len(), 0);
}

#[test]
fn an_indented_test_module_inside_another_module_does_not_split_the_file() {
    let nested = "a\nmod inner {\n    #[cfg(test)]\n    mod tests {\n    }\n}\nb\n";
    assert_eq!(split_code_test(nested).1.len(), 0);
    let nested_one_line = "a\nmod inner {\n    #[cfg(test)] mod tests {\n    }\n}\nb\n";
    assert_eq!(split_code_test(nested_one_line).1.len(), 0);
    // The real, top-level module after it is still the boundary.
    let both = format!("{nested}#[cfg(test)]\nmod tests {{\n}}\n");
    assert_eq!(split_code_test(&both).0.len(), 7);
    // Production code after a nested module stays in `Code`.
    let text = format!("{}{nested}{}", lines(200), lines(250));
    let measures = measure(SRC, &text);
    assert_eq!(measures, [measured(SRC, Code, 457, RUST_SOURCE_LIMIT)]);
    one(
        &check_budget(&measures, &[]),
        "code above its test module is 457 lines",
    );
}

#[test]
fn a_module_whose_name_starts_with_tests_does_not_split_the_file() {
    let foo = "a\n#[cfg(test)]\nmod tests_foo {\n}\n";
    assert_eq!(split_code_test(foo).1.len(), 0);
    let foo_one_line = "a\n#[cfg(test)] mod tests_foo {\n}\n";
    assert_eq!(split_code_test(foo_one_line).1.len(), 0);
    // `mod tests;` and `mod tests{` are the test module.
    assert_eq!(split_code_test("a\n#[cfg(test)]\nmod tests;\n").1.len(), 2);
    assert_eq!(
        split_code_test("a\n#[cfg(test)]\nmod tests{\n}\n").1.len(),
        3
    );
}

#[test]
fn each_path_falls_under_one_kind() {
    use Kind::{RustSource, TestFile, WebSource};
    for (path, want) in [
        ("crates/app/src/lib.rs", Some(RustSource)),
        ("crates/engine/benches/cpm.rs", Some(RustSource)),
        ("crates/xer/examples/synth.rs", Some(RustSource)),
        ("crates/app/tests/drag.rs", Some(TestFile)),
        (
            "crates/app/tests/architecture/exports/mod.rs",
            Some(TestFile),
        ),
        ("crates/engine/tests/common/mod.rs", Some(TestFile)),
        ("crates/app/src/web/tests.rs", Some(TestFile)),
        ("crates/import/tests/fixtures/links.xml", None),
        (web!("src/main.ts"), Some(WebSource)),
        (web!("index.html"), Some(WebSource)),
        ("demos/50k/measure.mjs", Some(WebSource)),
        ("scripts/build-web.sh", None),
        ("demos/50k/results/measure.json", None),
        (web!("tests/open.spec.ts"), Some(TestFile)),
        (web!("tests/fixtures/one-project.xer"), None),
        (web!("scripts/verify.mjs"), Some(TestFile)),
        (web!("vite.config.ts"), None),
        ("apps/hosted/src/main.ts", Some(WebSource)),
        ("apps/hosted/src/samples.ts", Some(WebSource)),
        ("apps/hosted/index.html", Some(WebSource)),
        ("apps/hosted/samples/small.mjs", Some(WebSource)),
        ("apps/hosted/scripts/make-samples.mjs", Some(WebSource)),
        ("apps/hosted/vite.config.ts", Some(WebSource)),
        ("apps/hosted/playwright.config.ts", Some(WebSource)),
        ("apps/hosted/tests/hosted.spec.ts", Some(TestFile)),
        ("apps/hosted/tests/fixtures/two.xer", None),
        ("apps/hosted/package.json", None),
        ("apps/hosted/samples/small.xer", None),
        ("apps/hosted/dist/index.html", None),
        ("apps/hosted/playwright-report/index.html", None),
        ("apps/hosted/tests/vite.config.ts", Some(TestFile)),
    ] {
        assert_eq!(kind(path), want, "{path}");
    }
}

#[test]
fn a_tree_with_no_file_of_a_kind_fails() {
    let (_, failures) = measure_all(&[]);
    assert_eq!(failures.len(), 3, "{failures:#?}");
    one(&failures, "found 0 Rust source files");
    one(&failures, "found 0 TS/JS/HTML/CSS source files");
    one(&failures, "found 0 test files");
    let files = [
        (SRC.to_string(), lines(3)),
        (web!("src/a.ts").to_string(), lines(3)),
    ];
    let (_, failures) = measure_all(&files);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, "found 0 test files");
}
