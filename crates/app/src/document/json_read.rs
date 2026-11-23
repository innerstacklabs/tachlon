//! A small JSON value reader for the schedule document, with no dependency (decision 0010 item 5).
//!
//! It reads `null`, booleans, integers, strings (every escape, surrogate pairs included), arrays
//! and objects with their keys in order. A fraction or an exponent is an error in version 1 ([`parse_host`]
//! reads one), nesting is capped at [`MAX_DEPTH`], and every error names the byte offset it stopped at.
//!
//! [`parse_streaming`] reads the same text but hands each element of the top-level `tasks` and
//! `links` arrays to a sink as it is read, so no tree of the whole file is ever held.

use std::borrow::Cow;

/// The deepest nesting of arrays and objects the reader accepts.
pub(super) const MAX_DEPTH: usize = 64;

/// One JSON value. A string or a key borrows from the text, unless it holds an escape.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value<'a> {
    Null,
    Bool(bool),
    Int(i64),
    /// A number with a fraction or an exponent: only [`parse_host`] reads one.
    Frac(f64),
    Str(Cow<'a, str>),
    Array(Vec<Value<'a>>),
    /// Members in the order the text gave them.
    Object(Vec<Member<'a>>),
}

impl Value<'_> {
    pub(crate) fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[Value<'_>]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The member `key` of an object.
    pub(crate) fn get(&self, key: &str) -> Option<&Value<'_>> {
        match self {
            Value::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// How the value reads in an error message: short, and never a whole document.
    pub(super) fn describe(&self) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Frac(f) => f.to_string(),
            Value::Str(s) => format!("{:?}", s.chars().take(40).collect::<String>()),
            Value::Array(_) => "an array".to_string(),
            Value::Object(_) => "an object".to_string(),
        }
    }
}

/// A member of an object: its key and its value.
pub(crate) type Member<'a> = (Cow<'a, str>, Value<'a>);

/// What [`parse_streaming`] calls for each element of a streamed array, with the members of the
/// top-level object read before the array, the array's key, the element's index, the byte offset
/// where the element ends and the element.
pub(crate) type Sink<'a> = dyn for<'v> FnMut(&[Member<'v>], &str, usize, usize, Value<'v>) + 'a;

/// The top-level arrays that are streamed, each with the keys that must be read before it. An
/// array that comes sooner, a second one under the same key and a value that is not an array are
/// read into the tree as [`parse`] reads them.
const STREAMED: [(&str, &[&str]); 2] = [
    ("tasks", &["calendars", "lag_calendar"]),
    ("links", &["calendars", "lag_calendar", "tasks"]),
];

/// The one JSON value `text` holds, or why it is not, with every array in the tree: a message
/// that starts with `byte <offset>`. The reader of a document streams instead; a host's records
/// are small, and the tests check the two refuse the same texts.
pub(crate) fn parse(text: &str) -> Result<Value<'_>, String> {
    parse_with(text, None, false)
}

/// [`parse`] for a host's records, which hand back the fractions `results` writes: a [`Value::Frac`].
pub(crate) fn parse_host(text: &str) -> Result<Value<'_>, String> {
    parse_with(text, None, true)
}

/// The one JSON value `text` holds, or why it is not: a message that starts with `byte <offset>`.
/// Each element of a streamed array goes to `sink` and is dropped, so the array is left empty in
/// the value returned.
pub(crate) fn parse_streaming<'t>(text: &'t str, sink: &mut Sink<'_>) -> Result<Value<'t>, String> {
    parse_with(text, Some(sink), false)
}

fn parse_with<'t, 's>(
    text: &'t str,
    sink: Option<&'s mut Sink<'s>>,
    fractions: bool,
) -> Result<Value<'t>, String> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        at: 0,
        sink,
        fractions,
    };
    let value = reader.value(0)?;
    reader.space();
    if reader.at != reader.bytes.len() {
        return Err(reader.fail("text after the value"));
    }
    Ok(value)
}

struct Reader<'t, 's> {
    bytes: &'t [u8],
    at: usize,
    sink: Option<&'s mut Sink<'s>>,
    fractions: bool,
}

impl<'t> Reader<'t, '_> {
    fn fail(&self, message: &str) -> String {
        format!("byte {}: {message}", self.at)
    }

    fn space(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        self.at += usize::from(found);
        found
    }

    fn word(&mut self, word: &str, value: Value<'t>) -> Result<Value<'t>, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.fail("not a JSON value"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value<'t>, String> {
        self.space();
        match self.peek() {
            None => Err(self.fail("the text ends inside a value")),
            Some(b'n') => self.word("null", Value::Null),
            Some(b't') => self.word("true", Value::Bool(true)),
            Some(b'f') => self.word("false", Value::Bool(false)),
            Some(b'"') => self.string().map(Value::Str),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b'[' | b'{') if depth >= MAX_DEPTH => {
                Err(self.fail(&format!("nested more than {MAX_DEPTH} levels deep")))
            }
            Some(b'[') => self.array(depth),
            Some(b'{') => self.object(depth),
            Some(_) => Err(self.fail("not a JSON value")),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value<'t>, String> {
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.eat(b']') {
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.space();
            if self.eat(b']') {
                return Ok(Value::Array(items));
            }
            if !self.eat(b',') {
                return Err(self.fail("expected , or ]"));
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value<'t>, String> {
        self.at += 1;
        let mut members = Vec::new();
        self.space();
        if self.eat(b'}') {
            return Ok(Value::Object(members));
        }
        loop {
            self.space();
            if self.peek() != Some(b'"') {
                return Err(self.fail("expected a key"));
            }
            let key = self.string()?;
            self.space();
            if !self.eat(b':') {
                return Err(self.fail("expected :"));
            }
            self.space();
            let value = if self.streams(depth, &key, &members) {
                self.stream(&key, &members)?;
                Value::Array(Vec::new())
            } else {
                self.value(depth + 1)?
            };
            members.push((key, value));
            self.space();
            if self.eat(b'}') {
                return Ok(Value::Object(members));
            }
            if !self.eat(b',') {
                return Err(self.fail("expected , or }"));
            }
        }
    }

    /// Whether the array at the cursor, the value of `key` in the top-level object, is streamed.
    fn streams(&self, depth: usize, key: &str, members: &[Member<'_>]) -> bool {
        let seen = |key: &str| members.iter().any(|(k, _)| k == key);
        depth == 0
            && self.sink.is_some()
            && self.peek() == Some(b'[')
            && !seen(key)
            && STREAMED
                .iter()
                .any(|(name, first)| *name == key && first.iter().all(|k| seen(k)))
    }

    /// The array at the cursor, each element handed to the sink as soon as it is read.
    fn stream(&mut self, key: &str, members: &[Member<'t>]) -> Result<(), String> {
        self.at += 1;
        self.space();
        if self.eat(b']') {
            return Ok(());
        }
        let mut index = 0;
        loop {
            let element = self.value(2)?;
            if let Some(sink) = self.sink.as_mut() {
                sink(members, key, index, self.at, element);
            }
            index += 1;
            self.space();
            if self.eat(b']') {
                return Ok(());
            }
            if !self.eat(b',') {
                return Err(self.fail("expected , or ]"));
            }
        }
    }

    fn number(&mut self) -> Result<Value<'t>, String> {
        let start = self.at;
        self.eat(b'-');
        let digits = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        if self.at == digits || (self.bytes[digits] == b'0' && self.at - digits > 1) {
            return Err(self.fail("not a number"));
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            if !self.fractions {
                return Err(self.fail("a fraction or an exponent; version 1 has integers only"));
            }
            return self.fraction(start);
        }
        std::str::from_utf8(&self.bytes[start..self.at])
            .ok()
            .and_then(|digits| digits.parse().ok())
            .map(Value::Int)
            .ok_or_else(|| self.fail("an integer that does not fit in 64 bits"))
    }

    /// The rest of the number that starts at `start`, once past its whole digits, as Rust reads a float.
    fn fraction(&mut self, start: usize) -> Result<Value<'t>, String> {
        while (self.peek()).is_some_and(|b| b.is_ascii_digit() || b".eE+-".contains(&b)) {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or_default();
        match text.parse::<f64>() {
            Ok(n) if n.is_finite() => Ok(Value::Frac(n)),
            _ => Err(self.fail("not a number")),
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.bytes.get(self.at..self.at + 4);
        let value = digits
            .and_then(|d| std::str::from_utf8(d).ok())
            .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|d| u32::from_str_radix(d, 16).ok())
            .ok_or_else(|| self.fail("\\u needs four hex digits"))?;
        self.at += 4;
        Ok(value)
    }

    /// The string at the cursor: a slice of the text, and an owned copy only once it has an escape.
    fn string(&mut self) -> Result<Cow<'t, str>, String> {
        self.at += 1;
        let bytes: &'t [u8] = self.bytes;
        let mut out = Cow::Borrowed("");
        loop {
            let run = self.at;
            while !matches!(self.peek(), None | Some(b'"' | b'\\' | 0..=0x1f)) {
                self.at += 1;
            }
            // The text is a `str` and the run stops at ASCII bytes, so it ends on a boundary.
            let piece = String::from_utf8_lossy(&bytes[run..self.at]);
            if out.is_empty() {
                out = piece;
            } else {
                out.to_mut().push_str(&piece);
            }
            match self.peek() {
                None => return Err(self.fail("the text ends inside a string")),
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    let escape = self.peek();
                    self.at += 1;
                    let out = out.to_mut();
                    match escape {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => out.push('\u{8}'),
                        Some(b'f') => out.push('\u{c}'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => out.push(self.unicode()?),
                        _ => {
                            self.at -= 1;
                            return Err(self.fail("not a string escape"));
                        }
                    }
                }
                Some(_) => return Err(self.fail("a control character in a string")),
            }
        }
    }

    /// The character after `\u`, joining a high surrogate with the `\u` low surrogate after it.
    fn unicode(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        let code = match first {
            0xD800..=0xDBFF => {
                if !self.bytes[self.at..].starts_with(b"\\u") {
                    return Err(self.fail("a high surrogate with no low surrogate after it"));
                }
                self.at += 2;
                let second = self.hex4()?;
                if !(0xDC00..=0xDFFF).contains(&second) {
                    return Err(self.fail("a high surrogate with no low surrogate after it"));
                }
                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
            }
            0xDC00..=0xDFFF => return Err(self.fail("a low surrogate with no high surrogate")),
            other => other,
        };
        char::from_u32(code).ok_or_else(|| self.fail("not a character"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_value_kind_and_keeps_key_order() {
        let value =
            parse(r#" {"b":[1,-2,0],"a":{"x":null,"y":true,"z":false},"s":"hi"} "#).unwrap();
        let Value::Object(members) = &value else {
            panic!("an object");
        };
        let keys: Vec<&str> = members.iter().map(|(k, _)| k.as_ref()).collect();
        assert_eq!(keys, ["b", "a", "s"]);
        assert_eq!(
            value.get("b").unwrap().as_array().unwrap()[1].as_int(),
            Some(-2)
        );
        assert_eq!(value.get("a").unwrap().get("y"), Some(&Value::Bool(true)));
    }

    #[test]
    fn a_string_or_key_borrows_from_the_text_unless_it_holds_an_escape() {
        let text = r#"{"plain":"abc","esc":"a\nb","k\/y":"x\"y"}"#;
        let Value::Object(members) = parse(text).unwrap() else {
            panic!("an object");
        };
        let borrows = |c: &Cow<'_, str>| matches!(c, Cow::Borrowed(_));
        let kinds: Vec<(bool, bool)> = members
            .iter()
            .map(|(k, v)| (borrows(k), matches!(v, Value::Str(s) if borrows(s))))
            .collect();
        assert_eq!(kinds, [(true, true), (true, false), (false, false)]);
        assert_eq!(members[2].0, "k/y");
        assert_eq!(members[2].1.as_str(), Some("x\"y"));
    }

    #[test]
    fn reads_escapes_and_surrogate_pairs() {
        let value = parse(r#""a\"\\\/\b\f\n\r\t\u00e9\ud83d\ude00 é""#).unwrap();
        assert_eq!(
            value.as_str(),
            Some("a\"\\/\u{8}\u{c}\n\r\t\u{e9}\u{1F600} \u{e9}")
        );
    }

    /// `Value::Frac` is built in one place, which only `parse_host` opens (a streamed read is `parse`'s, below).
    #[test]
    fn only_parse_host_reads_a_fraction() {
        let want = [Value::Frac(1.5), Value::Frac(-20.0), Value::Int(3)];
        let read = parse_host("[1.5,-2e1,3]").unwrap();
        assert_eq!(read.as_array(), Some(&want[..]));
        assert!(parse("[1.5]").is_err() && parse("[1e3]").is_err());
        assert!(
            ["1e", "1.5.2", "1e999"]
                .iter()
                .all(|t| parse_host(t).is_err())
        );
    }

    #[test]
    fn refuses_what_version_1_does_not_read_and_names_the_offset() {
        for (text, offset) in [
            ("1.5", "byte 1:"),
            ("[1e3]", "byte 2:"),
            ("01", "byte 2:"),
            ("-", "byte 1:"),
            ("99999999999999999999", "byte 20:"),
            ("[1,]", "byte 3:"),
            ("{\"a\" 1}", "byte 5:"),
            ("\"abc", "byte 4:"),
            ("\"\\ud83d\"", "byte 7:"),
            ("\"\\ude00\"", "byte 7:"),
            ("\"a\nb\"", "byte 2:"),
            ("nul", "byte 0:"),
            ("1 2", "byte 2:"),
            ("", "byte 0:"),
        ] {
            let error = parse(text).unwrap_err();
            assert!(error.starts_with(offset), "{text:?} gave {error}");
        }
    }

    #[test]
    fn caps_the_nesting_at_64_levels() {
        let nested = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
        assert!(parse(&nested(MAX_DEPTH)).is_ok());
        let error = parse(&nested(MAX_DEPTH + 1)).unwrap_err();
        assert!(error.contains("nested more than 64"), "{error}");
        let deep = "[".repeat(100_000);
        assert!(parse(&deep).is_err());
    }

    /// What the sink got, as `key[index]`, and the value `parse_streaming` returned.
    fn streamed(text: &str) -> (Vec<String>, Result<Value<'_>, String>) {
        let mut got = Vec::new();
        let value = parse_streaming(text, &mut |_, key, index, _, _| {
            got.push(format!("{key}[{index}]"));
        });
        (got, value)
    }

    #[test]
    fn hands_over_tasks_and_links_after_what_they_need_and_drops_them() {
        let (got, value) =
            streamed(r#"{"calendars":[0],"lag_calendar":1,"tasks":[1,2],"links":[3],"x":[4]}"#);
        assert_eq!(got, ["tasks[0]", "tasks[1]", "links[0]"]);
        let value = value.unwrap();
        assert_eq!(value.get("tasks"), Some(&Value::Array(vec![])));
        assert_eq!(value.get("links"), Some(&Value::Array(vec![])));
        assert_eq!(value.get("x").unwrap().as_array().unwrap().len(), 1);
    }

    #[test]
    fn keeps_an_array_in_the_tree_when_what_it_needs_has_not_come() {
        for (text, held) in [
            (
                r#"{"links":[1],"calendars":[],"lag_calendar":1,"tasks":[2]}"#,
                "links",
            ),
            (r#"{"tasks":[1],"calendars":[],"lag_calendar":1}"#, "tasks"),
            (r#"{"calendars":[],"tasks":[1],"lag_calendar":1}"#, "tasks"),
            (
                r#"{"calendars":[],"lag_calendar":1,"tasks":5,"links":[1]}"#,
                "tasks",
            ),
            (
                r#"{"calendars":[],"lag_calendar":1,"tasks":[],"tasks":[1]}"#,
                "tasks",
            ),
        ] {
            let (got, value) = streamed(text);
            assert!(
                !got.contains(&format!("{held}[0]")),
                "{text} streamed {held}"
            );
            assert!(value.unwrap().get(held).is_some(), "{text}");
        }
    }

    #[test]
    fn refuses_the_texts_it_streams_as_it_refuses_them_whole() {
        let wrap = |tasks: &str| format!(r#"{{"calendars":[],"lag_calendar":1,"tasks":{tasks}}}"#);
        let deep = format!(
            "[{}{}]",
            "[".repeat(MAX_DEPTH - 1),
            "]".repeat(MAX_DEPTH - 1)
        );
        for tasks in [
            "[1,]",
            "[1 2]",
            "[1",
            "[",
            "[1e3]",
            "[\"a",
            "[{\"a\" 1}]",
            &deep,
            "[[]",
        ] {
            let text = wrap(tasks);
            let (_, value) = streamed(&text);
            let whole = parse(&text);
            assert!(whole.is_err() == value.is_err(), "{text}");
            assert_eq!(value.err(), whole.err(), "{text}");
        }
    }
}
