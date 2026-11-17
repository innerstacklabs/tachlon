//! The schedule document (decision 0010): a schedule with its edits, names, outline, baseline and
//! the file it came from, written as one versioned JSON text. [`write()`] is the writer (gs#738),
//! [`read()`] the reader (gs#739). Gzip is the browser's job.
//!
//! Version 1 keeps live tasks only, renumbered in outline order, so a task deleted in the session
//! is not in the text and the ids a reader rebuilds follow the outline. Keys are written in a
//! fixed order with no insignificant whitespace, so writing the same session twice gives the same
//! bytes.

mod base64;
pub(crate) mod fields;
pub(crate) mod json_read;
mod read;
mod write;

pub use read::read;
pub(crate) use write::live_order;
pub use write::write;

use crate::Loaded;
use crate::baseline::Baseline;
use engine::TaskId;
use std::fmt;

/// The value of the `version` key this build writes.
pub const VERSION: u32 = 1;

/// The `format` key a document is written with.
pub const FORMAT: &str = "gantt";

/// The file format a schedule was imported from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    /// Primavera P6 XER.
    Xer,
    /// Microsoft Project MSPDI.
    Mspdi,
}

impl SourceFormat {
    /// The name the document writes for the format.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            SourceFormat::Xer => "xer",
            SourceFormat::Mspdi => "mspdi",
        }
    }
}

/// The original imported file. Nothing in `app` keeps the bytes after a load, so the caller
/// passes them in.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    /// What the bytes are.
    pub format: SourceFormat,
    /// The file as it was opened.
    pub bytes: &'a [u8],
}

/// What a document holds that a [`Session`](crate::Session) does not.
#[derive(Debug, Clone, Copy)]
pub struct Extras<'a> {
    /// The schedule's name: the opened file's name without its extension, `untitled` for a new
    /// schedule.
    pub name: &'a str,
    /// The "unsupported features" lines the open reported.
    pub notes: &'a [String],
    /// The original file, if there is one.
    pub source: Option<Source<'a>>,
}

/// What [`read`] returns: the schedule as [`load_bytes`](crate::load_bytes) returns one, and what
/// the document holds beyond it. Install `loaded` the way a file's is (`Session::new`,
/// `with_outline`, `with_origin`, `with_data_date`, `with_progress`), then `baseline` with
/// [`Session::with_baseline`](crate::Session::with_baseline) and `collapsed` with
/// [`Session::with_collapsed`](crate::Session::with_collapsed). Neither leaves an undo step, so a
/// reopened document has an empty history.
pub struct Document {
    /// The schedule, solved, with its outline and display data. `unsupported` is `notes`.
    pub loaded: Loaded,
    /// The schedule's name.
    pub name: String,
    /// The "unsupported features" lines the open reported.
    pub notes: Vec<String>,
    /// The baseline, if one was set; indexed by the ids of `loaded`.
    pub baseline: Option<Baseline>,
    /// The collapsed summaries.
    pub collapsed: Vec<TaskId>,
    /// The original imported file, if the document holds it.
    pub source: Option<(SourceFormat, Vec<u8>)>,
    /// The keys of the links and calendars, and the maps over every key; install them with
    /// [`Session::with_host_keys`](crate::Session::with_host_keys).
    pub keys: crate::host::Keys,
}

/// Why [`read`] refused a text. Every message names the value it could not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    /// Not JSON, or not a schedule document: the text says what was found in place of
    /// `"format": "gantt"`.
    NotDocument(String),
    /// A document from a newer version of the app: its `version`.
    Newer(i64),
    /// The text is not JSON the reader accepts; the message starts with the byte offset.
    Syntax(String),
    /// A field is missing, unknown or of the wrong type (its path is in the message), or the
    /// schedule it describes is refused by the engine or the outline.
    Invalid(String),
    /// Two records of one kind hold one host key (decision 0016): the key, and the paths of the first
    /// record to hold it and of the one that repeats it.
    DuplicateKey {
        key: String,
        first: String,
        again: String,
    },
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDocument(found) => write!(f, "not a schedule document: {found}"),
            Self::Newer(version) => write!(
                f,
                "this document is version {version}; this app reads version {VERSION}"
            ),
            Self::Syntax(message) => write!(f, "not a schedule document: bad JSON at {message}"),
            Self::Invalid(message) => write!(f, "bad schedule document: {message}"),
            Self::DuplicateKey { key, first, again } => write!(
                f,
                "bad schedule document: {again}: key {key:?} is already the key of {first}"
            ),
        }
    }
}

impl std::error::Error for DocumentError {}
