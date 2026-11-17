//! Check 11: every export of the page entry, `packages/gantt/src/index.ts`, is a row under "Page entry" in
//! `docs/WASM_API.md`, and every row has an export. No exported declaration of the entry names a type it
//! imports from `./measure.js` or `./page-knobs.js` (gs#552), directly, through a local `interface` or `type`,
//! or by re-export.

use std::collections::{BTreeMap, BTreeSet};

use crate::architecture::{support::*, web_root::WEB_ROOT};

/// What the reader found in the entry: the exported names with their lines, and the lines it could not read.
#[derive(Debug, Default, PartialEq)]
struct Exports {
    names: Vec<(String, usize)>,
    /// Forms whose names are not in the text (`export *`, `export default`, a destructuring `export const`).
    unreadable: Vec<(String, usize)>,
}

/// The word that follows `export` and starts a declaration.
const DECLARATIONS: [&str; 9] = [
    "function",
    "class",
    "const",
    "let",
    "var",
    "interface",
    "type",
    "enum",
    "namespace",
];

/// The first identifier in `text`, after any `*` of a generator.
fn first_name(text: &str) -> Option<String> {
    let text = text.trim_start().trim_start_matches('*').trim_start();
    let name: String = text.chars().take_while(|c| is_ident(*c)).collect();
    (!name.is_empty()).then_some(name)
}

/// The names in the body of `export { a, type B, c as d }` (the name a host imports is the last one).
fn braced_names(body: &str) -> Vec<String> {
    body.split(',')
        .filter_map(|item| {
            let item = item.trim();
            let item = item.strip_prefix("type ").unwrap_or(item);
            let name = item.rsplit(" as ").next().unwrap_or(item).trim();
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

/// Whether the declaration whose first name is read has a second: a comma outside every bracket, string,
/// regex literal and generic, before its `;` (or the next line that starts at the margin).
/// A `<` opens a generic after a name, `=` or `async`, with no space after it; `x<y, b = 2` reads the same,
/// so a generic still open when the statement ends was a comparison, and a comma in it is a second name.
/// A `/` after `=(,:[!&|?{` opens a regex literal, read to its next `/` (one holding `/` is cut short).
/// Lines inside a generic must be indented: one at the margin ends the statement.
fn has_second_name(tail: &str) -> bool {
    let mut open: Vec<(char, bool)> = Vec::new();
    let (mut quote, mut prev, mut last) = (None, ' ', ' ');
    let mut chars = tail.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        let next = chars.peek().map_or(' ', |p| p.1);
        let (before, bare) = (tail[..at].trim_end(), open.iter().all(|g| g.0 == '>'));
        let opens = (is_ident(prev) || before.ends_with('=') || before.ends_with("async"))
            && !matches!(next, ' ' | '\t' | '=');
        match (quote, c) {
            (Some(_), '\\') => _ = chars.next(),
            (Some(q), _) if c == q => quote = None,
            (None, '"' | '\'' | '`') => quote = Some(c),
            (None, '/') if "=(,:[!&|?{".contains(last) => {
                while chars.next_if(|p| p.1 != '\n').is_some_and(|p| p.1 != '/') {}
            }
            (None, '(') => open.push((')', false)),
            (None, '[') => open.push((']', false)),
            (None, '{') => open.push(('}', false)),
            (None, '<') if opens => open.push(('>', false)),
            (None, ')' | ']' | '}' | '>') if prev != '=' => {
                // A close also ends the generics left open inside it (`f(a<b, c)`).
                let from = open.iter().rposition(|g| g.0 == c);
                open.truncate(
                    from.filter(|i| c != '>' || i + 1 == open.len())
                        .unwrap_or(open.len()),
                );
            }
            (None, ',') if open.is_empty() => return true,
            (None, ',') => _ = open.last_mut().map(|g| g.1 = true),
            (None, ';' | '\n') if bare && (c == ';' || is_ident(next)) => break,
            _ => {}
        }
        prev = c;
        last = if c.is_whitespace() { last } else { c };
    }
    open.iter().any(|g| g.1)
}

/// Every export of `text` (comments already blanked). Reads top-level statements that start with `export`.
fn exports_of(text: &str) -> Exports {
    let mut out = Exports::default();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let Some(rest) = line.trim_start().strip_prefix("export") else {
            continue;
        };
        if rest.starts_with(is_ident) {
            continue; // `exported`, `exports`: not the keyword
        }
        let rest = rest.trim_start();
        if rest.starts_with('*') {
            out.unreadable.push(("export *".to_string(), n));
        } else if let Some(braced) = rest
            .strip_prefix("type")
            .map_or(rest, str::trim_start)
            .strip_prefix('{')
        {
            // The list may run over several lines: read to the closing brace.
            let from = text.lines().take(i).map(|l| l.len() + 1).sum::<usize>();
            let start = from + line.len() - braced.len();
            let body = text[start..].split('}').next().unwrap_or("");
            out.names
                .extend(braced_names(body).into_iter().map(|name| (name, n)));
        } else {
            let mut words = rest;
            // `export default`, `export declare` and the modifiers before the keyword.
            loop {
                let word: String = words.chars().take_while(|c| is_ident(*c)).collect();
                match word.as_str() {
                    "default" => {
                        out.unreadable.push(("export default".to_string(), n));
                        break;
                    }
                    "declare" | "async" | "abstract" => {
                        words = words[word.len()..].trim_start();
                    }
                    w if DECLARATIONS.contains(&w) => {
                        let after = words[w.len()..].trim_start();
                        match first_name(after) {
                            Some(name) => {
                                let from = text.lines().take(i).map(|l| l.len() + 1).sum::<usize>();
                                let tail = &text[from + line.len() - after.len() + name.len()..];
                                if matches!(w, "const" | "let" | "var") && has_second_name(tail) {
                                    out.unreadable.push((format!("export {w} {name}, ..."), n));
                                }
                                out.names.push((name, n));
                            }
                            None => out.unreadable.push((format!("export {w}"), n)),
                        }
                        break;
                    }
                    _ => {
                        out.unreadable
                            .push((format!("export {}", rest.trim_end()), n));
                        break;
                    }
                }
            }
        }
    }
    out
}

/// Failures for `entry` (the text of `packages/gantt/src/index.ts`) against the rows of the page.
pub(crate) fn check_entry(entry: &str, rows: &[(String, usize)]) -> Vec<String> {
    let text = strip_ts_comments(entry);
    let exports = exports_of(&text);
    let mut failures = check_entry_types(&text);
    if exports.names.is_empty() {
        failures.push(format!(
            "{INDEX_TS}: found 0 exports; the scan reads nothing (did the entry move? fix the path in the test, because an empty parse would pass)"
        ));
    }
    if rows.is_empty() {
        failures.push(format!(
            "{PAGE}: found 0 rows under \"Page entry\"; the section is missing or moved (an empty parse would pass)"
        ));
    }
    for (what, line) in &exports.unreadable {
        failures.push(format!(
            "{INDEX_TS}:{line} has `{what}`, whose names the check cannot read; name each export (`export function x`, `export {{ x }}`)"
        ));
    }
    let row_names: BTreeSet<&str> = rows.iter().map(|(name, _)| name.as_str()).collect();
    let export_names: BTreeSet<&str> = exports
        .names
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    for (name, line) in &exports.names {
        if !row_names.contains(name.as_str()) {
            failures.push(format!(
                "{INDEX_TS}:{line} exports `{name}`, which has no row under \"Page entry\" in {PAGE}; add a row, or stop exporting it"
            ));
        }
    }
    let mut seen = BTreeSet::new();
    for (name, line) in rows {
        if !seen.insert(name.as_str()) {
            failures.push(format!(
                "{PAGE}:{line} has a second row for `{name}` under \"Page entry\"; one row per export"
            ));
        } else if !export_names.contains(name.as_str()) {
            failures.push(format!(
                "{PAGE}:{line} has a row for `{name}` under \"Page entry\", but {INDEX_TS} exports no `{name}`; remove the row, or export it"
            ));
        }
    }
    failures
}

/// The modules only the developer page may use, by file stem under `packages/gantt/src` (`measure.ts` and
/// `measure/`, `page-knobs.ts`). A type from one of them must not be reachable from the entry's exported
/// declarations: a host page that imports the entry would see the measure types (gs#552).
const PAGE_ONLY: [&str; 2] = ["measure", "page-knobs"];

/// Whether the import specifier `spec` names a page-only module.
fn is_page_only(spec: &str) -> bool {
    let first = spec
        .strip_prefix("./")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");
    PAGE_ONLY.contains(&first.split('.').next().unwrap_or(""))
}

/// The top-level lines (no indent) of `text`: line number, byte offset and the line.
fn top_lines(text: &str) -> Vec<(usize, usize, &str)> {
    let mut at = 0;
    text.split_inclusive('\n')
        .enumerate()
        .map(|(i, line)| {
            let start = at;
            at += line.len();
            (i + 1, start, line.trim_end_matches('\n'))
        })
        .collect()
}

/// `text` from `from` to its first `;` (or the end).
fn statement_from(text: &str, from: usize) -> &str {
    let rest = &text[from..];
    &rest[..rest.find(';').map_or(rest.len(), |at| at + 1)]
}

/// An import or export statement split at its `from`: the clause before it, and the specifier after it.
fn split_from(stmt: &str) -> Option<(&str, &str)> {
    let hits: Vec<_> = stmt.match_indices("from").collect();
    let (at, _) = hits.into_iter().rev().find(|(at, _)| {
        stmt[at + 4..].trim_start().starts_with(['"', '\''])
            && stmt[..*at].ends_with(|c: char| c.is_whitespace() || c == '}')
    })?;
    let spec = stmt[at + 4..].trim().trim_end_matches(';').trim();
    let spec = spec.strip_prefix(['"', '\''])?.strip_suffix(['"', '\''])?;
    Some((&stmt[..at], spec))
}

/// The local names an import clause binds: `a`, `a, { b, type C, d as e }`, `* as ns`, `type { F }`.
fn clause_names(clause: &str) -> Vec<String> {
    let clause = clause.trim();
    let clause = match clause.strip_prefix("type") {
        Some(rest) if rest.starts_with(|c: char| c.is_whitespace() || c == '{') => rest,
        _ => clause,
    };
    let (head, braced) = match clause.split_once('{') {
        Some((head, rest)) => (head, rest.split('}').next().unwrap_or("")),
        None => (clause, ""),
    };
    let mut names: Vec<String> = head
        .split(',')
        .filter_map(|part| first_name(part.trim().strip_prefix("* as ").unwrap_or(part)))
        .collect();
    names.extend(braced_names(braced));
    names
}

/// Every name `text` imports from a page-only module, with the module it comes from.
fn page_only_imports(text: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    for (_, start, line) in top_lines(text) {
        let Some(rest) = line.strip_prefix("import") else {
            continue;
        };
        if !rest.starts_with(|c: char| c.is_whitespace() || c == '{' || c == '*') {
            continue; // `important` and `import(` are not import statements
        }
        let Some((clause, spec)) = split_from(statement_from(text, start + "import".len())) else {
            continue;
        };
        if is_page_only(spec) {
            for name in clause_names(clause) {
                found.insert(name, spec.to_string());
            }
        }
    }
    found
}

/// A top-level declaration of the entry that can carry a type.
struct Decl {
    name: String,
    exported: bool,
    /// `interface` and `type` declarations are read whole; the others only to their signature.
    is_type: bool,
    line: usize,
    text: String,
}

/// The end of the declaration that starts at `from`: an `interface` at its closing brace, a `type` at its `;`.
fn type_end(text: &str, from: usize, interface: bool) -> usize {
    let mut depth = 0usize;
    for (at, c) in text[from..].char_indices() {
        match c {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && interface && c == '}' {
                    return from + at + 1;
                }
            }
            ';' if depth == 0 => return from + at + 1,
            _ => {}
        }
    }
    text.len()
}

/// The signature of a function or class: its lines to the first one that ends in `{`.
fn signature_end(text: &str, from: usize) -> usize {
    let mut end = from;
    for line in text[from..].split_inclusive('\n') {
        end += line.len();
        if line.trim_end().ends_with('{') {
            break;
        }
    }
    end
}

/// Every top-level `interface`, `type`, `class`, `function`, `const`, `let`, `var` and `enum`, exported or not.
fn declarations(text: &str) -> Vec<Decl> {
    let mut out = Vec::new();
    for (line, start, text_line) in top_lines(text) {
        let (exported, mut rest) = match text_line.strip_prefix("export ") {
            Some(after) => (true, after.trim_start()),
            None => (false, text_line),
        };
        while let Some(after) = ["declare ", "async ", "abstract "]
            .iter()
            .find_map(|m| rest.strip_prefix(m))
        {
            rest = after.trim_start();
        }
        let Some(word) = DECLARATIONS.iter().find(|w| {
            rest.strip_prefix(**w)
                .is_some_and(|r| r.starts_with(char::is_whitespace))
        }) else {
            continue;
        };
        let Some(name) = first_name(&rest[word.len()..]) else {
            continue;
        };
        let is_type = matches!(*word, "interface" | "type");
        let end = match *word {
            "interface" | "type" => type_end(text, start, *word == "interface"),
            "function" | "class" => signature_end(text, start),
            _ => start + text_line.len(),
        };
        out.push(Decl {
            name,
            exported,
            is_type,
            line,
            text: text[start..end].to_string(),
        });
    }
    out
}

/// The identifiers in `text`.
fn identifiers(text: &str) -> BTreeSet<&str> {
    text.split(|c| !is_ident(c))
        .filter(|word| !word.is_empty() && !word.starts_with(|c: char| c.is_ascii_digit()))
        .collect()
}

/// Failures for a page-only name (a type of the developer page's knobs or of the measured run) reachable from the
/// entry's exports (gs#552). `text` is the entry with its comments blanked. Reached from an exported declaration
/// directly, through a local `interface` or `type` it names, or by re-export.
pub(crate) fn check_entry_types(text: &str) -> Vec<String> {
    let imports = page_only_imports(text);
    let decls = declarations(text);
    let local: BTreeMap<&str, &Decl> = decls
        .iter()
        .filter(|d| d.is_type)
        .map(|d| (d.name.as_str(), d))
        .collect();
    let mut failures = Vec::new();
    for root in decls.iter().filter(|d| d.exported) {
        let mut seen = BTreeSet::from([root.name.as_str()]);
        let mut stack = vec![(root, Vec::<&str>::new())];
        while let Some((decl, via)) = stack.pop() {
            for word in identifiers(&decl.text) {
                if word == decl.name {
                    continue;
                }
                if let Some(module) = imports.get(word) {
                    let through = if via.is_empty() {
                        String::new()
                    } else {
                        format!(" (through `{}`)", via.join("`, `"))
                    };
                    failures.push(format!(
                        "{INDEX_TS}:{} the export `{}` names `{word}`{through}, which comes from `{module}`, a module only the developer page may use; a host page would see it. Keep it out of the entry's types: pass it through `{WEB_ROOT}/src/page-knobs.ts`",
                        root.line, root.name
                    ));
                } else if let Some(next) = local.get(word)
                    && seen.insert(next.name.as_str())
                {
                    let mut path = via.clone();
                    path.push(next.name.as_str());
                    stack.push((next, path));
                }
            }
        }
    }
    for (line, start, line_text) in top_lines(text) {
        if !line_text.starts_with("export") || !line_text.contains('{') {
            continue;
        }
        let stmt = statement_from(text, start);
        let reexport = split_from(stmt).filter(|(_, spec)| is_page_only(spec));
        let clause = stmt
            .split_once('{')
            .map_or("", |(_, rest)| rest.split('}').next().unwrap_or(""));
        let named = braced_names(clause)
            .into_iter()
            .find(|name| imports.contains_key(name));
        if let Some((_, spec)) = reexport {
            failures.push(format!(
                "{INDEX_TS}:{line} re-exports from `{spec}`, a module only the developer page may use; a host page would see it"
            ));
        } else if let Some(name) = named {
            failures.push(format!(
                "{INDEX_TS}:{line} exports `{name}`, which comes from `{}`, a module only the developer page may use; a host page would see it",
                imports[&name]
            ));
        }
    }
    failures
}

fn entry_rows(names: &[&str]) -> Vec<(String, usize)> {
    names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_string(), 100 + i))
        .collect()
}

const GOOD: &str = "import x from \"./x.js\";\n\n/** Doc. */\nexport interface Opts { a: number }\nexport class Oops extends Error {}\nexport async function mount(): Promise<void> {}\nexport const seven = 7;\n";

#[test]
fn an_entry_whose_exports_are_all_rows_passes() {
    assert_eq!(
        check_entry(GOOD, &entry_rows(&["Opts", "Oops", "mount", "seven"])),
        Vec::<String>::new()
    );
}

#[test]
fn an_export_with_no_row_fails_with_its_file_and_line() {
    let text = format!("{GOOD}export const x = 1;\n");
    let failures = check_entry(&text, &entry_rows(&["Opts", "Oops", "mount", "seven"]));
    one(
        &failures,
        web!("src/index.ts:8 exports `x`, which has no row"),
    );
    assert_eq!(failures.len(), 1, "{failures:#?}");
}

#[test]
fn a_row_with_no_export_fails_with_its_page_line() {
    let failures = check_entry(
        GOOD,
        &entry_rows(&["Opts", "Oops", "mount", "seven", "gone"]),
    );
    one(&failures, "docs/WASM_API.md:104 has a row for `gone`");
    assert_eq!(failures.len(), 1, "{failures:#?}");
}

#[test]
fn a_second_row_for_one_export_fails() {
    let failures = check_entry(
        GOOD,
        &entry_rows(&["Opts", "Oops", "mount", "seven", "mount"]),
    );
    one(
        &failures,
        "docs/WASM_API.md:104 has a second row for `mount`",
    );
}

#[test]
fn every_form_of_export_is_read() {
    let text = "export function a() {}\nexport function* b() {}\nexport let c = 1;\nexport var d = 1;\nexport type E = string;\nexport enum F { G }\nexport abstract class H {}\nexport declare const I: number;\nexport {\n  j,\n  type K,\n  l as M,\n};\nexport type { N } from \"./n.js\";\nexport { O } from \"./o.js\";\n";
    let names: Vec<_> = exports_of(text).names.into_iter().collect();
    let want = [
        ("a", 1),
        ("b", 2),
        ("c", 3),
        ("d", 4),
        ("E", 5),
        ("F", 6),
        ("H", 7),
        ("I", 8),
        ("j", 9),
        ("K", 9),
        ("M", 9),
        ("N", 14),
        ("O", 15),
    ];
    let want: Vec<_> = want.iter().map(|(n, l)| (n.to_string(), *l)).collect();
    assert_eq!(names, want);
}

#[test]
fn a_form_the_check_cannot_read_fails_closed() {
    let text = "export * from \"./a.js\";\nexport default function () {}\nexport const { p, q } = o;\nexport let;\n";
    let failures = check_entry(text, &entry_rows(&["mount"]));
    one(&failures, web!("src/index.ts:1 has `export *`"));
    one(&failures, web!("src/index.ts:2 has `export default`"));
    one(&failures, web!("src/index.ts:3 has `export const`"));
}

#[test]
fn a_multi_name_declaration_fails_closed_and_names_its_line() {
    let text = "export const y = 1, z = 2;\nexport let a: number,\n  b: number;\n";
    let failures = check_entry(text, &entry_rows(&["y", "a"]));
    one(&failures, web!("src/index.ts:1 has `export const y, ...`"));
    one(&failures, web!("src/index.ts:2 has `export let a, ...`"));
    assert_eq!(failures.len(), 2, "{failures:#?}");
}

#[test]
fn a_comma_inside_a_bracket_string_or_generic_is_still_one_name() {
    let text = "export const a = f(1, 2);\nexport const b = {\n  p: 1,\n  q: 2,\n};\nexport const c = [1, 2];\nexport const e = \"x, y\";\nexport const g: Map<string, number> = new Map<string, number>();\nexport const h = (x: number, y: number) => {\n  return x;\n};\nexport let i: (a: number, b: number) => void;\nexport const j = 1;\n";
    let rows = entry_rows(&["a", "b", "c", "e", "g", "h", "i", "j"]);
    assert_eq!(check_entry(text, &rows), Vec::<String>::new());
}

#[test]
fn a_comment_or_a_longer_word_is_not_an_export() {
    let text = "// export const a = 1;\n/* export const b = 1;\n*/\nconst exported = 1;\nexport const c = 1;\n";
    let failures = check_entry(text, &entry_rows(&["c"]));
    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn an_empty_parse_fails() {
    let failures = check_entry("const a = 1;\n", &[]);
    one(&failures, web!("src/index.ts: found 0 exports"));
    one(&failures, "found 0 rows under \"Page entry\"");
}

#[test]
fn every_form_of_import_is_read() {
    let text = "import M, { type A, B as C } from \"./measure.js\";\nimport * as ns from \"./measure/x.js\";\nimport type {\n  D,\n  E,\n} from \"./page-knobs.js\";\nimport \"./styles/a.css\";\nimport { F } from \"./measure-other.js\";\nimport { G } from \"./from.js\";\nconst important = import(\"./measure.js\");\n";
    let imports = page_only_imports(text);
    let names: Vec<&str> = imports.keys().map(String::as_str).collect();
    assert_eq!(names, ["A", "C", "D", "E", "M", "ns"]);
}

fn multi(rest: &str) -> bool {
    !exports_of(&format!("export const {rest}\n"))
        .unreadable
        .is_empty()
}

#[test]
fn a_generic_arrow_is_one_name() {
    assert!(!multi("f = <A, B>(a: A, b: B) => a;"));
    assert!(!multi("f = async <A, B>(a: A) => a;"));
}

#[test]
fn a_generic_arrow_with_a_trailing_comma_is_one_name() {
    assert!(!multi("f = <T,>(x: T) => x;"));
}

#[test]
fn a_generic_written_over_lines_is_one_name() {
    assert!(!multi("g: Record<\n  string,\n  number\n> = {};"));
}

#[test]
fn a_comma_in_a_regex_literal_is_one_name() {
    assert!(!multi("r = /a,b/;") && !multi("s = x.split(/[,;]/, 2);"));
}

#[test]
fn a_comparison_with_a_second_name_is_two_names_and_a_division_is_not_a_regex() {
    assert!(multi("a = x<y, b = 2;") && multi("d = a / b, c = 1;"));
}
