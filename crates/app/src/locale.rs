//! The names a host sets for its language (gantt#1875): the text of `App.set_locale`, and the
//! `d Mon` the app's three date labels write with them.

use crate::Session;
use crate::document::fields::{fields_allowing, list, small, text_of};
use crate::document::json_read::parse_streaming;
use crate::texts::Texts;
use engine::Instant;
use timeline::names::Names;

/// The `N` names `value` lists under `key`.
fn names_of<const N: usize>(
    value: &crate::document::json_read::Value,
    key: &str,
) -> Result<[String; N], String> {
    let names = list(value, key)?
        .iter()
        .map(|item| text_of(item, key))
        .collect::<Result<Vec<_>, _>>()?;
    let found = names.len();
    names
        .try_into()
        .map_err(|_| format!("{key}: expected {N} names, found {found}"))
}

impl Session {
    /// Use the names in `json`, `{"months":[12 names],"weekdays":[7 names],"weekStart":0}`, January
    /// and Monday first, for the app's date labels, and for the sessions `reopen` makes from this
    /// one. `weekStart` is the day the axis's weeks start on: 0 is Monday, 6 is Sunday. An optional
    /// `texts`, an object from a key of [`crate::texts::english_json`] to a sentence, gives the words
    /// this chart refuses an edit in and words its printed pages in; a key it leaves out stays English.
    ///
    /// # Errors
    /// A message that starts with the key at fault, for text that is not JSON of that shape; the
    /// names stay as they were.
    pub fn set_locale(&mut self, json: &str) -> Result<(), String> {
        let root = parse_streaming(json, &mut |_, _, _, _, _| {})?;
        let [months, weekdays, week_start] =
            fields_allowing(&root, &["months", "weekdays", "weekStart"], &["texts"])?;
        let week_start: u8 = small(week_start, "weekStart")?;
        if week_start > 6 {
            return Err(format!("weekStart: {week_start} is not 0 to 6"));
        }
        let texts = root.get("texts").map(Texts::read).transpose()?;
        self.names = Names {
            months: names_of(months, "months")?,
            weekdays: names_of(weekdays, "weekdays")?,
            week_start,
        };
        self.texts = texts.unwrap_or_default();
        Ok(())
    }

    /// `d Mon` for the day that holds `t`, for example `12 Jan`, with the month in the set names.
    pub(crate) fn day_month(&self, t: Instant) -> String {
        let date = self.origin.date_of(t);
        format!(
            "{} {}",
            date.day,
            self.names.months[(date.month as usize - 1) % 12]
        )
    }
}
