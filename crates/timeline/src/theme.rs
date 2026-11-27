//! Timeline colour themes and their JSON interchange format.

use std::fmt;

mod color;
use color::{format_color, parse_color};

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> [f32; 4] {
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

/// A token colour: `(r, g, b)` is opaque, `(r, g, b, a)` carries alpha.
macro_rules! color {
    ($r:literal, $g:literal, $b:literal) => {
        rgba($r, $g, $b, 255)
    };
    ($r:literal, $g:literal, $b:literal, $a:literal) => {
        rgba($r, $g, $b, $a)
    };
}

macro_rules! theme_tokens {
    ($macro:ident) => {
        $macro! {
            /// Chart background.
            bg => ("bg", (0xf2, 0xf5, 0xf9), (0x0e, 0x16, 0x30));
            /// Raised surface background.
            surface => ("surface", (0xe8, 0xed, 0xf4), (0x13, 0x1c, 0x3a));
            /// Secondary band background.
            band => ("band", (0xe1, 0xe7, 0xf0), (0x16, 0x20, 0x3f));
            /// Rules and separators.
            rule => ("rule", (0xc9, 0xd2, 0xe3), (0x26, 0x32, 0x4f));
            /// Primary text and drag outlines.
            text => ("text", (0x18, 0x21, 0x3b), (0xe6, 0xec, 0xf5));
            /// Muted text.
            muted => ("muted", (0x4f, 0x5b, 0x80), (0x9a, 0xa6, 0xc6));
            /// Normal task bars: the progress fill, a grey.
            bar => ("bar", (0x6f, 0x86, 0xc9), (0x4a, 0x5f, 0xa8));
            /// Normal task bar edges.
            bar_edge => ("bar-edge", (0x3a, 0x4c, 0x8f), (0x8f, 0xa3, 0xe8));
            /// Critical task bars: the progress fill.
            crit_fill => ("crit-fill", (0xe0, 0x4f, 0x47), (0xc2, 0x3a, 0x36));
            /// The critical red of text, borders and banners; the bar fill is `crit_fill`.
            crit => ("crit", (0xc4, 0x2b, 0x2b), (0xf0, 0x54, 0x4a));
            /// Edge around a critical task bar, equal to `crit_fill`.
            crit_edge => ("crit-edge", (0xa8, 0x22, 0x22), (0xd3, 0x3f, 0x36));
            /// Pastel body of a normal task bar, under the `bar` progress fill.
            bar_soft => ("bar-soft", (0xcd, 0xd5, 0xe8), (0x26, 0x32, 0x4f));
            /// Pastel body of a critical task bar, under the `crit` progress fill.
            crit_soft => ("crit-soft", (0xf0, 0xbc, 0xb7), (0x5a, 0x24, 0x30));
            /// A task name drawn inside its bar.
            bar_ink => ("bar-ink", (0x0e, 0x14, 0x26), (0xf2, 0xf5, 0xf9));
            /// Critical-task text.
            crit_text => ("crit-text", (0xa8, 0x22, 0x22), (0xff, 0x7d, 0x73));
            /// Summary task bars.
            sum => ("sum", (0x18, 0x21, 0x3b), (0xdb, 0xe3, 0xf2));
            /// Selection fill.
            sel => ("sel", (0xd9, 0xe2, 0xf6), (0x1c, 0x2d, 0x5c));
            /// Selection and highlighted-path edges.
            sel_edge => ("sel-edge", (0x1b, 0x5b, 0xbd), (0x5a, 0xa9, 0xff));
            /// Hovered row fill.
            hov => ("hov", (0xe6, 0xec, 0xf6), (0x17, 0x21, 0x3f));
            /// Dependency links.
            link => ("link", (0xa3, 0xae, 0xc6), (0x34, 0x41, 0x6a));
            /// The data-date line and its axis tag.
            today => ("today", (0x17, 0x70, 0x5f), (0x3f, 0xbf, 0xa4));
            /// Minor grid lines.
            grid => ("grid", (0xe8, 0xed, 0xf4), (0x11, 0x1a, 0x36));
            /// Major grid lines.
            grid_major => ("grid-major", (0xd3, 0xda, 0xe7), (0x1c, 0x27, 0x47));
            /// Axis band.
            axis => ("axis", (0xe8, 0xed, 0xf4), (0x13, 0x1c, 0x3a));
            /// Non-working days, a translucent column tint over the row fills.
            nonwork => ("nonwork", (0x18, 0x21, 0x3b, 0x12), (0xe6, 0xec, 0xf5, 0x12));
            /// Alternating row stripe.
            stripe => ("stripe", (0xed, 0xf1, 0xf7), (0x10, 0x19, 0x34));
            /// Text drawn inside a task bar.
            bar_text => ("bar-text", (0xff, 0xff, 0xff), (0x0e, 0x16, 0x30));
            /// Progress fill of a highlighted task bar, over the accent; its alpha is part of the token.
            bar_sel_progress => ("bar-sel-progress", (0x00, 0x00, 0x00, 0x59), (0xff, 0xff, 0xff, 0x66));
            /// Outline of a dragged task held back by its predecessors, and a link a pin breaks.
            held => ("held", (0x9a, 0x6a, 0x00), (0xe0, 0xa0, 0x30));
            /// The link being dragged out when the drop would be refused.
            link_refused => ("link-refused", (0x9a, 0x6a, 0x00), (0xe0, 0xa0, 0x30));
            /// The link terminal dot beside the hovered or selected bar. The drag preview does not use it: it is `sel-edge`, or `link-refused` when the drop would be refused.
            link_terminal => ("link-terminal", (0x1b, 0x5b, 0xbd), (0x5a, 0xa9, 0xff));
            /// The comparison bar under a task bar, from the baseline start to the baseline finish.
            baseline => ("baseline", (0x6f, 0x7a, 0x9c), (0x7f, 0x8d, 0xb0));
            /// The host overlay bar, a second thin bar under the comparison bar, from the overlay start to the overlay finish.
            overlay => ("overlay", (0x8a, 0x6b, 0xb8), (0xa5, 0x8b, 0xd6));
        }
    };
}

macro_rules! define_theme {
    ($(#[$doc:meta] $field:ident => ($key:literal, ($lr:literal, $lg:literal, $lb:literal $(, $la:literal)?), ($dr:literal, $dg:literal, $db:literal $(, $da:literal)?));)*) => {
        /// The component's theming API (decision 0005).
        ///
        /// The web shell reads `--gantt-<key>` custom properties from the chart
        /// root and passes them through the wasm `set_theme` method.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Theme {
            $(#[$doc]
            pub $field: [f32; 4],)*
        }

        /// Theme token keys, in their stable JSON and CSS declaration order.
        pub const TOKENS: [&str; 33] = [$($key,)*];

        impl Theme {
            /// The built-in light theme.
            pub const fn light() -> Theme {
                Theme {
                    $($field: color!($lr, $lg, $lb $(, $la)?),)*
                }
            }

            /// The built-in dark theme.
            pub const fn dark() -> Theme {
                Theme {
                    $($field: color!($dr, $dg, $db $(, $da)?),)*
                }
            }

            /// Return the colour for a JSON/CSS token key.
            pub fn get(&self, key: &str) -> Option<[f32; 4]> {
                match key {
                    $($key => Some(self.$field),)*
                    _ => None,
                }
            }

            /// Replace the colour for a JSON/CSS token key.
            ///
            /// Returns `false` without changing the theme when `key` is unknown.
            pub fn set(&mut self, key: &str, color: [f32; 4]) -> bool {
                match key {
                    $($key => {
                        self.$field = color;
                        true
                    },)*
                    _ => false,
                }
            }
        }
    };
}

theme_tokens!(define_theme);

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

impl Theme {
    /// Serialize this theme as a flat JSON object in [`TOKENS`] order.
    ///
    /// # Panics
    ///
    /// Panics if [`TOKENS`] lists a key that [`Theme::get`] does not know, a bug in this module.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut json = String::from("{");
        for (index, key) in TOKENS.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            json.push('"');
            json.push_str(key);
            json.push_str("\":\"");
            json.push_str(&format_color(
                self.get(key).expect("TOKENS contains a known key"),
            ));
            json.push('"');
        }
        json.push('}');
        json
    }

    /// Return a copy with colours overridden by a flat JSON object.
    ///
    /// Values must be `#rgb`, `#rrggbb`, or `#rrggbbaa`.
    ///
    /// # Errors
    ///
    /// Unknown or duplicate keys, non-string values, invalid colours, and
    /// malformed JSON are errors.
    pub fn with_json(&self, json: &str) -> Result<Theme, ThemeError> {
        let mut parser = Parser::new(json);
        parser.skip_ws();
        parser.expect(b'{')?;
        parser.skip_ws();
        let mut theme = *self;
        let mut seen = Vec::new();
        if parser.take(b'}') {
            parser.finish()?;
            return Ok(theme);
        }
        loop {
            parser.skip_ws();
            let key = parser.string()?;
            if seen.iter().any(|seen| seen == &key) {
                return Err(ThemeError::key(&key, "duplicate key"));
            }
            if theme.get(&key).is_none() {
                return Err(ThemeError::key(&key, "unknown key"));
            }
            seen.push(key.clone());
            parser.skip_ws();
            parser.expect_for_key(b':', &key)?;
            parser.skip_ws();
            if parser.peek() != Some(b'"') {
                return Err(ThemeError::key(&key, "value is not a string"));
            }
            let value = parser.string()?;
            let color = parse_color(&value)
                .ok_or_else(|| ThemeError::key(&key, &format!("bad colour `{value}`")))?;
            // The key was checked above; `set` must run in release builds too.
            let known = theme.set(&key, color);
            debug_assert!(known);
            parser.skip_ws();
            if parser.take(b'}') {
                break;
            }
            parser.expect(b',')?;
        }
        parser.finish()?;
        Ok(theme)
    }
}

/// An invalid theme JSON object or colour value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError {
    message: String,
}

impl ThemeError {
    fn offset(offset: usize, message: &str) -> ThemeError {
        ThemeError {
            message: format!("theme JSON at byte {offset}: {message}"),
        }
    }

    fn key(key: &str, message: &str) -> ThemeError {
        ThemeError {
            message: format!("theme key `{key}`: {message}"),
        }
    }
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ThemeError {}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Parser<'a> {
        Parser {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn take(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), ThemeError> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(ThemeError::offset(
                self.pos,
                &format!("expected `{}`", char::from(byte)),
            ))
        }
    }

    fn expect_for_key(&mut self, byte: u8, key: &str) -> Result<(), ThemeError> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(ThemeError::key(
                key,
                &format!("expected `{}` at byte {}", char::from(byte), self.pos),
            ))
        }
    }

    fn string(&mut self) -> Result<String, ThemeError> {
        self.expect(b'"')?;
        let mut value = Vec::new();
        loop {
            let at = self.pos;
            let Some(byte) = self.peek() else {
                return Err(ThemeError::offset(self.pos, "unterminated string"));
            };
            self.pos += 1;
            match byte {
                b'"' => {
                    return String::from_utf8(value)
                        .map_err(|_| ThemeError::offset(at, "invalid UTF-8 in string"));
                }
                b'\\' => {
                    let escape_at = self.pos;
                    let Some(escaped) = self.peek() else {
                        return Err(ThemeError::offset(self.pos, "unterminated escape"));
                    };
                    self.pos += 1;
                    match escaped {
                        b'"' | b'\\' => value.push(escaped),
                        _ => {
                            return Err(ThemeError::offset(
                                escape_at,
                                "only `\\\"` and `\\\\` escapes are accepted",
                            ));
                        }
                    }
                }
                0..=0x1f => {
                    return Err(ThemeError::offset(at, "control character in string"));
                }
                _ => value.push(byte),
            }
        }
    }

    fn finish(&mut self) -> Result<(), ThemeError> {
        self.skip_ws();
        if self.pos == self.input.len() {
            Ok(())
        } else {
            Err(ThemeError::offset(self.pos, "trailing characters"))
        }
    }
}
