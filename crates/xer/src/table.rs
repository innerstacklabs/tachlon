use crate::Encoding;
use std::borrow::Cow;

/// A named XER table and the rows collected from all of its occurrences.
pub struct Table<'a> {
    name: &'a [u8],
    line: usize,
    encoding: Encoding,
    fields: Vec<&'a [u8]>,
    rows: Vec<Row<'a>>,
    fields_seen: bool,
    fields_in_occurrence: bool,
}

impl<'a> Table<'a> {
    pub(crate) fn new(name: &'a [u8], line: usize, encoding: Encoding) -> Self {
        Self {
            name,
            line,
            encoding,
            fields: Vec::new(),
            rows: Vec::new(),
            fields_seen: false,
            fields_in_occurrence: false,
        }
    }

    pub(crate) fn begin_occurrence(&mut self) {
        self.fields_in_occurrence = false;
    }

    pub(crate) fn has_fields(&self) -> bool {
        self.fields_seen
    }

    pub(crate) fn has_fields_in_occurrence(&self) -> bool {
        self.fields_in_occurrence
    }

    pub(crate) fn field_bytes(&self) -> &[&'a [u8]] {
        &self.fields
    }

    pub(crate) fn set_fields(&mut self, fields: Vec<&'a [u8]>) {
        self.fields = fields;
        self.fields_seen = true;
        self.fields_in_occurrence = true;
    }

    pub(crate) fn accept_repeated_fields(&mut self) {
        self.fields_in_occurrence = true;
    }

    pub(crate) fn push_row(&mut self, row: Row<'a>) {
        self.rows.push(row);
    }

    /// Returns the decoded table name.
    #[must_use]
    pub fn name(&self) -> Cow<'a, str> {
        self.encoding.decode(self.name)
    }

    /// Returns the 1-based line number of this table's first `%T` line.
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    /// Iterates over the decoded field names in column order.
    pub fn fields(&self) -> impl Iterator<Item = Cow<'a, str>> + '_ {
        self.fields.iter().map(|field| self.encoding.decode(field))
    }

    /// Returns the number of fields in the table.
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Returns the first column whose raw bytes exactly equal `name`.
    ///
    /// Comparison is byte-exact, ASCII-compatible, case-sensitive, and does
    /// not use decoded Unicode equivalence.
    #[must_use]
    pub fn column(&self, name: &str) -> Option<usize> {
        self.fields
            .iter()
            .position(|field| *field == name.as_bytes())
    }

    /// Returns the number of rows collected for the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns whether the table contains no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns the rows collected from every occurrence of this table.
    ///
    /// Rows with too few cells read as though their missing trailing cells
    /// were empty. Cells beyond the table width are ignored by cell access.
    #[must_use]
    pub fn rows(&self) -> &[Row<'a>] {
        &self.rows
    }
}

/// One XER row, retained as a slice of the original input.
pub struct Row<'a> {
    bytes: &'a [u8],
    line: usize,
    width: usize,
    encoding: Encoding,
}

impl<'a> Row<'a> {
    pub(crate) fn new(bytes: &'a [u8], line: usize, width: usize, encoding: Encoding) -> Self {
        Self {
            bytes,
            line,
            width,
            encoding,
        }
    }

    /// Returns the row's 1-based source line number.
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    /// Returns the field count of the row's table.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns decoded cell `i`, or `None` when `i` is outside the table width.
    ///
    /// A cell missing from a short row is returned as an empty string. Extra
    /// cells in a long row are ignored because access is limited by the table
    /// width.
    #[must_use]
    pub fn cell(&self, i: usize) -> Option<Cow<'a, str>> {
        self.raw_cell(i).map(|cell| self.encoding.decode(cell))
    }

    /// Returns raw cell `i`, or `None` when `i` is outside the table width.
    ///
    /// A cell missing from a short row is returned as an empty byte slice.
    /// Extra cells in a long row are ignored.
    #[must_use]
    pub fn raw_cell(&self, i: usize) -> Option<&'a [u8]> {
        if i >= self.width {
            return None;
        }

        match self.bytes.split(|byte| *byte == b'\t').nth(i) {
            Some(cell) => Some(cell),
            None => Some(&[]),
        }
    }

    /// Iterates over raw cells up to the row's table width.
    ///
    /// Missing trailing cells are returned as empty byte slices, and cells
    /// beyond the table width are ignored.
    pub fn raw_cells(&self) -> impl Iterator<Item = &'a [u8]> + '_ {
        self.bytes
            .split(|byte| *byte == b'\t')
            .chain(std::iter::repeat(&[][..]))
            .take(self.width)
    }

    /// Looks up `name` byte-exactly in `table` and returns the decoded cell.
    ///
    /// Missing cells in short rows are returned as empty strings. Extra cells
    /// in long rows are ignored.
    #[must_use]
    pub fn get(&self, table: &Table<'_>, name: &str) -> Option<Cow<'a, str>> {
        table.column(name).and_then(|column| self.cell(column))
    }
}
