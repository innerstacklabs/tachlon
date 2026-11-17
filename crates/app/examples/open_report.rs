//! Which of a folder of schedules open: one line per `.xer` or `.xml` file.
//!
//! Usage: `cargo run -q --release -p app --example open_report -- <folder or file>...`
//!
//! Each line is the verdict (`opened`, `opened-with-notes`, `choose-project` or `refused`), the
//! file, the format, the task, link and note counts, then the refusal sentence or the first notes.
//! The open is `app::load_bytes`, the one the page runs. Exit 0 when nothing is refused, 1 when
//! any file is, 64 on a usage error (no path, or a path that cannot be read).

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool: its report is its output"
)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn files(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::path);
        for entry in entries {
            let child = entry.path();
            if child.is_dir() || is_schedule(&child) {
                files(&child, out)?;
            }
        }
    } else {
        out.push(path.to_path_buf());
    }
    Ok(())
}

fn is_schedule(path: &Path) -> bool {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    extension.eq_ignore_ascii_case("xer") || extension.eq_ignore_ascii_case("xml")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut found = Vec::new();
    if args.is_empty()
        || args
            .iter()
            .any(|arg| files(Path::new(arg), &mut found).is_err())
    {
        eprintln!("usage: open_report <folder or file>...  (every path must be readable)");
        return ExitCode::from(64);
    }
    let mut refused = false;
    for path in found {
        let line = match std::fs::read(&path) {
            Ok(bytes) => app::open_report::open_report(&bytes),
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::from(64);
            }
        };
        refused |= line.verdict == "refused";
        println!("{} {}", path.display(), line);
    }
    ExitCode::from(u8::from(refused))
}
