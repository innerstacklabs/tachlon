//! A host's values as the session keeps them (decision 0017): a date is read by the kind of its
//! field, a duration as whole working minutes, the engine's own unit.

use crate::{Session, document::json_read::Value};
use engine::{Instant, Minutes};

impl Session {
    /// The start-kind date `value` holds, `YYYY-MM-DD` or `YYYY-MM-DDTHH:mm`, or `None` for anything else.
    pub(super) fn read_date(&self, value: &Value) -> Option<Instant> {
        let text = value.as_str().filter(|text| !text.contains(' '))?;
        self.parse_start(&text.replacen('T', " ", 1)).ok()?
    }
}

/// The whole, non-negative number of working minutes `value` is, or `None` (a fractional number too).
pub(super) fn read_duration(value: &Value) -> Option<Minutes> {
    value.as_int().filter(|&n| n >= 0)
}
