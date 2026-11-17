//! Writes tables as XER text: the `ERMHDR` line, then `%T`, `%F` and `%R` lines, then `%E`.
//!
//! The writer knows the text form and nothing of what the tables mean. It adds a byte order
//! mark only when asked, and it refuses a cell that would break the line.

use crate::{Encoding, Xer};
use std::fmt;

/// The byte sequence that ends every line of an XER file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    /// A single line-feed byte.
    Lf,
    /// A carriage return followed by a line feed.
    CrLf,
    /// A single carriage-return byte.
    Cr,
}

impl LineEnding {
    /// Returns the bytes written after each line.
    #[must_use]
    pub fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Lf => b"\n",
            Self::CrLf => b"\r\n",
            Self::Cr => b"\r",
        }
    }

    /// Reads the ending the parser splits `content` on: the first line feed (with its
    /// carriage return, if any), else a carriage return, else [`LineEnding::Lf`].
    pub(crate) fn detect(content: &[u8]) -> Self {
        match content.iter().position(|byte| *byte == b'\n') {
            Some(position) if position > 0 && content.get(position - 1) == Some(&b'\r') => {
                Self::CrLf
            }
            None if content.contains(&b'\r') => Self::Cr,
            Some(_) | None => Self::Lf,
        }
    }
}

/// One cell to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell<'a> {
    /// Text, encoded with the writer's encoding.
    Text(&'a str),
    /// Bytes copied untouched, already in the writer's encoding.
    Raw(&'a [u8]),
}

/// Why a line was not written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// The text holds a character the encoding cannot represent.
    Unencodable {
        /// The first such character.
        character: char,
        /// The encoding that cannot hold it.
        encoding: Encoding,
    },
    /// A cell holds a tab, a carriage return or a line feed, which would split the line.
    Delimiter,
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unencodable {
                character,
                encoding,
            } => write!(f, "{character:?} cannot be written as {}", encoding.name()),
            Self::Delimiter => f.write_str("a cell holds a tab or a line break"),
        }
    }
}

impl std::error::Error for WriteError {}

/// Builds an XER file in memory, one line at a time.
///
/// A line that is refused is not written at all, so the output stays parseable.
pub struct Writer {
    bytes: Vec<u8>,
    encoding: Encoding,
    ending: LineEnding,
}

impl Writer {
    /// Starts an empty file that will use `encoding` and `ending`.
    #[must_use]
    pub fn new(encoding: Encoding, ending: LineEnding) -> Self {
        Self {
            bytes: Vec::new(),
            encoding,
            ending,
        }
    }

    /// Starts the file with a UTF-8 byte order mark, which the reader takes as the encoding.
    ///
    /// It has no effect on Windows-1252, or once a line has been written.
    #[must_use]
    pub fn byte_order_mark(mut self) -> Self {
        if self.encoding == Encoding::Utf8 && self.bytes.is_empty() {
            self.bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
        }
        self
    }

    /// Writes the `ERMHDR` line; `cells` follow the `ERMHDR` word.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Unencodable`] when a cell holds a character the encoding cannot
    /// represent, and [`WriteError::Delimiter`] when a cell holds a tab, a carriage return or a
    /// line feed.
    pub fn header(&mut self, cells: &[Cell<'_>]) -> Result<(), WriteError> {
        self.line("ERMHDR", cells)
    }

    /// Writes a `%T` line naming the table and the `%F` line listing its fields.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Unencodable`] when a cell holds a character the encoding cannot
    /// represent, and [`WriteError::Delimiter`] when a cell holds a tab, a carriage return or a
    /// line feed.
    pub fn table(&mut self, name: &str, fields: &[&str]) -> Result<(), WriteError> {
        let fields: Vec<Cell<'_>> = fields.iter().map(|field| Cell::Text(field)).collect();
        // Both lines are checked before either is kept.
        let mark = self.bytes.len();
        let written = self
            .line("%T", &[Cell::Text(name)])
            .and_then(|()| self.line("%F", &fields));
        if written.is_err() {
            self.bytes.truncate(mark);
        }
        written
    }

    /// Writes a `%R` line with one cell per column.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Unencodable`] when a cell holds a character the encoding cannot
    /// represent, and [`WriteError::Delimiter`] when a cell holds a tab, a carriage return or a
    /// line feed.
    pub fn row(&mut self, cells: &[Cell<'_>]) -> Result<(), WriteError> {
        self.line("%R", cells)
    }

    /// Writes the `%E` line and returns the file's bytes.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        self.bytes.extend_from_slice(b"%E");
        self.bytes.extend_from_slice(self.ending.as_bytes());
        self.bytes
    }

    fn line(&mut self, marker: &str, cells: &[Cell<'_>]) -> Result<(), WriteError> {
        let mut line = marker.as_bytes().to_vec();
        for cell in cells {
            line.push(b'\t');
            let bytes = match cell {
                Cell::Text(text) => {
                    self.encoding
                        .encode(text)
                        .map_err(|character| WriteError::Unencodable {
                            character,
                            encoding: self.encoding,
                        })?
                }
                Cell::Raw(bytes) => (*bytes).into(),
            };
            if bytes
                .iter()
                .any(|byte| matches!(byte, b'\t' | b'\r' | b'\n'))
            {
                return Err(WriteError::Delimiter);
            }
            line.extend_from_slice(&bytes);
        }
        self.bytes.extend_from_slice(&line);
        self.bytes.extend_from_slice(self.ending.as_bytes());
        Ok(())
    }
}

impl Xer<'_> {
    /// Writes the parsed tables back as XER text in the file's own encoding and line ending.
    ///
    /// Cells are copied as the raw bytes that were read, so they come back unchanged; a row
    /// is written at its table's width (a short row gains empty cells, a long row loses its
    /// extra ones). Repeated occurrences of a table are written as one. Table names are
    /// decoded and encoded again. A UTF-8 byte order mark is written back, so the reader picks
    /// the same encoding for bytes that are not valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Unencodable`] when a cell holds a character the encoding cannot
    /// represent, and [`WriteError::Delimiter`] when a cell holds a tab, a carriage return or a
    /// line feed.
    pub fn write(&self) -> Result<Vec<u8>, WriteError> {
        let mut writer = Writer::new(self.encoding(), self.line_ending());
        if self.byte_order_mark() {
            writer = writer.byte_order_mark();
        }
        let header: Vec<Cell<'_>> = self.header().raw_cells().map(Cell::Raw).collect();
        writer.header(&header)?;
        for table in self.tables() {
            let name = table.name();
            let fields: Vec<Cell<'_>> = table.field_bytes().iter().map(|f| Cell::Raw(f)).collect();
            writer.line("%T", &[Cell::Text(&name)])?;
            writer.line("%F", &fields)?;
            for row in table.rows() {
                let cells: Vec<Cell<'_>> = row.raw_cells().map(Cell::Raw).collect();
                writer.row(&cells)?;
            }
        }
        Ok(writer.finish())
    }
}
