//! The document's `baseline` object.

use super::{Field, fields, int, list, opt, within};
use crate::baseline::{Baseline, BaselineTask};
use crate::document::json_read::Value;

pub(super) fn read_baseline(value: &Value, count: usize) -> Field<Baseline> {
    // A file written before the data date was kept has no such key: read it as none.
    let dated = matches!(value, Value::Object(m) if m.iter().any(|(k, _)| k == "data_date"));
    let (data_date, finish, tasks) = if dated {
        let [at, finish, tasks] = fields(value, &["data_date", "project_finish", "tasks"])?;
        (opt(at, |v| int(v, "data_date"))?, finish, tasks)
    } else {
        let [finish, tasks] = fields(value, &["project_finish", "tasks"])?;
        (None, finish, tasks)
    };
    let items = list(tasks, "tasks")?;
    if items.len() != count {
        return Err(format!(
            "tasks: expected {count} entries, one per task, found {}",
            items.len()
        ));
    }
    let mut slots = Vec::with_capacity(count);
    for (i, item) in items.iter().enumerate() {
        let slot = opt(item, |v| match list(v, "entry")? {
            [es, ef, float] => Ok(BaselineTask {
                es: int(es, "es")?,
                ef: int(ef, "ef")?,
                float: opt(float, |v| int(v, "float"))?,
            }),
            _ => Err("expected [early start, early finish, float]".to_string()),
        });
        slots.push(slot.map_err(|e| within(&format!("tasks[{i}]"), &e))?);
    }
    Ok(Baseline::from_parts(slots, int(finish, "project_finish")?).with_data_date(data_date))
}
