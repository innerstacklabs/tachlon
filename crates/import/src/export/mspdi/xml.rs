//! A small XML writer: one element per line, two-space indent, text escaped, so the same
//! project always gives the same bytes. Plus the date and time texts the reader parses.

use crate::ImportError;
use crate::fields::invalid;
use crate::units::civil_from_days;

const MINUTES_PER_DAY: i64 = 1440;

pub(super) struct Xml {
    text: String,
    depth: usize,
}

impl Xml {
    pub(super) fn new() -> Xml {
        Xml {
            text: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
            depth: 0,
        }
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.text.push_str("  ");
        }
    }

    /// The `Project` root, in MS Project's namespace.
    pub(super) fn open_project(&mut self) {
        self.text
            .push_str("<Project xmlns=\"http://schemas.microsoft.com/project\">\n");
        self.depth += 1;
    }

    pub(super) fn open(&mut self, tag: &str) {
        self.indent();
        self.text.push('<');
        self.text.push_str(tag);
        self.text.push_str(">\n");
        self.depth += 1;
    }

    pub(super) fn close(&mut self, tag: &str) {
        self.depth = self.depth.saturating_sub(1);
        self.indent();
        self.text.push_str("</");
        self.text.push_str(tag);
        self.text.push_str(">\n");
    }

    /// `<tag>value</tag>` with the value escaped.
    pub(super) fn leaf(&mut self, tag: &str, value: &str) {
        self.indent();
        self.text.push('<');
        self.text.push_str(tag);
        self.text.push('>');
        escape_into(&mut self.text, value);
        self.text.push_str("</");
        self.text.push_str(tag);
        self.text.push_str(">\n");
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.text.into_bytes()
    }
}

fn escape_into(out: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            other => out.push(other),
        }
    }
}

/// `text` without the characters XML 1.0 cannot hold, and whether any were removed.
pub(super) fn clean(text: &str) -> (String, bool) {
    let allowed = |character: char| {
        matches!(character, '\t' | '\n' | '\r')
            || (character >= ' ' && character != '\u{fffe}' && character != '\u{ffff}')
    };
    let kept: String = text
        .chars()
        .filter(|character| allowed(*character))
        .collect();
    let changed = kept.len() != text.len();
    (kept, changed)
}

/// The date `YYYY-MM-DD` of a day count since 1970-01-01, a four-digit year only.
pub(super) fn date_text(day: i64, element: &str) -> Result<String, ImportError> {
    let (year, month, day_of_month) = civil_from_days(day);
    if !(0..=9999).contains(&year) {
        return Err(invalid(element, &format!("day {day} since 1970-01-01")));
    }
    Ok(format!("{year:04}-{month:02}-{day_of_month:02}"))
}

/// An instant from the origin as `YYYY-MM-DDTHH:MM:00`, the wall-clock time the reader reads
/// back as the same instant.
pub(super) fn stamp(origin_day: i64, instant: i64, element: &str) -> Result<String, ImportError> {
    let day = origin_day
        .checked_add(instant.div_euclid(MINUTES_PER_DAY))
        .ok_or_else(|| invalid(element, &instant.to_string()))?;
    let minute = instant.rem_euclid(MINUTES_PER_DAY);
    Ok(format!(
        "{}T{:02}:{:02}:00",
        date_text(day, element)?,
        minute / 60,
        minute % 60
    ))
}

/// A minute of the day, 0 to 1440, as `HH:MM:00`; the end of the day is `00:00:00`, which the
/// reader takes as 24:00.
pub(super) fn time_text(minute: u16) -> String {
    let minute = minute % 1440;
    format!("{:02}:{:02}:00", minute / 60, minute % 60)
}
