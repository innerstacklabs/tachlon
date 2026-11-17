//! Check 8: the chart takes its size and its keys from its own root.
//!
//! The window size and the key listeners on the page are read here. The page lookups (a `getElementById`,
//! `querySelector` or `addEventListener` on the document outside `packages/gantt/src/main.ts`) are read from the syntax
//! tree in the Node tier, `packages/gantt/tests-node/page-lookups.test.ts`.

use std::collections::BTreeSet;

use crate::architecture::support::*;

// ---------------------------------------------------------------------------
// Check 8: the chart takes its size and its keys from its own root

/// The one file that may read the window's size: the measurement report records it.
const MEASURE_TS: &str = web!("src/measure.ts");

/// Reading the window's size makes the chart window-sized wherever it is placed.
const WINDOW_SIZE_NAMES: [&str; 4] = ["innerWidth", "innerHeight", "outerWidth", "outerHeight"];

/// CSS units that are a share of the window, in lower case (a unit is matched without regard to case).
const WINDOW_UNITS: [&str; 18] = [
    "vw", "vh", "vi", "vb", "vmin", "vmax", "svw", "svh", "svmin", "svmax", "lvw", "lvh", "lvmin",
    "lvmax", "dvw", "dvh", "dvmin", "dvmax",
];

const KEY_EVENTS: [&str; 3] = ["keydown", "keyup", "keypress"];

/// The window-sized units in `line` (`100vh`, `1.5dvw`).
fn window_units(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !(chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
        }
        let unit: String = chars[i..]
            .iter()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if (start == 0 || !is_ident(chars[start - 1]))
            && WINDOW_UNITS.contains(&unit.to_ascii_lowercase().as_str())
        {
            found.push(unit);
        }
    }
    found
}

/// Failures for a window size read in `text` (comments already blanked): a name or a unit.
fn window_size_uses(path: &str, text: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let mut seen = BTreeSet::new();
        let mut names: Vec<String> = identifiers(line)
            .into_iter()
            .filter(|id| WINDOW_SIZE_NAMES.contains(id) && seen.insert(*id))
            .map(str::to_string)
            .collect();
        names.extend(window_units(line));
        for name in names {
            failures.push(format!(
                "{path}:{n} uses {name}, which is the window's size; the chart takes its size \
                 from the box its host gives it, so use the root's own size or a percentage \
                 (docs/ARCHITECTURE.md \"Embedding\")"
            ));
        }
    }
    failures
}

/// The expression just before `at` (the `.` of `.addEventListener`): dotted identifiers, `?` dropped,
/// and a chain broken across lines (`document` then `.body` on the next line) read as one.
fn receiver_before(text: &str, at: usize) -> String {
    let mut out: Vec<char> = Vec::new();
    for c in text[..at].chars().rev() {
        if is_ident(c) || c == '.' {
            out.push(c);
        } else if !(c == '?' || (c.is_whitespace() && out.last().is_none_or(|l| *l == '.'))) {
            break;
        }
    }
    out.iter().rev().collect()
}

/// Whether `receiver` is the window, the document, or the document's body or root element.
fn is_page_target(receiver: &str) -> bool {
    let bare = ["window.", "globalThis.", "self."]
        .iter()
        .find_map(|p| receiver.strip_prefix(p))
        .unwrap_or(receiver);
    matches!(
        bare,
        "window"
            | "globalThis"
            | "self"
            | "document"
            | "document.body"
            | "document.documentElement"
    )
}

/// Key listeners on the window or the document (comments already blanked): `addEventListener` with a
/// literal key event, and `onkeydown`-style properties. Offsets are into `text`.
fn page_key_listeners(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(".addEventListener") {
        let dot = from + at;
        from = dot + 1;
        let rest = text[dot + ".addEventListener".len()..].trim_start();
        let Some(rest) = rest.strip_prefix('(') else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(quote) = rest
            .chars()
            .next()
            .filter(|c| matches!(c, '"' | '\'' | '`'))
        else {
            continue;
        };
        let event: String = rest[1..].chars().take_while(|c| *c != quote).collect();
        let receiver = receiver_before(text, dot);
        if KEY_EVENTS.contains(&event.as_str()) && is_page_target(&receiver) {
            found.push((dot, format!("{receiver}.addEventListener(\"{event}\")")));
        }
    }
    for event in KEY_EVENTS {
        let prop = format!(".on{event}");
        let mut from = 0;
        while let Some(at) = text[from..].find(&prop) {
            let dot = from + at;
            from = dot + 1;
            let next = text[dot + prop.len()..].chars().next();
            let receiver = receiver_before(text, dot);
            if !next.is_some_and(is_ident) && is_page_target(&receiver) {
                found.push((dot, format!("{receiver}.on{event}")));
            }
        }
    }
    found
}

/// `onkeydown`-style attributes on the page's `<html>` and `<body>` tags.
fn page_key_attributes(path: &str, html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut failures = Vec::new();
    for tag in ["<html", "<body"] {
        let mut from = 0;
        while let Some(at) = lower[from..].find(tag) {
            let start = from + at;
            let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
            from = end;
            if let Some(event) = KEY_EVENTS
                .iter()
                .find(|e| lower[start..end].contains(&format!("on{e}")))
            {
                failures.push(format!(
                    "{path}:{} puts on{event} on {}; a key handler on the page is outside \
                     the chart root, so it would act while focus is elsewhere",
                    line_of(html, start),
                    &tag[1..]
                ));
            }
        }
    }
    failures
}

pub(crate) fn check_embedding(
    web: &[SourceFile],
    css: &[SourceFile],
    html: &[SourceFile],
) -> Vec<String> {
    let mut failures = Vec::new();
    for file in web {
        let text = strip_ts_comments(&file.text);
        if file.path != MEASURE_TS {
            failures.extend(window_size_uses(&file.path, &text));
        }
        for (offset, what) in page_key_listeners(&text) {
            failures.push(format!(
                "{}:{} has {what}; keys act only while focus is inside the chart, so the listener \
                 belongs on the chart's root, not on the page (decision 0005)",
                file.path,
                line_of(&text, offset)
            ));
        }
    }
    for file in css {
        failures.extend(window_size_uses(
            &file.path,
            &strip_block_comments(&file.text),
        ));
    }
    for file in html {
        failures.extend(window_size_uses(
            &file.path,
            &strip_block_comments(&file.text),
        ));
        failures.extend(page_key_attributes(&file.path, &file.text));
    }
    failures
}

// ---------------------------------------------------------------------------
// Checks 8 and 9 fire: one synthetic failing case per message

fn ts(path: &str, text: &str) -> Vec<String> {
    check_embedding(&[file(path, text)], &[], &[])
}

#[test]
fn a_window_size_in_the_web_sources_fails() {
    let failures = ts(
        web!("src/main.ts"),
        "const x = innerWidth;\nconst y = window.innerHeight + outerWidth;\nconst z = `${outerHeight}px`;\n",
    );
    assert_eq!(failures.len(), 4, "{failures:#?}");
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:1 uses innerWidth, which is the window's size"
        ),
    );
    one(&failures, web!("src/main.ts:2 uses innerHeight"));
    one(&failures, web!("src/main.ts:2 uses outerWidth"));
    one(&failures, web!("src/main.ts:3 uses outerHeight"));
}

#[test]
fn a_window_size_is_allowed_in_the_measurement_file_and_in_comments() {
    assert_eq!(ts(MEASURE_TS, "const w = innerWidth;\n"), [] as [String; 0]);
    let failures = ts(
        web!("src/main.ts"),
        "// innerWidth\n/* innerHeight\n   outerWidth */\nconst innerWidthOfRoot = 1;\nconst r = root.clientWidth;\n",
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_window_unit_in_css_fails_and_a_percentage_does_not() {
    let html = "<style>\n.gantt {\n  height: 100vh;\n  width: calc(100% - 32px);\n}\n.gantt #a {\n  max-height: 1.5dvh;\n  top: 12px;\n}\n/* 50vw */\n</style>\n";
    let failures = check_embedding(&[], &[], &index_page(html));
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(&failures, web!("index.html:3 uses vh"));
    one(&failures, web!("index.html:7 uses dvh"));
    let css = [file(
        web!("src/chart.css"),
        ".gantt { width: 50vw; margin: 0.5vmin; }\n",
    )];
    let failures = check_embedding(&[], &css, &[]);
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(&failures, web!("src/chart.css:1 uses vw"));
    one(&failures, web!("src/chart.css:1 uses vmin"));
}

#[test]
fn a_word_that_only_ends_like_a_unit_is_not_a_unit() {
    assert_eq!(
        window_units("a: 100px; b: #1c2430; c: 12vhx; d: v100vh"),
        Vec::<String>::new()
    );
    assert_eq!(window_units("x: calc(100vh - 2px)"), vec!["vh".to_string()]);
    assert_eq!(window_units("x: .5vw"), vec!["vw".to_string()]);
}

#[test]
fn every_viewport_unit_is_found_in_any_case() {
    let units = [
        "vw", "vh", "vi", "vb", "vmin", "vmax", "svw", "svh", "svmin", "svmax", "lvw", "lvh",
        "lvmin", "lvmax", "dvw", "dvh", "dvmin", "dvmax",
    ];
    assert_eq!(units.len(), WINDOW_UNITS.len());
    for unit in units {
        let line = format!("x: calc(10{unit} - 2px)");
        assert_eq!(window_units(&line), vec![unit.to_string()], "{line}");
        let upper = unit.to_ascii_uppercase();
        let line = format!("x: 10{upper}");
        assert_eq!(window_units(&line), vec![upper], "{line}");
    }
    assert_eq!(window_units("x: 5Svh 6dVMIN"), vec!["Svh", "dVMIN"]);
    let css = [file(
        web!("src/chart.css"),
        ".gantt { height: 100SVMAX; }\n",
    )];
    let failures = check_embedding(&[], &css, &[]);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, web!("src/chart.css:1 uses SVMAX"));
}

#[test]
fn a_key_listener_on_the_page_fails() {
    let src = "\
window.addEventListener(\"keydown\", f);
document.addEventListener('keyup', f);
document.body.addEventListener(
  `keypress`,
  f,
);
document.documentElement?.addEventListener(\"keydown\", f);
window.document.addEventListener(\"keydown\", f);
globalThis.addEventListener(\"keydown\", f);
window.onkeydown = f;
document.body.onkeyup = f;
";
    let failures = ts(web!("src/main.ts"), src);
    assert_eq!(failures.len(), 8, "{failures:#?}");
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:1 has window.addEventListener(\"keydown\")"
        ),
    );
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:2 has document.addEventListener(\"keyup\")"
        ),
    );
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:3 has document.body.addEventListener(\"keypress\")"
        ),
    );
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:7 has document.documentElement.addEventListener(\"keydown\")"
        ),
    );
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:8 has window.document.addEventListener(\"keydown\")"
        ),
    );
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:9 has globalThis.addEventListener(\"keydown\")"
        ),
    );
    one(&failures, web!("src/main.ts:10 has window.onkeydown"));
    one(&failures, web!("src/main.ts:11 has document.body.onkeyup"));
    assert!(
        failures
            .iter()
            .any(|f| f.contains("belongs on the chart's root"))
    );
}

#[test]
fn a_key_listener_inside_the_chart_or_another_event_on_the_page_passes() {
    let src = "\
root.addEventListener(\"keydown\", f);
findInput?.addEventListener(\"keydown\", f);
input.addEventListener(\"keydown\", f);
window.addEventListener(\"resize\", f);
document.addEventListener(\"visibilitychange\", f);
document.getElementById(\"x\").addEventListener(\"keydown\", f);
window.addEventListener(type, f);
root.onkeydown = f;
window.onkeydownish = f;
// window.addEventListener(\"keydown\", f);
";
    let failures = ts(web!("src/main.ts"), src);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_key_attribute_on_html_or_body_fails() {
    let html = "<!doctype html>\n<html lang=\"en\">\n<body onkeydown=\"f()\">\n<div onkeydown=\"g()\"></div>\n";
    let failures = check_embedding(&[], &[], &index_page(html));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, web!("index.html:3 puts onkeydown on body"));
    let failures = check_embedding(&[], &[], &index_page("<HTML ONKEYUP=\"f()\"><body>"));
    one(&failures, web!("index.html:1 puts onkeyup on html"));
}

#[test]
fn a_window_unit_in_the_chart_markup_fails_and_the_page_read_includes_it() {
    let chart = [file(
        CHART_HTML,
        "<div id=\"a\">\n<p style=\"height: 100vh\"></p>\n</div>\n",
    )];
    let failures = check_embedding(&[], &[], &chart);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(&failures, web!("src/chart.html:2 uses vh"));
    let paths: Vec<_> = web_html().into_iter().map(|f| f.path).collect();
    assert_eq!(paths, [INDEX_HTML, CHART_HTML]);
}
