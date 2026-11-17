# xer

A reader and table writer for Primavera P6 XER files, written in Rust. It has no dependencies and runs in the browser (it builds for `wasm32-unknown-unknown`).

**This reader is best effort.** XER is Oracle's export format and has no public specification. The code is written against the format as observed in public sample files from P6 8.2, 8.4 and 20.12, so it can fail or misread a file from another version, another locale or another tool. It is written not to panic on a malformed or truncated file: it returns an `Error` or a `Warning`, and it reports what it did not read.

## What it reads

- **Table layer:** `xer::parse(&[u8])` detects the encoding (a UTF-8 BOM, then valid UTF-8, then Windows-1252), accepts LF, CRLF and CR line ends, and returns the `ERMHDR` line and every `%T` table with its field names and rows. A long row, a missing `%E` and text after `%E` are warnings; an unknown line or a repeated table with a different field list is an error.
- **Typed layer:** `xer::read(&Xer)` returns owned rows for `PROJECT`, `PROJWBS`, `TASK`, `TASKPRED`, `SCHEDOPTIONS` and `CALENDAR`, and lists every other table with its row count (`Data::not_read`). It parses the `clndr_data` calendar blob into a weekly schedule and exception days.
- **Writing:** `Xer::write` writes the parsed tables back as XER text in the file's own encoding and line ending, and `xer::Writer` builds a file cell by cell (text, or raw bytes copied untouched). It refuses a character the encoding cannot hold, and a cell holding a tab or a line break. It writes tables only: it knows the text form, not what the tables mean, and it is as best effort as the reader. `Xer::line_ending` reports the line ending the parser found.
- **Not read:** resources, assignments, activity codes, UDFs, baselines and every other table.

## Test input: the `synth` feature

`xer::synth` (cargo feature `synth`) writes a file in the `%T`/`%F`/`%R` text form from a seed, for tests, benchmarks and demos: any size, one or more projects, UTF-8 or Windows-1252, LF or CRLF. **`synth` is not an XER exporter**; its files are for this reader's tests and are not meant to be opened in P6.

```
cargo test -p xer --features synth
XER_CORPUS_DIR=<dir> cargo test -p xer --features synth --test xer_corpus -- --nocapture
```

The second command parses and reads every `.xer` file under `<dir>` and prints the version, encoding, tables and warnings of each. The test fails if the directory cannot be listed, if it holds no `.xer` file, or if the library panics on a file. It does not fail on a parse error or a read error: those appear in the printout. No sample file is committed to this repository.

## A file that fails

Open an issue (the link is in "Where to start" of the [top README](../../README.md)) with the error or warning text and the P6 version from the first line of the file (`ERMHDR`). **Do not attach a schedule that belongs to someone else, or one you may not share.** If you cannot share the file, send the first line, the `%T` table names and the `%F` field line of the table that failed; none of that holds project data.

## Licence

`MIT`: see `LICENSE-MIT` in this directory. The whole repository is under the same license.
