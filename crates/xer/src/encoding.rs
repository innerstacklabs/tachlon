use std::borrow::Cow;

const WINDOWS_1252: [char; 32] = [
    '\u{20ac}', '\u{0081}', '\u{201a}', '\u{0192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02c6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008d}', '\u{017d}', '\u{008f}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02dc}', '\u{2122}', '\u{0161}', '\u{203a}', '\u{0153}', '\u{009d}', '\u{017e}', '\u{0178}',
];

/// The character encoding detected for an XER file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8, decoded with replacement characters for invalid byte sequences.
    Utf8,
    /// Windows-1252 as defined by the WHATWG encoding standard.
    Windows1252,
}

impl Encoding {
    /// Returns `"UTF-8"` or `"Windows-1252"`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Windows1252 => "Windows-1252",
        }
    }

    /// Encodes text, borrowing the input when UTF-8 or ASCII needs no conversion.
    ///
    /// Windows-1252 is the reverse of [`Encoding::decode`]. Text holding a character the
    /// encoding cannot represent is refused with the first such character.
    ///
    /// # Errors
    ///
    /// Returns the first character the encoding cannot represent.
    pub fn encode(self, text: &str) -> Result<Cow<'_, [u8]>, char> {
        match self {
            Self::Utf8 => Ok(Cow::Borrowed(text.as_bytes())),
            Self::Windows1252 if text.is_ascii() => Ok(Cow::Borrowed(text.as_bytes())),
            Self::Windows1252 => {
                let mut encoded = Vec::with_capacity(text.len());
                for character in text.chars() {
                    encoded.push(windows_1252_byte(character).ok_or(character)?);
                }
                Ok(Cow::Owned(encoded))
            }
        }
    }

    /// Decodes bytes, borrowing the input when no conversion is necessary.
    #[must_use]
    pub fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        match self {
            Self::Utf8 => String::from_utf8_lossy(bytes),
            Self::Windows1252 => match std::str::from_utf8(bytes) {
                Ok(decoded) if bytes.is_ascii() => Cow::Borrowed(decoded),
                Ok(_) | Err(_) => {
                    let mut decoded = String::new();
                    for &byte in bytes {
                        let character = match byte {
                            0x80..=0x9f => WINDOWS_1252[usize::from(byte - 0x80)],
                            _ => char::from(byte),
                        };
                        decoded.push(character);
                    }
                    Cow::Owned(decoded)
                }
            },
        }
    }
}

/// Returns the Windows-1252 byte for `character`, the reverse of the decoding table.
pub(crate) fn windows_1252_byte(character: char) -> Option<u8> {
    match u32::from(character) {
        code @ (0x00..=0x7f | 0xa0..=0xff) => u8::try_from(code).ok(),
        _ => WINDOWS_1252
            .iter()
            .position(|&special| special == character)
            .and_then(|index| u8::try_from(index).ok())
            .map(|index| index + 0x80),
    }
}

/// Detects the input encoding and returns the number of leading BOM bytes to skip.
pub(crate) fn detect(bytes: &[u8]) -> (Encoding, usize) {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        (Encoding::Utf8, 3)
    } else if std::str::from_utf8(bytes).is_ok() {
        (Encoding::Utf8, 0)
    } else {
        (Encoding::Windows1252, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Encoding, detect};
    use std::borrow::Cow;

    #[test]
    fn bom_selects_utf8_and_is_skipped() {
        assert_eq!(detect(b"\xef\xbb\xbfhello"), (Encoding::Utf8, 3));
    }

    #[test]
    fn valid_utf8_without_bom_selects_utf8() {
        assert_eq!(detect("caf\u{e9}".as_bytes()), (Encoding::Utf8, 0));
    }

    #[test]
    fn invalid_utf8_selects_windows_1252() {
        assert_eq!(detect(b"\x80"), (Encoding::Windows1252, 0));
    }

    #[test]
    fn bom_takes_precedence_over_invalid_utf8() {
        assert_eq!(detect(b"\xef\xbb\xbf\x80"), (Encoding::Utf8, 3));
    }

    #[test]
    fn empty_input_is_utf8_without_a_skip() {
        assert_eq!(detect(b""), (Encoding::Utf8, 0));
    }

    #[test]
    fn windows_1252_decodes_the_whatwg_special_range() {
        let bytes: Vec<u8> = (0x80..=0x9f).collect();
        let expected: String = [
            '\u{20ac}', '\u{0081}', '\u{201a}', '\u{0192}', '\u{201e}', '\u{2026}', '\u{2020}',
            '\u{2021}', '\u{02c6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008d}',
            '\u{017d}', '\u{008f}', '\u{0090}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}',
            '\u{2022}', '\u{2013}', '\u{2014}', '\u{02dc}', '\u{2122}', '\u{0161}', '\u{203a}',
            '\u{0153}', '\u{009d}', '\u{017e}', '\u{0178}',
        ]
        .into_iter()
        .collect();

        assert_eq!(Encoding::Windows1252.decode(&bytes), expected);
    }

    #[test]
    fn windows_1252_upper_range_equals_latin_1() {
        let bytes: Vec<u8> = (0xa0..=0xff).collect();
        let expected: String = (0xa0..=0xff).map(char::from).collect();

        assert_eq!(Encoding::Windows1252.decode(&bytes), expected);
    }

    #[test]
    fn windows_1252_borrows_ascii() {
        assert!(matches!(
            Encoding::Windows1252.decode(b"plain ASCII"),
            Cow::Borrowed("plain ASCII")
        ));
    }

    #[test]
    fn windows_1252_encoding_reverses_decoding_for_every_byte() {
        let bytes: Vec<u8> = (0..=255).collect();
        let text = Encoding::Windows1252.decode(&bytes);
        assert_eq!(Encoding::Windows1252.encode(&text).unwrap().as_ref(), bytes);
    }

    #[test]
    fn windows_1252_refuses_what_decoding_never_yields() {
        for character in ['\u{80}', '\u{9f}', '\u{100}', '\u{2192}', '\u{1f600}'] {
            assert_eq!(
                Encoding::Windows1252.encode(&character.to_string()),
                Err(character)
            );
        }
    }

    #[test]
    fn utf8_lossy_decoding_accepts_invalid_bytes() {
        assert_eq!(Encoding::Utf8.decode(b"a\xffb"), "a\u{fffd}b");
    }
}
