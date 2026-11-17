//! The month and weekday names the time axis writes.
//!
//! The axis takes them from a [`Names`] value, not from fixed arrays, so a host's own names can
//! take the place of the English ones. A name can start with a letter outside ASCII, so a cut to
//! a letter goes by character, with [`first_letter`].

/// The names of the twelve months, January first, and of the seven weekdays, Monday first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    /// The short month names.
    pub months: [String; 12],
    /// The short weekday names.
    pub weekdays: [String; 7],
    /// The day a week starts on, as an index into `weekdays`: 0 is Monday, 6 is Sunday.
    pub week_start: u8,
}

impl Default for Names {
    /// English, three letters each.
    fn default() -> Names {
        Names {
            months: [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ]
            .map(String::from),
            weekdays: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map(String::from),
            week_start: 0,
        }
    }
}

/// The first character of `name`, whole: never a cut inside a multi-byte one.
#[must_use]
pub fn first_letter(name: &str) -> &str {
    name.chars().next().map_or("", |c| &name[..c.len_utf8()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_letter_is_a_whole_character() {
        assert_eq!(first_letter("Wed"), "W");
        assert_eq!(first_letter("Écu"), "É");
        assert_eq!(first_letter("日曜"), "日");
        assert_eq!(first_letter(""), "");
    }
}
