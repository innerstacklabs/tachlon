//! [`RowFilter`]: which tasks the table keeps besides collapse (gs#703, split out of `rows.rs`).

use crate::texts::{self as t, Text, Texts};
use crate::{check_filter::CheckKind, status_filter::StatusKind};
use import::ActivityCode;
use std::sync::Arc;

/// Which tasks the table keeps besides collapse. One at a time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RowFilter {
    #[default]
    Off,
    Critical,
    /// Tasks with at most ten working days of float (gs#703).
    NearCritical,
    LongestPath,
    /// Tasks that finish later than the baseline has them (gs#703).
    Slipped,
    /// Tasks that have lost five working days of float since the baseline (gs#703).
    FloatLost,
    /// Tasks that are critical now and were not in the baseline (gs#703).
    JoinedCritical,
    /// Tasks that were critical in the baseline and are not now (gs#703).
    LeftCritical,
    /// Tasks the last move report lists (gs#703). Each new report moves it to that report's
    /// tasks, an undo's included; a change that makes none ends it.
    Moved,
    /// Leaves that carry this P6 activity code (a code type and a value). The `Arc` keeps a copy,
    /// which the status makes every frame, from allocating.
    ActivityCode(Arc<ActivityCode>),
    /// Leaves in one state, by the card's rule (gs#703).
    Status(StatusKind),
    /// Leaves one of the ten schedule checks counts (gs#703).
    Check(CheckKind),
    /// Tasks with a real start constraint, one strictly above their import floor (gs#703).
    StartConstraint,
    /// Leaves that name this resource (gs#703). The `Arc` keeps a copy, which the status makes
    /// every frame, from allocating.
    Resource(Arc<str>),
}

impl RowFilter {
    /// Whether the filter compares against the baseline, so it cannot stay on without one.
    #[must_use]
    pub fn needs_baseline(&self) -> bool {
        matches!(
            self,
            Self::Slipped | Self::JoinedCritical | Self::LeftCritical
        )
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Critical => "critical",
            Self::NearCritical => "near",
            Self::LongestPath => "longest",
            Self::Slipped => "slipped",
            Self::FloatLost => "float",
            Self::JoinedCritical => "joined",
            Self::LeftCritical => "left",
            Self::Moved => "moved",
            Self::ActivityCode(_) => "code",
            Self::Status(_) => "status",
            Self::Check(_) => "check",
            Self::StartConstraint => "constraint",
            Self::Resource(_) => "resource",
        }
    }

    /// The filter's translated word for a printed page.
    pub(crate) fn print_key(&self) -> &'static Text {
        match self {
            Self::Off => &t::FILTER_OFF,
            Self::Critical => &t::FILTER_CRITICAL,
            Self::NearCritical => &t::FILTER_NEAR,
            Self::LongestPath => &t::FILTER_LONGEST,
            Self::Slipped => &t::FILTER_SLIPPED,
            Self::FloatLost => &t::FILTER_FLOAT,
            Self::JoinedCritical => &t::FILTER_JOINED,
            Self::LeftCritical => &t::FILTER_LEFT,
            Self::Moved => &t::FILTER_MOVED,
            Self::ActivityCode(_) => &t::FILTER_CODE,
            Self::Status(_) => &t::FILTER_STATUS,
            Self::Check(_) => &t::FILTER_CHECK,
            Self::StartConstraint => &t::FILTER_CONSTRAINT,
            Self::Resource(_) => &t::FILTER_RESOURCE,
        }
    }

    /// The filter as the printed page's header words it, including a code, status or check value.
    #[must_use]
    pub fn print_label(&self, texts: &Texts) -> String {
        let filter = texts.say(self.print_key(), &[]);
        let label = |text, hole| texts.say(text, &[("filter", &filter), hole]);
        match self {
            Self::ActivityCode(code) => texts.say(
                &t::FILTER_CODE_FORMAT,
                &[
                    ("filter", &filter),
                    ("type", &code.code_type),
                    ("value", &code.value),
                ],
            ),
            Self::Status(kind) => label(&t::FILTER_STATUS_FORMAT, ("state", kind.name())),
            Self::Check(kind) => label(&t::FILTER_CHECK_FORMAT, ("check", kind.name())),
            _ => filter,
        }
    }
}
