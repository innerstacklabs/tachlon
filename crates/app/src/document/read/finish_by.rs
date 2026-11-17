//! The `must_finish_by` key of a document (gantt#2074): the date the project must finish by, in
//! minutes from `origin`, within the engine's date range (decision 0020).

use super::Field;
use crate::document::{fields::int, json_read::Value};
use engine::{Instant, MAX_DATE, MIN_DATE};

/// The date a document saves; no key is no date.
pub(super) fn read_finish_by(root: &Value) -> Field<Option<Instant>> {
    let Value::Object(members) = root else {
        return Ok(None);
    };
    if members
        .iter()
        .filter(|(k, _)| k == "must_finish_by")
        .count()
        > 1
    {
        return Err("must_finish_by: given twice".to_string());
    }
    let Some(value) = root.get("must_finish_by") else {
        return Ok(None);
    };
    let at = int(value, "must_finish_by")?;
    if (MIN_DATE..=MAX_DATE).contains(&at) {
        Ok(Some(at))
    } else {
        Err(format!("must_finish_by: {at} is out of range"))
    }
}
