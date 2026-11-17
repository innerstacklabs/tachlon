//! Table-layer reading and writing, and typed reading, for Primavera P6 XER files.
//!
//! This dependency-free crate detects and decodes XER text. Use [`parse`] for
//! the table layer, then [`read`] for typed rows. [`Xer::write`] and [`Writer`]
//! write tables back as XER text in a chosen encoding and line ending; the
//! crate knows the text form, not what the tables mean. It is licensed
//! under `MIT`.
//! The `synth` module writes test input and is not an XER exporter.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), warn(clippy::too_many_lines))]

mod calendar;
mod codes;
mod data;
mod encoding;
mod error;
mod read;
mod rows;
mod table;
mod value;
mod write;

#[cfg(feature = "synth")]
pub mod synth;

pub use calendar::{
    CalendarData, CalendarException, ParsedClndr, Period, Week, parse_clndr_data, write_clndr_data,
};
pub use codes::{CalendarType, ConstraintType, PredType, StatusCode, TaskType};
pub use data::{Data, NotRead, ProjectSummary};
pub use encoding::Encoding;
pub use error::{Error, Warning};
pub use read::read;
pub use rows::{
    ActvCodeRow, ActvTypeRow, CalendarRow, Header, Minutes, PredRow, ProjectRow, SchedOptionsRow,
    TaskActvRow, TaskRow, WbsRow,
};
pub use table::{Row, Table};
pub use write::{Cell, LineEnding, WriteError, Writer};

use encoding::detect;
use std::collections::HashMap;

/// A parsed XER file and its table-oriented contents.
pub struct Xer<'a> {
    encoding: Encoding,
    line_ending: LineEnding,
    byte_order_mark: bool,
    header: Row<'a>,
    tables: Vec<Table<'a>>,
    warnings: Vec<Warning>,
}

impl<'a> Xer<'a> {
    /// Returns the encoding detected for the file.
    #[must_use]
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Returns the line ending the parser split the file on.
    ///
    /// A file with no line break reads as [`LineEnding::Lf`].
    #[must_use]
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Returns whether the file began with a UTF-8 byte order mark.
    #[must_use]
    pub fn byte_order_mark(&self) -> bool {
        self.byte_order_mark
    }

    /// Returns the `ERMHDR` line as cells following the `ERMHDR` word.
    ///
    /// Cell zero is normally the XER version. An `ERMHDR` line with no cell
    /// content has width zero.
    #[must_use]
    pub fn header(&self) -> &Row<'a> {
        &self.header
    }

    /// Returns tables in first-appearance order.
    ///
    /// Repeated occurrences of the same decoded table name are represented by
    /// one table whose rows are concatenated.
    #[must_use]
    pub fn tables(&self) -> &[Table<'a>] {
        &self.tables
    }

    /// Returns the first table with the given decoded name.
    #[must_use]
    pub fn table(&self, name: &str) -> Option<&Table<'a>> {
        self.tables.iter().find(|table| table.name() == name)
    }

    /// Returns recoverable parse warnings in file order.
    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }
}

/// Parses a Primavera P6 XER byte buffer into tables.
///
/// The parser detects UTF-8 or Windows-1252, accepts LF, CRLF, and CR-only
/// line endings, and retains rows as slices of `bytes`. Short rows produce a
/// warning and expose missing trailing cells as empty strings; long rows also
/// produce a warning and ignore cells beyond the field count.
///
/// # Errors
///
/// Returns an [`Error`] when the file has no `ERMHDR` line, a line has no recognised marker,
/// a `%F` or `%R` line comes before any `%T` line, a `%R` line comes before its table's `%F`
/// line, a table occurrence has a second `%F` line, or a repeated table has a different field
/// list.
#[expect(clippy::too_many_lines, reason = "one pass builds every table")]
pub fn parse(bytes: &[u8]) -> Result<Xer<'_>, Error> {
    let (encoding, bytes_to_skip) = detect(bytes);
    let Some(content) = bytes.get(bytes_to_skip..) else {
        return Err(Error::MissingHeader);
    };
    let line_ending = LineEnding::detect(content);
    let mut lines = Lines::new(content);

    let (header_line, header_bytes) = loop {
        match lines.next() {
            Some((_, [])) => {}
            Some((line_number, line)) => match header_cells(line) {
                Some(cells) => break (line_number, cells),
                None => return Err(Error::MissingHeader),
            },
            None => return Err(Error::MissingHeader),
        }
    };
    let header_width = if header_bytes.is_empty() {
        0
    } else {
        count_tabs(header_bytes).saturating_add(1)
    };
    let header = Row::new(header_bytes, header_line, header_width, encoding);

    let mut tables: Vec<Table<'_>> = Vec::new();
    // Table name (raw bytes) -> index in `tables`, so a repeated `%T` is found without a scan.
    let mut by_name: HashMap<&[u8], usize> = HashMap::new();
    let mut warnings = Vec::new();
    let mut current_table = None;
    let mut found_end = false;

    for (line_number, line) in lines {
        if line.is_empty() {
            continue;
        }

        if found_end {
            warnings.push(Warning::AfterEnd { line: line_number });
            break;
        }

        let Some((marker, payload, has_tab)) = marker(line) else {
            let text = encoding.decode(line).chars().take(40).collect();
            return Err(Error::UnknownLine {
                line: line_number,
                text,
            });
        };

        match marker {
            Marker::Table => {
                let name = first_cell(payload);
                if let Some(index) = by_name.get(name).copied() {
                    if let Some(table) = tables.get_mut(index) {
                        table.begin_occurrence();
                    }
                    current_table = Some(index);
                } else {
                    let index = tables.len();
                    tables.push(Table::new(name, line_number, encoding));
                    by_name.insert(name, index);
                    current_table = Some(index);
                }
            }
            Marker::Fields => {
                let Some(index) = current_table else {
                    return Err(Error::LineBeforeTable {
                        line: line_number,
                        marker: 'F',
                    });
                };
                let fields = field_cells(payload, has_tab);
                let Some(table) = tables.get_mut(index) else {
                    return Err(Error::LineBeforeTable {
                        line: line_number,
                        marker: 'F',
                    });
                };

                if table.has_fields_in_occurrence() {
                    return Err(Error::FieldsRepeated {
                        table: table.name().into_owned(),
                        line: line_number,
                    });
                }
                if table.has_fields() {
                    if table.field_bytes() != fields.as_slice() {
                        return Err(Error::FieldsDiffer {
                            table: table.name().into_owned(),
                            line: line_number,
                        });
                    }
                    table.accept_repeated_fields();
                } else {
                    table.set_fields(fields);
                }
            }
            Marker::Row => {
                let Some(index) = current_table else {
                    return Err(Error::LineBeforeTable {
                        line: line_number,
                        marker: 'R',
                    });
                };
                let Some(table) = tables.get_mut(index) else {
                    return Err(Error::LineBeforeTable {
                        line: line_number,
                        marker: 'R',
                    });
                };
                if !table.has_fields_in_occurrence() {
                    return Err(Error::RowBeforeFields {
                        table: table.name().into_owned(),
                        line: line_number,
                    });
                }

                let cells = if has_tab {
                    count_tabs(payload).saturating_add(1)
                } else {
                    0
                };
                let fields = table.field_count();
                if cells > fields {
                    warnings.push(Warning::LongRow {
                        table: table.name().into_owned(),
                        line: line_number,
                        cells,
                        fields,
                    });
                }
                table.push_row(Row::new(payload, line_number, fields, encoding));
            }
            Marker::End => found_end = true,
        }
    }

    if !found_end {
        warnings.push(Warning::MissingEnd);
    }

    Ok(Xer {
        encoding,
        line_ending,
        byte_order_mark: bytes_to_skip > 0,
        header,
        tables,
        warnings,
    })
}

#[derive(Clone, Copy)]
enum Marker {
    Table,
    Fields,
    Row,
    End,
}

fn marker(line: &[u8]) -> Option<(Marker, &[u8], bool)> {
    match line {
        [b'%', b'T'] => Some((Marker::Table, &[], false)),
        [b'%', b'T', b'\t', payload @ ..] => Some((Marker::Table, payload, true)),
        [b'%', b'F'] => Some((Marker::Fields, &[], false)),
        [b'%', b'F', b'\t', payload @ ..] => Some((Marker::Fields, payload, true)),
        [b'%', b'R'] => Some((Marker::Row, &[], false)),
        [b'%', b'R', b'\t', payload @ ..] => Some((Marker::Row, payload, true)),
        [b'%', b'E'] => Some((Marker::End, &[], false)),
        [b'%', b'E', b'\t', payload @ ..] => Some((Marker::End, payload, true)),
        _ => None,
    }
}

fn header_cells(line: &[u8]) -> Option<&[u8]> {
    if line == b"ERMHDR" {
        Some(&[])
    } else {
        line.strip_prefix(b"ERMHDR\t")
    }
}

fn first_cell(bytes: &[u8]) -> &[u8] {
    match bytes.split(|byte| *byte == b'\t').next() {
        Some(cell) => cell,
        None => &[],
    }
}

fn field_cells(bytes: &[u8], has_tab: bool) -> Vec<&[u8]> {
    if has_tab {
        bytes.split(|byte| *byte == b'\t').collect()
    } else {
        Vec::new()
    }
}

#[expect(
    clippy::naive_bytecount,
    reason = "the crate takes no dependencies, so the bytecount crate it suggests is not available"
)]
fn count_tabs(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\t').count()
}

struct Lines<'a> {
    remaining: &'a [u8],
    delimiter: Option<u8>,
    next_line: usize,
}

impl<'a> Lines<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        let delimiter = if bytes.contains(&b'\n') {
            Some(b'\n')
        } else if bytes.contains(&b'\r') {
            Some(b'\r')
        } else {
            None
        };
        Self {
            remaining: bytes,
            delimiter,
            next_line: 1,
        }
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = (usize, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        let remaining = self.remaining;
        let mut line = if let Some(delimiter) = self.delimiter {
            if let Some(position) = remaining.iter().position(|byte| *byte == delimiter) {
                let line = remaining.get(..position)?;
                let rest_start = position.checked_add(1)?;
                self.remaining = remaining.get(rest_start..)?;
                line
            } else {
                self.remaining = &[];
                remaining
            }
        } else {
            self.remaining = &[];
            remaining
        };
        if self.delimiter == Some(b'\n')
            && let Some(without_carriage_return) = line.strip_suffix(b"\r")
        {
            line = without_carriage_return;
        }

        let line_number = self.next_line;
        self.next_line = self.next_line.saturating_add(1);
        Some((line_number, line))
    }
}
