//! Print the dates the engine computes for an MS Project XML file: one line per task, in file
//! order, `name<TAB>early start<TAB>early finish`, each as `YYYY-MM-DDTHH:MM` wall-clock time.
//!
//! ```sh
//! cargo run -q --release -p import --example mspdi_dates -- <file.xml>
//! ```
//!
//! The benchmark's Bryntum side compares these with the dates Bryntum's engine writes back for the
//! same schedule (`demos/bench/sides/bryntum-dates.ts`). Exit 1 when the file cannot be read or
//! scheduled, 64 without exactly one argument.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line example: its dates go to stdout and its errors to stderr"
)]

use std::fmt::Write as _;
use std::process::ExitCode;

/// `days` since 1970-01-01 as (year, month, day): the civil calendar (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// Minutes from 1970-01-01 00:00 as `YYYY-MM-DDTHH:MM`.
fn wall(minutes: i64) -> String {
    let (y, m, d) = civil(minutes.div_euclid(1440));
    let t = minutes.rem_euclid(1440);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}", t / 60, t % 60)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path] = args.as_slice() else {
        eprintln!("usage: mspdi_dates <file.xml>");
        return ExitCode::from(64);
    };
    let run = || -> Result<String, String> {
        let xml = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let mut imported = import::parse_mspdi(&xml).map_err(|e| format!("{path}: {e:?}"))?;
        let solution = imported
            .schedule
            .compute()
            .map_err(|e| format!("{path}: {e:?}"))?;
        let origin = imported.origin_day * 1440;
        let mut out = String::new();
        for (task, dates) in imported.tasks.iter().zip(&solution.dates) {
            writeln!(
                out,
                "{}\t{}\t{}",
                task.name,
                wall(origin + dates.es),
                wall(origin + dates.ef)
            )
            .expect("a String takes writes");
        }
        Ok(out)
    };
    match run() {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wall;

    #[test]
    fn wall_clock_dates() {
        assert_eq!(wall(0), "1970-01-01T00:00");
        // 2026-03-02 is day 20,514 since the epoch.
        assert_eq!(wall(20_514 * 1440 + 8 * 60), "2026-03-02T08:00");
        assert_eq!(wall(-1), "1969-12-31T23:59");
    }
}
