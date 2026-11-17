//! Whether a schedule file opens: the verdict, the counts and the notes `load_bytes` gives it.

use crate::{OpenError, load_bytes};

/// What opening one file came to.
pub struct OpenReport {
    /// `opened`, `opened-with-notes`, `choose-project` or `refused`.
    pub verdict: &'static str,
    /// `xer` or `mspdi`, by the same test the page uses.
    pub format: &'static str,
    pub tasks: usize,
    pub links: usize,
    /// `Loaded::unsupported`: what the open left out or changed.
    pub notes: Vec<String>,
    /// The importer's or the engine's sentence, for a refusal.
    pub refusal: Option<String>,
}

impl std::fmt::Display for OpenReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} format={} tasks={} links={} notes={}",
            self.verdict,
            self.format,
            self.tasks,
            self.links,
            self.notes.len()
        )?;
        if let Some(sentence) = &self.refusal {
            write!(f, " {sentence}")?;
        }
        for note in self.notes.iter().take(3) {
            write!(f, " | {note}")?;
        }
        Ok(())
    }
}

/// Open `bytes` the way the page does and report the outcome; a refusal is a report, never a panic.
#[must_use]
pub fn open_report(bytes: &[u8]) -> OpenReport {
    let format = if crate::is_xer(bytes) { "xer" } else { "mspdi" };
    let mut report = OpenReport {
        verdict: "opened",
        format,
        tasks: 0,
        links: 0,
        notes: Vec::new(),
        refusal: None,
    };
    match load_bytes(bytes, &import::ProjectSelection::Unspecified, &|| 0.0) {
        Ok(loaded) => {
            report.tasks = loaded.schedule.tasks().len();
            report.links = loaded.schedule.live_dependencies().count();
            if !loaded.unsupported.is_empty() {
                report.verdict = "opened-with-notes";
            }
            report.notes = loaded.unsupported;
        }
        Err(OpenError::ChooseProject(_)) => report.verdict = "choose-project",
        Err(OpenError::Message(sentence)) => {
            report.verdict = "refused";
            report.refusal = Some(sentence);
        }
    }
    report
}

#[cfg(test)]
#[path = "open_report_tests.rs"]
mod tests;
