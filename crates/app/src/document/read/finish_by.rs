//! The `must_finish_by` key of a document (gantt#2074): the date the project must finish by, in
//! minutes from `origin`.

use super::Field;
use crate::document::{fields::int, json_read::Value};
use engine::Instant;

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
    int(value, "must_finish_by").map(Some)
}
