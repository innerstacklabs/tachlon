//! Writes test input in the `%T`/`%F`/`%R` text form. It is not an XER exporter.

use crate::Encoding;
pub use crate::LineEnding;
use crate::encoding::windows_1252_byte;
use std::fmt::{self, Write as _};

mod blobs;
mod names;
mod tables;

pub use names::Names;

use tables::{
    activity_codes, calendars, currency, header, predecessors, projects, resources,
    schedule_options, tasks, wbs,
};

/// A complete deterministic description of synthetic test input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    /// Seed for all generated choices.
    pub seed: u64,
    /// Number of projects; zero is treated as one.
    pub projects: usize,
    /// Number of activities in each project.
    pub activities: usize,
    /// Encoding used for the returned bytes.
    pub encoding: Encoding,
    /// Newline sequence used after every line.
    pub line_ending: LineEnding,
    /// Whether adjacent projects receive one cross-project relationship.
    pub cross_project_links: bool,
    /// How work packages and activities are named.
    pub names: Names,
}

impl Spec {
    /// Returns a one-project specification with 40 activities.
    #[must_use]
    pub fn small(seed: u64) -> Spec {
        Spec::with_activities(seed, 40)
    }

    /// Returns a one-project UTF-8, LF specification with `activities` activities.
    #[must_use]
    pub fn with_activities(seed: u64, activities: usize) -> Spec {
        Spec {
            seed,
            projects: 1,
            activities,
            encoding: Encoding::Utf8,
            line_ending: LineEnding::Lf,
            cross_project_links: false,
            names: Names::Numbered,
        }
    }

    /// Computes row counts without generating the file.
    #[must_use]
    pub fn counts(&self) -> Counts {
        let projects = self.projects.max(1);
        let activities = projects.saturating_mul(self.activities);
        let children = (self.activities / 20).clamp(1, 200);
        let links_per_project = self
            .activities
            .saturating_sub(1)
            .saturating_add(self.activities.saturating_sub(1) / 7);
        let cross = usize::from(self.cross_project_links && projects >= 2 && self.activities != 0)
            .saturating_mul(projects.saturating_sub(1));
        Counts {
            projects,
            activities,
            wbs: projects.saturating_mul(children.saturating_add(1)),
            links: projects
                .saturating_mul(links_per_project)
                .saturating_add(cross),
            calendars: projects.saturating_add(2),
        }
    }
}

/// Row counts for the five typed tables produced by [`write()`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    /// Rows in `PROJECT`.
    pub projects: usize,
    /// Rows in `TASK` across all projects.
    pub activities: usize,
    /// Rows in `PROJWBS`.
    pub wbs: usize,
    /// Rows in `TASKPRED`.
    pub links: usize,
    /// Rows in `CALENDAR`.
    pub calendars: usize,
}

/// Generates deterministic XER-shaped test input.
///
/// Numeric identifiers are intended for specifications of up to ten million activities per
/// project. Larger specifications remain panic-free but may wrap identifiers.
#[must_use]
pub fn write(spec: &Spec) -> Vec<u8> {
    let counts = spec.counts();
    let estimate = counts
        .activities
        .saturating_mul(950)
        .saturating_add(counts.wbs.saturating_mul(300))
        .saturating_add(counts.links.saturating_mul(100))
        .min(64 * 1024 * 1024);
    let mut out = Output::new(spec.encoding, spec.line_ending, estimate);
    let mut rng = SplitMix64(spec.seed);
    header(&mut out, spec.seed);
    currency(&mut out);
    projects(&mut out, &counts);
    calendars(&mut out, &counts);
    schedule_options(&mut out, &counts);
    wbs(&mut out, &counts, spec.activities, spec.names);
    tasks(&mut out, &counts, spec.activities, spec.names, &mut rng);
    predecessors(&mut out, &counts, spec, &mut rng);
    resources(&mut out, &counts, spec.activities);
    activity_codes(&mut out, &counts, spec.activities);
    out.line("%E");
    out.bytes
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the remainder is below upper, which came from a usize"
    )]
    fn below(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            // 64-bit arithmetic, so a seed gives the same file on native and wasm32.
            (self.next() % upper as u64) as usize
        }
    }
}

struct Output {
    bytes: Vec<u8>,
    encoding: Encoding,
    ending: LineEnding,
}

impl Output {
    fn new(encoding: Encoding, ending: LineEnding, capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            encoding,
            ending,
        }
    }

    fn raw(&mut self, text: &str) {
        match self.encoding {
            Encoding::Utf8 => self.bytes.extend_from_slice(text.as_bytes()),
            Encoding::Windows1252 => {
                for character in text.chars() {
                    self.bytes
                        .push(windows_1252_byte(character).unwrap_or(b'?'));
                }
            }
        }
    }

    fn formatted(&mut self, arguments: fmt::Arguments<'_>) {
        let result = self.write_fmt(arguments);
        debug_assert!(result.is_ok());
    }

    fn number(&mut self, value: impl fmt::Display) {
        self.formatted(format_args!("{value}"));
    }
    fn optional_number(&mut self, value: Option<u32>) {
        if let Some(value) = value {
            self.number(value);
        }
    }
    fn end_line(&mut self) {
        self.bytes.extend_from_slice(self.ending.as_bytes());
    }
    fn line(&mut self, text: &str) {
        self.raw(text);
        self.end_line();
    }
    fn table(&mut self, name: &str, fields: &str) {
        self.formatted(format_args!("%T\t{name}"));
        self.end_line();
        self.line(fields);
    }
    fn row(&mut self, fields: &str, mut value: impl FnMut(&str, &mut Self)) {
        self.raw("%R");
        for field in fields.split('\t').skip(1) {
            self.bytes.push(b'\t');
            value(field, self);
        }
        self.end_line();
    }
}

impl fmt::Write for Output {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.raw(text);
        Ok(())
    }
}
