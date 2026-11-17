//! What `packages/gantt/src/api.ts` declares for the JSON that crosses the WASM boundary.
//!
//! A scanner, not a TypeScript parser: it strips comments, finds `export interface Name { .. }`
//! and `export type TableRow = [..]`, and splits the body on the `;` and `,` that sit at depth 0.
//! Fail closed: a member or type it cannot read panics.

use std::collections::{BTreeMap, BTreeSet};

use crate::json_contract::captures::Kind;

/// One member of an interface.
#[derive(Debug, Clone)]
pub struct Member {
    pub optional: bool,
    /// The type as written, trimmed.
    pub ty: String,
}

pub type Members = BTreeMap<String, Member>;

pub struct TsShapes {
    /// Interface name to its members.
    pub interfaces: BTreeMap<String, Members>,
    /// `Status.lookahead`: the members of an inline object type, by `Parent.member`.
    pub inline: BTreeMap<String, Members>,
    /// The kinds each position of `TableRow` allows.
    pub table_row: Vec<BTreeSet<Kind>>,
}

pub fn read(src: &str) -> TsShapes {
    let src = strip_comments(src);
    let mut interfaces = BTreeMap::new();
    let mut inline = BTreeMap::new();
    let mut rest = src.as_str();
    while let Some(at) = rest.find("export interface ") {
        rest = &rest[at + "export interface ".len()..];
        let open = rest.find('{').expect("an interface with no body");
        let name = rest[..open].trim().to_string();
        let (body, after) = braces(&rest[open..]);
        let members = members(body);
        for (key, member) in &members {
            if member.ty.starts_with('{') {
                let (inner, _) = braces(&member.ty);
                inline.insert(format!("{name}.{key}"), self::members(inner));
            }
        }
        assert!(
            interfaces.insert(name.clone(), members).is_none(),
            "api.ts declares `interface {name}` twice"
        );
        rest = after;
    }
    TsShapes {
        interfaces,
        inline,
        table_row: table_row(&src),
    }
}

fn strip_comments(src: &str) -> String {
    let mut out = String::new();
    let mut rest = src;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("/*") {
            rest = after.split_once("*/").expect("an unclosed /* comment").1;
        } else if let Some(after) = rest.strip_prefix("//") {
            rest = after.find('\n').map_or("", |n| &after[n..]);
        } else {
            let c = rest.chars().next().expect("not empty");
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// `text` starts with `{`: the text inside the matching `}` and the text after it.
fn braces(text: &str) -> (&str, &str) {
    assert!(text.starts_with('{'), "expected `{{`, found {text:.20}");
    let mut depth = 0usize;
    for (at, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return (&text[1..at], &text[at + 1..]);
                }
            }
            _ => {}
        }
    }
    panic!("an unclosed `{{` in api.ts");
}

/// `text` split on `sep` where the nesting of `{} [] ()` is zero.
fn split_top(text: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut from) = (0i32, 0usize);
    for (at, c) in text.char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                parts.push(&text[from..at]);
                from = at + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[from..]);
    parts
        .into_iter()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

fn members(body: &str) -> Members {
    let mut out = Members::new();
    for piece in split_top(body, ';') {
        let (key, ty) = piece
            .split_once(':')
            .unwrap_or_else(|| panic!("an api.ts member with no type: {piece}"));
        let key = key.trim();
        let (key, optional) = key.strip_suffix('?').map_or((key, false), |k| (k, true));
        assert!(
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "an api.ts member key that is not an identifier: {key}"
        );
        let member = Member {
            optional,
            ty: ty.trim().to_string(),
        };
        assert!(
            out.insert(key.to_string(), member).is_none(),
            "a member {key} twice"
        );
    }
    out
}

fn table_row(src: &str) -> Vec<BTreeSet<Kind>> {
    let at = src
        .find("export type TableRow")
        .expect("api.ts declares no `export type TableRow`");
    let rest = &src[at..];
    let open = rest.find('[').expect("TableRow is not a tuple");
    let close = rest
        .find("];")
        .expect("TableRow tuple does not end in `];`");
    split_top(&rest[open + 1..close], ',')
        .into_iter()
        .map(|cell| split_top(cell, '|').into_iter().map(type_kind).collect())
        .collect()
}

fn type_kind(ty: &str) -> Kind {
    match ty {
        "number" => Kind::Number,
        "string" => Kind::String,
        "boolean" => Kind::Bool,
        "null" => Kind::Null,
        t if t.parse::<f64>().is_ok() => Kind::Number,
        t if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') => Kind::String,
        t => panic!("a TableRow cell type the reader does not know: {t}"),
    }
}
