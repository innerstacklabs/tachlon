//! Helpers for the `open*.rs` tests: load a synthetic XER, rewrite cells of its generated text,
//! and convert between minutes and `YYYY-MM-DD HH:MM` stamps. Each test binary uses part of it.

use app::{Loaded, OpenError, load_bytes};
use import::ProjectSelection;
use xer::synth::{Spec, write};

pub const MINUTES_PER_DAY: i64 = 1_440;

pub fn load(bytes: &[u8], selection: &ProjectSelection) -> Result<Loaded, OpenError> {
    load_bytes(bytes, selection, &|| 0.0)
}

pub fn loaded(bytes: &[u8], selection: &ProjectSelection) -> Loaded {
    load(bytes, selection).expect("the fixture should load")
}

/// Which project a synthetic id belongs to (`Spec` numbers project ids from 1000).
pub const FIRST_PROJECT: u32 = 1_000;

/// Rewrite one field of the generated text. `change(row, old)` returns the new value for row
/// `row` (0-based, in file order) of `table`, or `None` to keep the old one.
pub fn rewrite(
    text: &str,
    table: &str,
    field: &str,
    mut change: impl FnMut(usize, &str) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_table = false;
    let mut column = None;
    let mut row = 0;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches('\n');
        let mut cells: Vec<&str> = body.split('\t').collect();
        match cells[0] {
            "%T" => {
                in_table = cells.get(1) == Some(&table);
                column = None;
                row = 0;
            }
            "%F" if in_table => {
                column = cells.iter().position(|name| *name == field);
                assert!(column.is_some(), "{table} has no {field} column");
            }
            "%R" if in_table => {
                let column = column.expect("%F precedes %R");
                let new = change(row, cells[column]);
                row += 1;
                if let Some(new) = new {
                    cells[column] = &new;
                    out.push_str(&cells.join("\t"));
                    out.push('\n');
                    continue;
                }
            }
            _ => {}
        }
        out.push_str(line);
    }
    out
}

pub fn text(spec: &Spec) -> String {
    String::from_utf8(write(spec)).expect("the default encoding is UTF-8")
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `YYYY-MM-DD HH:MM` to minutes since 1970-01-01 00:00.
pub fn minutes_of(stamp: &str) -> i64 {
    let (date, time) = stamp.split_once(' ').expect("date and time");
    let d: Vec<i64> = date
        .split('-')
        .map(|n| n.parse().expect("number"))
        .collect();
    let t: Vec<i64> = time
        .split(':')
        .map(|n| n.parse().expect("number"))
        .collect();
    days_from_civil(d[0], d[1], d[2]) * MINUTES_PER_DAY + t[0] * 60 + t[1]
}

/// Minutes since 1970-01-01 00:00 to `YYYY-MM-DD HH:MM`.
pub fn stamp_of(minutes: i64) -> String {
    let days = minutes.div_euclid(MINUTES_PER_DAY);
    let rest = minutes.rem_euclid(MINUTES_PER_DAY);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rest / 60,
        rest % 60
    )
}

pub fn three_projects() -> Spec {
    Spec {
        projects: 3,
        ..Spec::small(9)
    }
}
