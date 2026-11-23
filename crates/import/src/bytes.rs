use crate::{ImportError, Imported, ProjectSelection, not_a_schedule, parse_mspdi, parse_xer};

/// Whether `bytes` is a Primavera P6 XER file: it starts with `ERMHDR`, after
/// an optional UTF-8 byte order mark. The file's content decides, never its name.
#[must_use]
pub fn is_xer(bytes: &[u8]) -> bool {
    bytes
        .strip_prefix(b"\xEF\xBB\xBF")
        .unwrap_or(bytes)
        .starts_with(b"ERMHDR")
}

/// The first eight bytes of an OLE2 compound file, the container of an MS Project binary file.
const OLE2_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The first byte after an optional UTF-8 byte order mark and ASCII white space.
fn first_content_byte(bytes: &[u8]) -> Option<u8> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

/// Read a file's bytes with the importer its content selects.
///
/// An XER file opens the projects `selection` names. Anything else is MS Project XML,
/// decoded as UTF-8 exactly as the browser's `Response.text()` did before the
/// shell passed bytes: a leading byte order mark is dropped and an invalid
/// byte becomes U+FFFD, and `selection` does not apply. A file that starts with the OLE2
/// signature is an MS Project binary file: it returns [`ImportError::NotXml`]. A file that
/// starts with `%PDF` or `PK\x03\x04` (a PDF or a ZIP archive), starts with a UTF-16 byte order mark,
/// or whose first byte after a UTF-8 byte order mark and white space is not `<`, returns [`ImportError::NotASchedule`].
///
/// # Errors
///
/// Returns the [`ImportError`] of the importer the content selects: the document is
/// malformed, a value is out of range, or `selection` does not name a project of the file.
pub fn parse_bytes(bytes: &[u8], selection: &ProjectSelection) -> Result<Imported, ImportError> {
    if is_xer(bytes) {
        parse_xer(bytes, selection)
    } else if bytes.starts_with(&OLE2_SIGNATURE) {
        Err(ImportError::NotXml)
    } else if bytes.starts_with(b"%PDF") {
        Err(not_a_schedule("a PDF file"))
    } else if bytes.starts_with(b"PK\x03\x04") {
        Err(not_a_schedule(
            "a ZIP archive (an .xlsx file, or a zipped schedule: unzip it first)",
        ))
    } else if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        Err(not_a_schedule(
            "UTF-16 text (save it as UTF-8 and open it again)",
        ))
    } else if first_content_byte(bytes) != Some(b'<') {
        Err(not_a_schedule(
            "a file that is not XML and does not start with ERMHDR (the first line of a P6 XER file)",
        ))
    } else {
        let text = String::from_utf8_lossy(bytes);
        parse_mspdi(text.strip_prefix('\u{feff}').unwrap_or(&text))
    }
}
