//! Check 9: every style rule is under the chart's root class.

use crate::architecture::support::*;
use crate::architecture::web_root::WEB_ROOT;

// ---------------------------------------------------------------------------
// Check 9: every style rule is under the chart's root class

/// `html` with everything outside its `<style>` blocks blanked (newlines kept), so line numbers hold.
pub(crate) fn style_blocks(html: &str) -> String {
    fn blank(text: &str) -> String {
        text.chars()
            .map(|c| if c == '\n' { '\n' } else { ' ' })
            .collect()
    }
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(rel) = lower[pos..].find("<style") {
        let open = pos + rel;
        let Some(gt) = lower[open..].find('>') else {
            break;
        };
        let body = open + gt + 1;
        let end = lower[body..]
            .find("</style")
            .map_or(html.len(), |e| body + e);
        out.push_str(&blank(&html[pos..body]));
        out.push_str(&html[body..end]);
        pos = end;
    }
    out.push_str(&blank(&html[pos..]));
    out
}

/// The parts of a selector list, split on commas outside parentheses and brackets.
pub(crate) fn split_selectors(prelude: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in prelude.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&prelude[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&prelude[start..]);
    parts
}

/// A selector that starts at the chart's root class: `.gantt` (not `.gantt-x`), or `:where(...)`
/// whose every comma-separated item starts at the root class in the same way.
fn under_root_class(selector: &str) -> bool {
    let s = selector.trim();
    if let Some(after) = s.strip_prefix(":where(") {
        let mut depth = 1i32;
        let mut close = None;
        for (i, c) in after.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        return close.is_some_and(|end| {
            split_selectors(&after[..end])
                .into_iter()
                .all(under_root_class)
        });
    }
    s.strip_prefix(".gantt").is_some_and(|rest| {
        !rest
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '-' || c == '_')
    })
}

/// The selector list of every rule in `css` with its 1-based line, in order. `@media`, `@supports`,
/// `@container` and `@layer` blocks are read through; other at-rule blocks (`@keyframes`,
/// `@font-face`) hold no selectors of the page and are skipped.
pub(crate) fn rule_preludes(css: &str) -> Vec<(usize, String)> {
    printed_preludes(css)
        .into_iter()
        .map(|(line, prelude, _)| (line, prelude))
        .collect()
}

/// `rule_preludes` with a flag for a rule inside an `@media print` block.
fn printed_preludes(css: &str) -> Vec<(usize, String, bool)> {
    let text = strip_block_comments(css);
    let mut lists = Vec::new();
    // One entry per open brace: whether the block holds rules (false when it holds declarations,
    // a nested rule, which is relative to its parent, or is skipped) and whether it is in `@media print`.
    let mut stack: Vec<(bool, bool)> = Vec::new();
    let mut prelude_start = 0;
    for (i, c) in text.char_indices() {
        match c {
            '{' => {
                let raw = &text[prelude_start..i];
                let prelude = raw.trim();
                let (parent_rules, parent_print) = stack.last().copied().unwrap_or((true, false));
                let holds_rules = if parent_rules {
                    if let Some(at_rule) = prelude.strip_prefix('@') {
                        let name: String = at_rule
                            .chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                            .collect();
                        matches!(name.as_str(), "media" | "supports" | "container" | "layer")
                    } else {
                        let line =
                            line_of(&text, prelude_start + raw.len() - raw.trim_start().len());
                        lists.push((line, prelude.to_string(), parent_print));
                        false
                    }
                } else {
                    false
                };
                stack.push((holds_rules, parent_print || prelude == "@media print"));
                prelude_start = i + 1;
            }
            '}' => {
                stack.pop();
                prelude_start = i + 1;
            }
            ';' => prelude_start = i + 1,
            _ => {}
        }
    }
    lists
}

/// The number of selector lists in `css` and one failure for each selector outside the root class.
pub(crate) fn style_scope(path: &str, css: &str) -> (usize, Vec<String>) {
    let mut failures = Vec::new();
    let lists = printed_preludes(css);
    for (line, prelude, in_print) in &lists {
        for selector in split_selectors(prelude) {
            // The one exception: the developer page's print rule lifts the clip its `html` and `body` carry inline.
            let page_print_reset =
                path == INDEX_HTML && *in_print && matches!(selector.trim(), "html" | "body");
            if !under_root_class(selector) && !page_print_reset {
                failures.push(format!(
                    "{path}:{line} has the selector `{}`, which does not start \
                     with .gantt or :where(.gantt); a rule outside the chart's \
                     root class can restyle the page that embeds the chart \
                     (docs/ARCHITECTURE.md \"Embedding\")",
                    selector.trim()
                ));
            }
        }
    }
    (lists.len(), failures)
}

pub(crate) fn check_style_scope(css: &[SourceFile], html: &[SourceFile]) -> Vec<String> {
    // The page keeps one `<style>`, the `@media print` rule for `html` and `body`; the chart's markup keeps none (gs#204). Any other added later is read here.
    let mut failures = Vec::new();
    for file in html {
        failures.extend(style_scope(&file.path, &style_blocks(&file.text)).1);
    }
    let mut lists = 0;
    for file in css {
        let (found, scoped) = style_scope(&file.path, &file.text);
        lists += found;
        failures.extend(scoped);
    }
    if lists == 0 {
        failures.push(format!(
            "{WEB_ROOT}/src/**/*.css (whole files): found 0 selectors in the stylesheets; the scan reads \
             nothing, so this test would pass on an empty parse"
        ));
    }
    failures
}

fn scope(css: &str) -> Vec<String> {
    style_scope(web!("src/chart.css"), css).1
}

#[test]
fn a_selector_outside_the_root_class_fails() {
    let failures = scope(
        "html, body {\n  margin: 0;\n}\n\n.gantt .a, .b {\n  x: 1;\n}\n*, :root { x: 1 }\ndiv { x: 1 }\n",
    );
    assert_eq!(failures.len(), 6, "{failures:#?}");
    one(&failures, web!("src/chart.css:1 has the selector `html`"));
    one(&failures, web!("src/chart.css:1 has the selector `body`"));
    one(&failures, web!("src/chart.css:5 has the selector `.b`"));
    one(&failures, web!("src/chart.css:8 has the selector `*`"));
    one(&failures, web!("src/chart.css:8 has the selector `:root`"));
    one(&failures, web!("src/chart.css:9 has the selector `div`"));
}

#[test]
fn only_the_root_class_itself_starts_a_selector() {
    let ok = ".gantt {}\n.gantt:focus {}\n.gantt[data-dropping=\"true\"] {}\n.gantt #a > .b::before {}\n:where(.gantt) {}\n:where(.gantt) .x {}\n.gantt .a,\n.gantt .b {}\n.gantt#a {}\n";
    assert!(scope(ok).is_empty(), "{:#?}", scope(ok));
    let failures =
        scope(".gantt-x {}\n.ganttish .a {}\n.not .gantt {}\n#a .gantt {}\n:is(.gantt) {}\n");
    assert_eq!(failures.len(), 5, "{failures:#?}");
}

#[test]
fn every_item_inside_where_must_start_at_the_root_class() {
    let ok = ":where(.gantt) {}\n:where(.gantt .a) {}\n:where(.gantt > .a) {}\n:where(.gantt:focus) {}\n:where(.gantt, .gantt .b) {}\n:where( .gantt ) .x {}\n:where(.gantt :is(.a, .b)) {}\n";
    assert!(scope(ok).is_empty(), "{:#?}", scope(ok));
    let failures = scope(
        ":where(.gantt-x) {}\n:where(.gantt, body) {}\n:where(body, .gantt) {}\n:where() {}\n:where(.gantt {}\n:where(.not .gantt) {}\n:where(:is(.gantt)) {}\n",
    );
    assert_eq!(failures.len(), 7, "{failures:#?}");
    one(
        &failures,
        web!("src/chart.css:1 has the selector `:where(.gantt-x)`"),
    );
    one(
        &failures,
        web!("src/chart.css:2 has the selector `:where(.gantt, body)`"),
    );
    one(
        &failures,
        web!("src/chart.css:3 has the selector `:where(body, .gantt)`"),
    );
    one(
        &failures,
        web!("src/chart.css:4 has the selector `:where()`"),
    );
    one(
        &failures,
        web!("src/chart.css:5 has the selector `:where(.gantt`"),
    );
    one(
        &failures,
        web!("src/chart.css:6 has the selector `:where(.not .gantt)`"),
    );
    one(
        &failures,
        web!("src/chart.css:7 has the selector `:where(:is(.gantt))`"),
    );
}

#[test]
fn a_comma_inside_parentheses_does_not_split_a_selector() {
    let failures = scope(".gantt :is(.a, .b), .gantt [data-x=\"a,b\"] {}\n");
    assert!(failures.is_empty(), "{failures:#?}");
    let failures = scope(".gantt :is(.a, .b), :is(.c, .d) {}\n");
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, "`:is(.c, .d)`");
}

#[test]
fn media_blocks_are_read_through_and_keyframes_are_not_selectors() {
    let css = "@media (prefers-color-scheme: dark) {\n  :where(.gantt) {\n    --gantt-bg: #000;\n  }\n  body {\n    x: 1;\n  }\n}\n@keyframes spin {\n  from { x: 0 }\n  50% { x: 1 }\n  to { x: 2 }\n}\n@font-face { font-family: a; }\n@supports (display: grid) {\n  .gantt { x: 1 }\n  p { x: 1 }\n}\n@import url(a.css);\nul { x: 1 }\n";
    let failures = scope(css);
    assert_eq!(failures.len(), 3, "{failures:#?}");
    one(&failures, web!("src/chart.css:5 has the selector `body`"));
    one(&failures, web!("src/chart.css:17 has the selector `p`"));
    one(&failures, web!("src/chart.css:20 has the selector `ul`"));
}

#[test]
fn a_nested_rule_and_a_comment_do_not_count_as_selectors() {
    let css = "/* html { x: 1 } */\n.gantt {\n  color: red;\n  & .a { color: blue }\n  .b { color: green }\n}\n";
    let (lists, failures) = style_scope(web!("src/chart.css"), css);
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(lists, 1);
}

#[test]
fn only_the_style_blocks_of_the_page_are_read() {
    let html = "<!doctype html>\n<html style=\"margin: 0\">\n<head>\n<style>\n  .gantt { x: 1 }\n</style>\n<style type=\"text/css\">\n  body { x: 1 }\n</style>\n</head>\n<body style=\"margin: 0\"><p>body { not: css }</p></body>\n";
    let css = [file(web!("src/styles/tokens.css"), ".gantt { x: 1 }\n")];
    let failures = check_style_scope(&css, &index_page(html));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, web!("index.html:8 has the selector `body`"));
}

#[test]
fn only_the_pages_print_rule_may_name_html_and_body() {
    let print = "<style>.gantt { x: 1 }\n@media print {\n  html, body { x: 1 }\n  div { x: 1 }\n}\nbody { x: 1 }\n</style>";
    let css = [file(web!("src/styles/tokens.css"), ".gantt { x: 1 }\n")];
    let failures = check_style_scope(&css, &index_page(print));
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(&failures, web!("index.html:4 has the selector `div`"));
    one(&failures, web!("index.html:6 has the selector `body`"));
    let chart = [file(
        CHART_HTML,
        "<style>@media print { body { x: 1 } }</style>",
    )];
    let failures = check_style_scope(&css, &chart);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, web!("src/chart.html:1 has the selector `body`"));
}

#[test]
fn a_page_with_no_style_rules_fails_the_scan() {
    let failures = check_style_scope(&[], &index_page("<html><body></body></html>"));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, "found 0 selectors in the stylesheets");
    let css = [file(web!("src/chart.css"), "h1 { x: 1 }\n")];
    let failures = check_style_scope(&css, &index_page("<style>.gantt { x: 1 }</style>"));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, web!("src/chart.css:1 has the selector `h1`"));
    // The page itself has no `<style>` block; its rules are in the stylesheets.
    let css = [file(
        web!("src/styles/tokens.css"),
        ":where(.gantt) { x: 1 }\n",
    )];
    let failures = check_style_scope(&css, &index_page("<html><body></body></html>"));
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_style_block_in_the_chart_markup_is_read() {
    let chart = [file(CHART_HTML, "<style>\n  body { x: 1 }\n</style>\n")];
    let css = [file(web!("src/styles/tokens.css"), ".gantt { x: 1 }\n")];
    let failures = check_style_scope(&css, &chart);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, web!("src/chart.html:2 has the selector `body`"));
}
