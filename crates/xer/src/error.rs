use std::fmt;

/// An error that prevents an XER file from being read as tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The first non-blank line is not an `ERMHDR` line, or the file is empty.
    MissingHeader,
    /// A `%F` or `%R` line appears before any `%T` line.
    LineBeforeTable {
        /// The 1-based source line number.
        line: usize,
        /// The marker character, either `F` or `R`.
        marker: char,
    },
    /// A `%R` line appears before the current table has a `%F` line.
    RowBeforeFields {
        /// The table name.
        table: String,
        /// The 1-based source line number.
        line: usize,
    },
    /// A second `%F` appears in the same occurrence of a table.
    FieldsRepeated {
        /// The table name.
        table: String,
        /// The 1-based source line number.
        line: usize,
    },
    /// A non-blank line does not have a recognised marker.
    UnknownLine {
        /// The 1-based source line number.
        line: usize,
        /// The first 40 decoded characters of the line.
        text: String,
    },
    /// A repeated table has a different field list from its first occurrence.
    FieldsDiffer {
        /// The table name.
        table: String,
        /// The 1-based source line number of the differing `%F` line.
        line: usize,
    },
    /// A key field is empty or contains an invalid value.
    BadValue {
        /// The table name.
        table: String,
        /// The field name.
        field: String,
        /// The 1-based source line number.
        line: usize,
        /// The cell text, limited to 40 characters.
        value: String,
    },
    /// A key field is absent from a table's `%F` field list.
    MissingColumn {
        /// The table name.
        table: String,
        /// The missing field name.
        field: String,
        /// The 1-based source line number of the table's first `%T` line.
        line: usize,
    },
    /// A calendar blob contains more than 64 nested nodes.
    CalendarTooDeep {
        /// The calendar identifier.
        clndr_id: u32,
        /// The 1-based source line number.
        line: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader => f.write_str("the first non-blank line is not an ERMHDR header"),
            Self::LineBeforeTable { line, marker } => {
                write!(f, "line {line}: %{marker} appears before any %T table line")
            }
            Self::RowBeforeFields { table, line } => {
                write!(
                    f,
                    "line {line}: row in table {table} appears before its %F field list"
                )
            }
            Self::FieldsRepeated { table, line } => {
                write!(f, "line {line}: table {table} repeats its %F field list")
            }
            Self::UnknownLine { line, text } => {
                write!(f, "line {line}: unrecognised line {text:?}")
            }
            Self::FieldsDiffer { table, line } => write!(
                f,
                "line {line}: table {table} has a different %F field list from its first occurrence"
            ),
            Self::BadValue {
                table,
                field,
                line,
                value,
            } => write!(
                f,
                "line {line}: table {table} field {field} has a bad value {value:?}"
            ),
            Self::MissingColumn { table, field, line } => {
                write!(f, "line {line}: table {table} is missing field {field}")
            }
            Self::CalendarTooDeep { clndr_id, line } => write!(
                f,
                "line {line}: calendar {clndr_id} clndr_data nests deeper than 64 nodes"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// A recoverable issue found while reading an XER file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// A row has more cells than its table has fields.
    LongRow {
        /// The table name.
        table: String,
        /// The 1-based source line number.
        line: usize,
        /// The number of cells in the row.
        cells: usize,
        /// The number of fields in the table.
        fields: usize,
    },
    /// The file ends without a `%E` line and may be truncated.
    MissingEnd,
    /// Non-blank lines after `%E` are ignored.
    AfterEnd {
        /// The first 1-based source line number after `%E`.
        line: usize,
    },
    /// A non-key field contains an invalid value and is read as absent.
    BadValue {
        /// The table name.
        table: String,
        /// The field name.
        field: String,
        /// The 1-based source line number.
        line: usize,
        /// The cell text, limited to 40 characters.
        value: String,
    },
    /// A calendar blob or one of its elements is malformed.
    CalendarMalformed {
        /// The calendar identifier.
        clndr_id: u32,
        /// The 1-based source line number.
        line: usize,
        /// A short description of the malformed content.
        detail: String,
    },
    /// A calendar blob contains a node that is not interpreted.
    CalendarUnknownNode {
        /// The calendar identifier.
        clndr_id: u32,
        /// The 1-based source line number.
        line: usize,
        /// The node name, limited to 40 characters.
        name: String,
    },
    /// Working periods overlap within a calendar day or exception.
    CalendarOverlap {
        /// The calendar identifier.
        clndr_id: u32,
        /// The 1-based source line number.
        line: usize,
        /// The day or exception, limited to 40 characters.
        scope: String,
    },
    /// A working period finishes before it starts.
    CalendarCrossesMidnight {
        /// The calendar identifier.
        clndr_id: u32,
        /// The 1-based source line number.
        line: usize,
        /// The day or exception, limited to 40 characters.
        scope: String,
    },
    /// A calendar identifier occurs more than once.
    CalendarDuplicate {
        /// The duplicated calendar identifier.
        clndr_id: u32,
        /// The 1-based source line number of the duplicate row.
        line: usize,
    },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LongRow {
                table,
                line,
                cells,
                fields,
            } => write!(
                f,
                "line {line}: row in table {table} has {cells} cells, its %F lists {fields}"
            ),
            Self::MissingEnd => f.write_str("the file ends without a %E line and may be truncated"),
            Self::AfterEnd { line } => {
                write!(f, "line {line}: non-blank content after %E is ignored")
            }
            Self::BadValue {
                table,
                field,
                line,
                value,
            } => write!(
                f,
                "line {line}: table {table} field {field} has a bad value {value:?}"
            ),
            Self::CalendarMalformed {
                clndr_id,
                line,
                detail,
            } => write!(
                f,
                "line {line}: calendar {clndr_id} has malformed clndr_data: {detail}"
            ),
            Self::CalendarUnknownNode {
                clndr_id,
                line,
                name,
            } => write!(
                f,
                "line {line}: calendar {clndr_id} has unknown node {name:?}"
            ),
            Self::CalendarOverlap {
                clndr_id,
                line,
                scope,
            } => write!(
                f,
                "line {line}: calendar {clndr_id} has overlapping periods in {scope}"
            ),
            Self::CalendarCrossesMidnight {
                clndr_id,
                line,
                scope,
            } => write!(
                f,
                "line {line}: calendar {clndr_id} has a period crossing midnight in {scope}"
            ),
            Self::CalendarDuplicate { clndr_id, .. } => write!(
                f,
                "calendar {clndr_id} appears more than once; the first row is used"
            ),
        }
    }
}
