//! Check 7: every `--gantt-` token the CSS, the scripts or the scene uses is on the page.

use std::collections::{BTreeMap, BTreeSet};

use crate::architecture::support::*;
use crate::architecture::web_root::WEB_ROOT;

pub(crate) const THEME_RS: &str = "crates/timeline/src/theme.rs";

// ---------------------------------------------------------------------------
// Check 7: theme tokens

/// Where a token is used: file and 1-based line.
pub(crate) type Site = (String, usize);

/// `--gantt-<name>` occurrences in `text`, as `(name, line)`. A name built at
/// run time (`--gantt-${token}`) has no characters after the prefix and is skipped.
pub(crate) fn css_tokens(text: &str) -> Vec<(String, usize)> {
    const PREFIX: &str = "--gantt-";
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let mut from = 0;
        while let Some(at) = line[from..].find(PREFIX) {
            let start = from + at + PREFIX.len();
            // The whole CSS identifier, so `--gantt-bar_edge` is never read as `bar`.
            let name: String = line[start..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || matches!(c, '-' | '_') || !c.is_ascii())
                .collect();
            if !name.is_empty() {
                out.push((name, i + 1));
            }
            from = start;
        }
    }
    out
}

/// The token keys the scene's `Theme` defines: lines like `bar => ("bar", ..);`
/// in `crates/timeline/src/theme.rs`.
pub(crate) fn scene_tokens(theme_rs: &str) -> Vec<(String, usize)> {
    theme_rs
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let (_, rest) = line.split_once("=> (\"")?;
            let (key, _) = rest.split_once('"')?;
            Some((key.to_string(), i + 1))
        })
        .collect()
}

/// Lowercase letters and digits, with single hyphens between them.
fn documented_spelling(token: &str) -> bool {
    !token.is_empty()
        && token.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

pub(crate) fn check_theming(
    listed: &[(String, usize)],
    used: &BTreeMap<String, Site>,
) -> Vec<String> {
    let mut failures = Vec::new();
    if listed.is_empty() {
        failures.push(format!(
            "{PAGE} (whole file): found 0 tokens in the \"Theming\" section; the parser reads nothing, \
             so this test would pass on an empty parse"
        ));
    }
    if used.is_empty() {
        failures.push(format!(
            "{INDEX_HTML}, {WEB_ROOT}/src and {THEME_RS} (whole files): found 0 --gantt- tokens \
                 in the CSS or the scene; the scan reads nothing, so this test would pass \
                 on an empty parse"
        ));
    }
    let listed_names: BTreeSet<&str> = listed.iter().map(|(n, _)| n.as_str()).collect();
    for (token, (file, line)) in used {
        if !documented_spelling(token) {
            failures.push(format!(
                "{file}:{line} uses --gantt-{token}, whose name is outside the documented \
                 spelling (lowercase letters and digits, single hyphens between them); rename \
                 it, or extend the parser in crates/app/tests/architecture/theming.rs"
            ));
        } else if !listed_names.contains(token.as_str()) {
            failures.push(format!(
                "{file}:{line} uses --gantt-{token} but the \"Theming\" section of {PAGE} \
                 does not list it; add a row for it"
            ));
        }
    }
    for (token, line) in listed {
        if !used.contains_key(token) {
            failures.push(format!(
                "{PAGE}:{line} lists --gantt-{token} but neither the CSS ({INDEX_HTML}, {WEB_ROOT}/src) \
                 nor the scene ({THEME_RS}) uses it"
            ));
        }
    }
    failures
}

/// The stylesheets that still hold a literal colour, as `(path, lines with one)`. `tokens.css` is the one place for a
/// colour. The list only shrinks: a file whose count differs from what it holds fails until the count is lowered.
pub(crate) const LITERAL_COLOURS: &[(&str, usize)] = &[(web!("src/styles/print.css"), 2)];

/// A hex colour, or a line that names an `rgb` or `hsl` function (`rgb`, `rgba`, `hsl`, `hsla`).
fn is_literal_colour(line: &str) -> bool {
    let call = line.contains("rgb") || line.contains("hsl");
    call || line.match_indices('#').any(|(i, _)| {
        let rest = &line[i + 1..];
        let n = rest
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(rest.len());
        matches!(n, 3 | 4 | 6 | 8) && !rest[n..].starts_with(|c| is_ident(c) || c == '-')
    })
}

pub(crate) fn check_literal_colours(css: &[SourceFile], exempt: &[(&str, usize)]) -> Vec<String> {
    let tokens = web!("src/styles/tokens.css");
    let mut failures = Vec::new();
    if css.is_empty() {
        failures.push(format!("{WEB_ROOT}/src: found 0 stylesheets"));
    }
    for sheet in css.iter().filter(|f| f.path != tokens) {
        let text = strip_block_comments(&sheet.text);
        let lines = (1..).zip(text.lines());
        let found: Vec<_> = lines
            .filter(|l| is_literal_colour(l.1))
            .map(|l| l.0)
            .collect();
        let allowed = exempt.iter().find(|e| e.0 == sheet.path).map_or(0, |e| e.1);
        if found.len() != allowed {
            failures.push(format!(
                "{}: lines {found:?} hold a literal colour and LITERAL_COLOURS (theming.rs) allows {allowed}; use a --gantt- token, or lower the count",
                sheet.path
            ));
        }
    }
    for (path, _) in exempt.iter().filter(|e| css.iter().all(|f| f.path != e.0)) {
        failures.push(format!(
            "LITERAL_COLOURS names {path}, which is no stylesheet"
        ));
    }
    failures
}

/// Where each `--gantt-` token is first used: the scene's token list, the CSS
/// (`index.html`, `chart.html`, standalone `.css` files) and the TypeScript sources.
pub(crate) fn used_tokens(
    theme_rs: &str,
    html: &[SourceFile],
    css: &[SourceFile],
    web: &[SourceFile],
) -> BTreeMap<String, Site> {
    let mut used: BTreeMap<String, Site> = BTreeMap::new();
    let mut note = |tokens: Vec<(String, usize)>, file: &str| {
        for (token, line) in tokens {
            used.entry(token)
                .or_insert_with(|| (file.to_string(), line));
        }
    };
    note(scene_tokens(theme_rs), THEME_RS);
    for file in html {
        note(css_tokens(&strip_block_comments(&file.text)), &file.path);
    }
    for file in css {
        note(css_tokens(&strip_block_comments(&file.text)), &file.path);
    }
    for file in web {
        note(css_tokens(&strip_ts_comments(&file.text)), &file.path);
    }
    used
}

#[test]
fn a_token_in_a_standalone_css_file_is_seen() {
    let css = [file(
        web!("src/chart.css"),
        "/* --gantt-old: 1; */\n.a {\n  color: var(--gantt-new-token);\n}\n",
    )];
    let used = used_tokens("", &[], &css, &[]);
    assert_eq!(
        used.get("new-token"),
        Some(&(web!("src/chart.css").to_string(), 3))
    );
    assert!(!used.contains_key("old"), "{used:?}");
    let failures = check_theming(&listed(&["bg"]), &used);
    one(
        &failures,
        web!("src/chart.css:3 uses --gantt-new-token but the \"Theming\" section"),
    );
}

#[test]
fn a_token_name_is_never_shortened_to_another_token() {
    let css = [file(
        web!("src/chart.css"),
        ".a {\n  color: var(--gantt-bar_edge);\n  border: var(--gantt-Bar);\n  fill: var(--gantt-bar--x);\n  stroke: var(--gantt-bar-);\n}\n",
    )];
    let used = used_tokens("", &[], &css, &[]);
    let failures = check_theming(&listed(&["bar"]), &used);
    for (token, line) in [("bar_edge", 2), ("Bar", 3), ("bar--x", 4), ("bar-", 5)] {
        one(
            &failures,
            &format!(
                "{WEB_ROOT}/src/chart.css:{line} uses --gantt-{token}, whose name is outside the documented spelling"
            ),
        );
    }
    // The same reading applies to a TypeScript source.
    let ts = [file(
        web!("src/x.ts"),
        "el.style.setProperty(\"--gantt-bar_edge\", v);\n",
    )];
    let failures = check_theming(&listed(&["bar"]), &used_tokens("", &[], &[], &ts));
    one(
        &failures,
        web!("src/x.ts:1 uses --gantt-bar_edge, whose name"),
    );
}

fn used(tokens: &[&str]) -> BTreeMap<String, Site> {
    tokens
        .iter()
        .enumerate()
        .map(|(i, t)| (t.to_string(), (web!("index.html").to_string(), i + 10)))
        .collect()
}

fn listed(tokens: &[&str]) -> Vec<(String, usize)> {
    tokens
        .iter()
        .enumerate()
        .map(|(i, t)| (t.to_string(), i + 100))
        .collect()
}

#[test]
fn a_token_used_but_not_listed_fails() {
    let failures = check_theming(&listed(&["bg"]), &used(&["bg", "bar"]));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert_eq!(
        failures[0],
        web!(
            "index.html:11 uses --gantt-bar but the \"Theming\" section of docs/WASM_API.md does \
         not list it; add a row for it"
        )
    );
}

#[test]
fn a_token_listed_but_unused_fails() {
    let failures = check_theming(&listed(&["bg", "gone"]), &used(&["bg"]));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert_eq!(
        failures[0],
        format!(
            "docs/WASM_API.md:101 lists --gantt-gone but neither the CSS ({WEB_ROOT}/index.html, {WEB_ROOT}/src) \
         nor the scene (crates/timeline/src/theme.rs) uses it"
        )
    );
}

#[test]
fn the_token_scanners_read_css_and_the_scene() {
    let css =
        "a { color: var(--gantt-bar-edge, red); --gantt-row-h: 22px; }\nb { x: `--gantt-${t}`; }\n";
    assert_eq!(
        css_tokens(css),
        vec![("bar-edge".to_string(), 1), ("row-h".to_string(), 1)]
    );
    assert_eq!(
        css_tokens(&strip_block_comments(
            "/* --gantt-old */\n.a { --gantt-new: 1; }\n"
        )),
        vec![("new".to_string(), 2)]
    );
    let scene =
        "            /// Bar.\n            bar_edge => (\"bar-edge\", (0, 0, 0), (1, 1, 1));\n";
    assert_eq!(scene_tokens(scene), vec![("bar-edge".to_string(), 2)]);
}

#[test]
fn a_literal_colour_outside_tokens_css_fails_and_a_listed_file_may_only_shrink() {
    let run = |path: &str, text: &str, ex: &[(&str, usize)]| {
        check_literal_colours(&[file(path, text)], ex)
    };
    let none = Vec::<String>::new();
    let (panels, moved) = (web!("src/styles/panels.css"), web!("src/styles/moved.css"));
    let clean = "a { color: var(--gantt-text); }\n/* #fff */\n#add-row, #overlay { top: 0 }\n";
    assert_eq!(run(panels, clean, &[]), none);
    for literal in ["#fff", "#e8ebef80", "rgba(0, 0, 0, 0.4)", "hsl(0 0% 0%)"] {
        let css = format!("{clean}b {{ color: {literal}; }}\n");
        one(
            &run(panels, &css, &[]),
            &format!("{panels}: lines [4] hold"),
        );
    }
    let tokens = web!("src/styles/tokens.css");
    assert_eq!(run(tokens, "a { --gantt-bg: #f2f5f9; }\n", &[]), none);
    let lit = "a { color: #fff; }\n";
    assert_eq!(run(moved, lit, &[(moved, 1)]), none);
    one(
        &run(moved, &format!("{lit}{lit}"), &[(moved, 1)]),
        "lines [1, 2] hold",
    );
    one(&run(moved, clean, &[(moved, 1)]), "lines [] hold");
    one(&run(panels, clean, &[(moved, 1)]), "names");
    one(&check_literal_colours(&[], &[]), "found 0 stylesheets");
}
