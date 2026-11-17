use crate::data::DataMetadata;
use crate::value::Parsed;
use crate::{Data, Encoding, Error, NotRead, Table, Warning, Xer};

mod activity_codes;
mod tables;

use activity_codes::{read_actv_codes, read_actv_types, read_task_actvs};
use tables::{
    read_calendars, read_header, read_preds, read_projects, read_sched_options, read_tasks,
    read_wbs,
};

type ValueWarnings = Vec<(usize, usize, Warning)>;

/// Reads the supported XER tables into typed rows.
///
/// # Errors
///
/// Returns [`Error::BadValue`] when a key field is empty or invalid, [`Error::MissingColumn`]
/// when a table has no column for a key field, and [`Error::CalendarTooDeep`] when a calendar
/// blob nests more than 64 nodes.
pub fn read(xer: &Xer<'_>) -> Result<Data, Error> {
    let mut value_warnings = Vec::new();
    let header = read_header(xer, &mut value_warnings);
    let project_rows = read_projects(xer, &mut value_warnings)?;
    let wbs_rows = read_wbs(xer, &mut value_warnings)?;
    let task_rows = read_tasks(xer, &mut value_warnings)?;
    let pred_rows = read_preds(xer, &mut value_warnings)?;
    let sched_options_rows = read_sched_options(xer, &mut value_warnings);
    let calendar_rows = read_calendars(xer, &mut value_warnings)?;
    let actv_type_rows = read_actv_types(xer, &mut value_warnings)?;
    let actv_code_rows = read_actv_codes(xer, &mut value_warnings)?;
    let task_actv_rows = read_task_actvs(xer, &mut value_warnings)?;
    value_warnings.sort_by_key(|(line, column, _)| (*line, *column));

    let mut warnings = xer.warnings().to_vec();
    warnings.extend(value_warnings.into_iter().map(|(_, _, warning)| warning));
    let not_read = xer
        .tables()
        .iter()
        .filter(|table| !is_read_table(&table.name()))
        .map(|table| NotRead {
            name: table.name().into_owned(),
            rows: table.len(),
        })
        .collect();

    Ok(Data::new(
        header,
        project_rows,
        wbs_rows,
        task_rows,
        pred_rows,
        sched_options_rows,
        calendar_rows,
        actv_type_rows,
        actv_code_rows,
        task_actv_rows,
        DataMetadata {
            not_read,
            encoding: xer.encoding(),
            warnings,
        },
    ))
}

fn required_column(table: &Table<'_>, field: &str) -> Result<usize, Error> {
    table.column(field).ok_or_else(|| Error::MissingColumn {
        table: table.name().into_owned(),
        field: field.to_owned(),
        line: table.line(),
    })
}

struct Cells<'a, 'w>(
    &'a [&'a [u8]],
    Encoding,
    &'static str,
    usize,
    &'w mut ValueWarnings,
);

impl<'a> Cells<'a, '_> {
    fn key<T>(
        &self,
        column: usize,
        field: &str,
        parser: fn(&str) -> Parsed<T>,
    ) -> Result<T, Error> {
        let value = self.decode(column);
        match parser(&value) {
            Parsed::Value(parsed) => Ok(parsed),
            Parsed::Empty | Parsed::Bad => Err(Error::BadValue {
                table: self.2.to_owned(),
                field: field.to_owned(),
                line: self.3,
                value: short(&value),
            }),
        }
    }

    fn optional<T>(
        &mut self,
        column: Option<usize>,
        field: &str,
        parser: fn(&str) -> Parsed<T>,
    ) -> Option<T> {
        let value = self.decode(column?);
        match parser(&value) {
            Parsed::Empty => None,
            Parsed::Value(parsed) => Some(parsed),
            Parsed::Bad => {
                self.4
                    .push((self.3, column?, bad_warning(self.2, field, self.3, &value)));
                None
            }
        }
    }

    fn text(&self, column: Option<usize>) -> Option<String> {
        let raw = self.0.get(column?).copied().unwrap_or(&[]);
        (!raw.is_empty()).then(|| self.1.decode(raw).into_owned())
    }

    fn decode(&self, column: usize) -> std::borrow::Cow<'a, str> {
        self.1.decode(self.0.get(column).copied().unwrap_or(&[]))
    }
}

fn header_text(raw: Option<&[u8]>, encoding: Encoding) -> Option<String> {
    raw.filter(|value| !value.is_empty())
        .map(|value| encoding.decode(value).into_owned())
}

fn bad_warning(table: &str, field: &str, line: usize, value: &str) -> Warning {
    Warning::BadValue {
        table: table.to_owned(),
        field: field.to_owned(),
        line,
        value: short(value),
    }
}

fn short(value: &str) -> String {
    value.chars().take(40).collect()
}

fn is_read_table(name: &str) -> bool {
    [
        "PROJECT",
        "PROJWBS",
        "TASK",
        "TASKPRED",
        "SCHEDOPTIONS",
        "CALENDAR",
        "ACTVTYPE",
        "ACTVCODE",
        "TASKACTV",
    ]
    .contains(&name)
}
