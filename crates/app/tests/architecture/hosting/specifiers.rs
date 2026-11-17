//! Reading module specifiers and page references out of TypeScript and HTML text (check 12).

use crate::architecture::support::*;

pub(super) fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

pub(super) fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// The keyword `kw` at `i`: not part of a longer identifier, and not a property (`x.import`).
fn keyword_at(b: &[u8], i: usize, kw: &str) -> bool {
    b[i..].starts_with(kw.as_bytes())
        && (i == 0 || !(is_ident_byte(b[i - 1]) || b[i - 1] == b'.'))
        && !b.get(i + kw.len()).copied().is_some_and(is_ident_byte)
}

pub(super) fn is_quote(c: Option<&u8>) -> bool {
    matches!(c, Some(b'"' | b'\'' | b'`'))
}

/// The text of the literal that opens at `i`, when it closes on the same line and holds no `${`.
pub(super) fn literal_at(text: &str, i: usize) -> Option<String> {
    let q = *text.as_bytes().get(i)?;
    if !is_quote(Some(&q)) {
        return None;
    }
    let rest = &text[i + 1..];
    let end = rest.find([q as char, '\n'])?;
    let literal = &rest[..end];
    (rest.as_bytes()[end] == q && !literal.contains("${")).then(|| literal.to_string())
}

/// After `import` or `export`: the offset of the quote of `from "X"`, when only names, braces,
/// commas and stars lie between.
fn from_literal(text: &str, mut i: usize) -> Option<usize> {
    let b = text.as_bytes();
    while i < b.len() {
        let c = b[i];
        if is_ident_byte(c) {
            let start = i;
            while i < b.len() && is_ident_byte(b[i]) {
                i += 1;
            }
            if &b[start..i] == b"from" {
                let q = skip_ws(b, i);
                if is_quote(b.get(q)) {
                    return Some(q);
                }
            }
        } else if c.is_ascii_whitespace() || matches!(c, b'{' | b'}' | b',' | b'*') {
            i += 1;
        } else {
            return None;
        }
    }
    None
}

/// The offset after the type argument `<...>` that opens at `i`, nested `<>` and `=>` inside it and
/// quoted text included; none when `i` is not a `<` or the `<` never closes.
fn skip_type_args(b: &[u8], i: usize) -> Option<usize> {
    if b.get(i) != Some(&b'<') {
        return None;
    }
    let mut depth = 0;
    let mut k = i;
    while k < b.len() {
        match b[k] {
            b'<' => depth += 1,
            b'=' if b.get(k + 1) == Some(&b'>') => k += 1,
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k + 1);
                }
            }
            q @ (b'"' | b'\'' | b'`') => {
                k += 1;
                while k < b.len() && b[k] != q {
                    k += if b[k] == b'\\' { 2 } else { 1 };
                }
            }
            _ => {}
        }
        k += 1;
    }
    None
}

/// The offset of the literal after `(` at `at`, past a type argument `<...>` before the `(`.
fn call_literal(text: &str, at: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut j = skip_ws(b, at);
    if let Some(after) = skip_type_args(b, j) {
        j = skip_ws(b, after);
    }
    (b.get(j) == Some(&b'(')).then(|| skip_ws(b, j + 1))
}

/// The offsets of the patterns of `import.meta.glob("p")` / `import.meta.glob(["p", "q"])` (and
/// `globEager`, and either written with a type argument, `import.meta.glob<string>("p")`) when
/// `import` ends at `i`; none for any other text.
fn glob_patterns(text: &str, i: usize) -> Vec<usize> {
    let b = text.as_bytes();
    let j = skip_ws(b, i);
    if !text[j..].starts_with(".meta.glob") {
        return Vec::new();
    }
    let mut k = j + ".meta.glob".len();
    while k < b.len() && is_ident_byte(b[k]) {
        k += 1;
    }
    let Some(mut at) = call_literal(text, k) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if b.get(at) == Some(&b'[') {
        at = skip_ws(b, at + 1);
        while is_quote(b.get(at)) {
            out.push(at);
            let Some(literal) = literal_at(text, at) else {
                break;
            };
            at = skip_ws(b, at + literal.len() + 2);
            if b.get(at) != Some(&b',') {
                break;
            }
            at = skip_ws(b, at + 1);
        }
    } else if is_quote(b.get(at)) {
        out.push(at);
    }
    out
}

/// Every module specifier of `text` (comments already blanked) with its 1-based line.
pub(super) fn module_specifiers(text: &str) -> Vec<(usize, String)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    for i in 0..b.len() {
        if !is_ident_byte(b[i]) || (i > 0 && is_ident_byte(b[i - 1])) {
            continue;
        }
        let globs = if keyword_at(b, i, "import") {
            glob_patterns(text, i + 6)
        } else {
            Vec::new()
        };
        let at = if keyword_at(b, i, "import") {
            let j = skip_ws(b, i + 6);
            match b.get(j) {
                Some(b'(') => call_literal(text, i + 6),
                Some(b'"' | b'\'' | b'`') => Some(j),
                Some(b'.') => None,
                _ => from_literal(text, j),
            }
        } else if keyword_at(b, i, "export") {
            from_literal(text, i + 6)
        } else if keyword_at(b, i, "require") {
            call_literal(text, i + 7)
        } else {
            None
        };
        for at in at.into_iter().chain(globs) {
            if let Some(spec) = literal_at(text, at) {
                out.push((line_of(text, at), spec));
            }
        }
    }
    out
}

/// The target of every `@import` of `css` (`"x"`, `'x'`, `url("x")`, `url(x)`) with its 1-based
/// line.
pub(super) fn css_imports(css: &str) -> Vec<(usize, String)> {
    let text = strip_block_comments(css);
    let lower = text.to_ascii_lowercase();
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("@import") {
        let at = from + rel;
        from = at + "@import".len();
        let mut k = skip_ws(b, from);
        let url = lower[k..].starts_with("url(");
        if url {
            k = skip_ws(b, k + "url(".len());
        }
        if is_quote(b.get(k)) {
            if let Some(spec) = literal_at(&text, k) {
                out.push((line_of(&text, k), spec));
            }
        } else if url {
            let end = text[k..]
                .find([')', ';', '\n'])
                .map_or(text.len(), |e| k + e);
            out.push((line_of(&text, k), text[k..end].trim().to_string()));
        }
    }
    out
}

/// `html` with its `<!-- -->` comments blanked, newlines kept.
fn blank_html_comments(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let end = rest[start..]
            .find("-->")
            .map_or(rest.len(), |e| start + e + 3);
        out.extend(
            rest[start..end]
                .chars()
                .map(|c| if c == '\n' { '\n' } else { ' ' }),
        );
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The value of attribute `name` in the tag `text[start..end]` (`lower` is `text` in lower case)
/// with the offset where it starts.
fn attr_value(
    text: &str,
    lower: &str,
    start: usize,
    end: usize,
    name: &str,
) -> Option<(usize, String)> {
    let b = lower.as_bytes();
    let mut from = start;
    while let Some(rel) = lower[from..end].find(name) {
        let at = from + rel;
        from = at + name.len();
        if !b[at - 1].is_ascii_whitespace() {
            continue;
        }
        let eq = skip_ws(b, at + name.len());
        if b.get(eq) != Some(&b'=') {
            continue;
        }
        let k = skip_ws(b, eq + 1);
        let (vs, ve) = match b.get(k) {
            Some(&q) if q == b'"' || q == b'\'' => {
                (k + 1, text[k + 1..end].find(q as char).map(|e| k + 1 + e)?)
            }
            _ => (
                k,
                text[k..end]
                    .find(char::is_whitespace)
                    .map_or(end, |e| k + e),
            ),
        };
        return Some((vs, text[vs..ve].to_string()));
    }
    None
}

/// The references of the page: `<script src>` and `<link href>`, and the specifiers inside an
/// inline `<script>`, each with its line.
pub(super) fn html_refs(html: &str) -> Vec<(usize, String)> {
    let text = blank_html_comments(html);
    let lower = text.to_ascii_lowercase();
    let is_tag = |at: usize, name: &str| {
        lower[at + 1..].starts_with(name)
            && lower[at + 1 + name.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == '>' || c == '/')
    };
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = lower[pos..].find('<') {
        let start = pos + rel;
        pos = start + 1;
        let (script, attr) = if is_tag(start, "script") {
            (true, "src")
        } else if is_tag(start, "link") {
            (false, "href")
        } else {
            continue;
        };
        let Some(gt) = lower[start..].find('>') else {
            break;
        };
        let end = start + gt;
        match attr_value(&text, &lower, start, end, attr) {
            Some((at, value)) => out.push((line_of(&text, at), value)),
            None if script => {
                let body_end = lower[end..]
                    .find("</script")
                    .map_or(text.len(), |e| end + e);
                let body = strip_ts_comments(&text[end + 1..body_end]);
                let first = line_of(&text, end + 1) - 1;
                out.extend(
                    module_specifiers(&body)
                        .into_iter()
                        .map(|(line, spec)| (first + line, spec)),
                );
            }
            None => {}
        }
    }
    out
}
