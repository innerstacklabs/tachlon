//! The row filters' and find's wasm calls (gs#703).

use super::App;
use crate::check_filter::CheckKind;
use crate::json::{quote, string_array};
use crate::status_filter::StatusKind;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Whether the Near critical filter is on.
    pub fn near_critical(&self) -> bool {
        self.session.near_critical()
    }

    /// Turn the Near critical filter on or off: with it on, a row shows only when its task has at
    /// most ten working days of float (of its own calendar), or a task inside it does. It is one row
    /// filter at a time. False when it cannot change now (no outline, or a bar press is active). A
    /// successful change bumps [`Session::stamp`](crate::Session::stamp), so the task table redraws.
    pub fn set_near_critical(&mut self, on: bool) -> bool {
        self.session.set_near_critical(on)
    }

    /// Whether the Start constraint filter is on.
    pub fn start_constraint_filter(&self) -> bool {
        self.session.start_constraint_filter()
    }

    /// Turn the Start constraint filter on or off: with it on, a row shows only when its task carries
    /// a real start constraint (`App.task_constraint_text` is not empty), or a task inside it does. It
    /// is one row filter at a time. False when it cannot change now (no outline, or a bar press is
    /// active). A successful change bumps [`Session::stamp`](crate::Session::stamp).
    pub fn set_start_constraint_filter(&mut self, on: bool) -> bool {
        self.session.set_start_constraint_filter(on)
    }

    /// The state the Status filter keeps: `in-progress`, `not-started` or `complete`; an empty
    /// string when that filter is off.
    pub fn status_filter(&self) -> String {
        self.session
            .status_filter()
            .map_or_else(String::new, |kind| kind.name().to_owned())
    }

    /// Keep only the leaves in state `kind` (`in-progress`, `not-started` or `complete`), and the
    /// summaries above them; an empty string turns the filter off. It replaces any other row
    /// filter. False, changing nothing, for any other `kind`, with no outline, or during a bar
    /// press. A successful change bumps [`Session::stamp`](crate::Session::stamp).
    pub fn set_status_filter(&mut self, kind: &str) -> bool {
        match kind {
            "" => self.session.set_status_filter(None),
            _ => StatusKind::from_name(kind)
                .is_some_and(|kind| self.session.set_status_filter(Some(kind))),
        }
    }

    /// The check the Check filter keeps: one of the ten [`CheckKind`] names; an empty
    /// string when that filter is off.
    pub fn check_filter(&self) -> String {
        self.session
            .check_filter()
            .map_or_else(String::new, |kind| kind.name().to_owned())
    }

    /// Keep only the leaves the check `kind` (one of the ten [`CheckKind`] names) counts,
    /// and the summaries above them; an empty string turns the filter off. It replaces any other
    /// row filter. False, changing nothing, for any other `kind`, with no outline, or during a bar
    /// press. A successful change bumps [`Session::stamp`](crate::Session::stamp).
    pub fn set_check_filter(&mut self, kind: &str) -> bool {
        match kind {
            "" => self.session.set_check_filter(None),
            _ => CheckKind::from_name(kind)
                .is_some_and(|kind| self.session.set_check_filter(Some(kind))),
        }
    }

    /// The resource names the open schedule's live tasks carry, as a JSON array of strings in order
    /// of name, each once; `[]` for a file that names none.
    pub fn resource_names(&self) -> String {
        string_array(&self.session.resource_names())
    }

    /// The resource the Resource filter keeps; an empty string when that filter is off.
    pub fn resource_filter(&self) -> String {
        self.session
            .resource_filter()
            .unwrap_or_default()
            .to_owned()
    }

    /// Keep only the leaves that name the resource `name`, and the summaries above them; an empty
    /// string turns the filter off. It replaces any other row filter. False, changing nothing, for a
    /// name no live task carries, with no outline, or during a bar press. A successful change bumps
    /// [`Session::stamp`](crate::Session::stamp).
    pub fn set_resource_filter(&mut self, name: &str) -> bool {
        self.session
            .set_resource_filter((!name.is_empty()).then_some(name))
    }
}

#[wasm_bindgen]
impl App {
    /// Turn the row filter and the Lookahead window off together, so every row is listed again. True
    /// when it ran, also with nothing on; false, changing nothing, with no outline or during a bar
    /// press. A change bumps [`Session::stamp`](crate::Session::stamp), so the task table redraws.
    pub fn clear_filters(&mut self) -> bool {
        self.session.clear_filters()
    }

    /// The activity code filter as a JSON pair `[type, value]`, exactly as it was set, or `null`
    /// when that filter is off. Unlike the header label it never needs splitting, and it holds
    /// while the filter is on even when no task carries the code any more.
    pub fn activity_code_filter_pair(&self) -> String {
        (self.session.activity_code_filter()).map_or_else(
            || "null".to_owned(),
            |code| format!("[{},{}]", quote(&code.code_type), quote(&code.value)),
        )
    }

    /// Whether the Float lost filter is on.
    pub fn float_lost(&self) -> bool {
        self.session.float_lost()
    }

    /// Whether the baseline holds float to compare with (false for none, or one read from a file).
    pub fn baseline_has_float(&self) -> bool {
        self.session.baseline_has_float()
    }

    /// Turn the Float lost filter on or off: with it on, a row shows only when its task has at
    /// least five working days (of its own calendar) less float than the baseline held, or a task
    /// inside it does. It is one row filter at a time. False when turning it on with no baseline
    /// float, or when there is no outline or a bar press is active. A successful change bumps
    /// [`Session::stamp`](crate::Session::stamp), so the task table redraws.
    pub fn set_float_lost(&mut self, on: bool) -> bool {
        self.session.set_float_lost(on)
    }

    /// Whether the Joined critical filter is on.
    pub fn joined_critical(&self) -> bool {
        self.session.joined_critical_only()
    }

    /// Turn the Joined critical filter on or off: with it on, a row shows only when its task is
    /// critical now and was not in the baseline, or a task inside it is. It is one row filter at a
    /// time. False when it cannot change now (turning it on with no baseline, no outline, or a bar
    /// press is active). Clearing the baseline turns it off. A successful change bumps
    /// [`Session::stamp`](crate::Session::stamp), so the task table redraws.
    pub fn set_joined_critical(&mut self, on: bool) -> bool {
        self.session.set_joined_critical_only(on)
    }

    /// Whether the Left critical filter is on.
    pub fn left_critical(&self) -> bool {
        self.session.left_critical_only()
    }

    /// Turn the Left critical filter on or off: the reverse of Joined critical, the tasks that were
    /// critical in the baseline and are not now. The same rules, and the same refusals.
    pub fn set_left_critical(&mut self, on: bool) -> bool {
        self.session.set_left_critical_only(on)
    }

    /// Find tasks whose displayed names contain `query`, case-insensitively,
    /// excluding tasks hidden by the active row filter. Collapse does not
    /// affect the results.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn find(&mut self, query: &str) -> u32 {
        self.session.find(query).len() as u32
    }

    /// Select and reveal the `k`th result from the most recent search.
    pub fn find_go(&mut self, k: u32) -> Option<u32> {
        self.session.find_go(k)
    }

    /// Whether the critical-only filter is on.
    pub fn critical_only(&self) -> bool {
        self.session.critical_only()
    }

    /// Turn the critical-only filter on or off. False when it cannot change
    /// now (no outline, or a bar press is active). A successful change bumps
    /// [`Session::stamp`], so the task table redraws.
    pub fn set_critical_only(&mut self, on: bool) -> bool {
        self.session.set_critical_only(on)
    }

    /// Whether the longest-path row filter is on.
    pub fn longest_path(&self) -> bool {
        self.session.longest_path()
    }

    /// Turn the longest-path row filter on or off. False when it cannot change
    /// now (no outline, or a bar press is active). A successful change bumps
    /// [`Session::stamp`], so the task table redraws.
    pub fn set_longest_path(&mut self, on: bool) -> bool {
        self.session.set_longest_path(on)
    }

    /// Whether the Slipped filter is on.
    pub fn slipped_only(&self) -> bool {
        self.session.slipped_only()
    }

    /// Turn the Slipped filter on or off: with it on, a row shows only when its task finishes
    /// later than the baseline has it, or a task inside it does. It is one row filter at a time:
    /// turning it on replaces Critical only, Longest path, Show moved or an activity code, and
    /// turning it off leaves a different filter on. False when it cannot change now (turning it on
    /// with no baseline, no outline, or a bar press is active). Clearing the baseline turns it off.
    /// A successful change bumps [`Session::stamp`], so the task table redraws.
    pub fn set_slipped_only(&mut self, on: bool) -> bool {
        self.session.set_slipped_only(on)
    }

    /// The P6 activity code types the open schedule's tasks carry, each with its values, as a JSON
    /// array of `[type, [value, ...]]`, in order of name; `[]` for a file with no activity codes.
    pub fn activity_codes(&self) -> String {
        let types: Vec<String> = (self.session.activity_codes().iter())
            .map(|(code_type, values)| {
                let values: Vec<String> = values.iter().map(|value| quote(value)).collect();
                format!("[{},[{}]]", quote(code_type), values.join(","))
            })
            .collect();
        format!("[{}]", types.join(","))
    }

    /// The activity code filter as it reads in the header (`Phase = Civil`), or an empty string
    /// when that filter is off.
    pub fn activity_code_filter(&self) -> String {
        (self.session.activity_code_filter())
            .map_or_default(|code| format!("{} = {}", code.code_type, code.value))
    }

    /// Keep only the leaves that carry the activity code `code_type` = `value`, and the summaries
    /// that contain one. It replaces the critical-only and longest-path filters and combines with
    /// the lookahead window and with collapse. False, and nothing changes, when no task carries
    /// that code, there is no outline, or a bar press is active. A successful change bumps
    /// [`Session::stamp`], so the task table redraws.
    pub fn set_activity_code_filter(&mut self, code_type: &str, value: &str) -> bool {
        self.session.set_activity_code_filter(code_type, value)
    }

    /// Turn the activity code filter off; another row filter stays on. False when it cannot
    /// change now (no outline, or a bar press is active). A successful change bumps
    /// [`Session::stamp`], so the task table redraws.
    pub fn clear_activity_code_filter(&mut self) -> bool {
        self.session.clear_activity_code_filter()
    }

    /// Group the rows under one header per value of the activity code type `code_type` (decision
    /// 0015), or show the outline again with an empty string. View state: not saved and not an undo
    /// step. False, and nothing changes, when no task carries that type, there is no outline or a
    /// bar press is active. A successful change bumps [`Session::stamp`], so the task table
    /// redraws.
    pub fn set_group_by(&mut self, code_type: &str) -> bool {
        let known = (self.session.activity_codes().iter()).any(|(known, _)| known == code_type);
        match code_type {
            "" => self.session.set_group_by(None),
            _ => known && self.session.set_group_by(Some(code_type)),
        }
    }

    /// The activity code type the rows are grouped by, or an empty string when they are not.
    pub fn group_by(&self) -> String {
        self.session.group_by().unwrap_or_default().to_owned()
    }

    /// The activity code types the rows can be grouped by, as a JSON array of names in order of
    /// name; `[]` for a file with no activity codes.
    pub fn group_choices(&self) -> String {
        let types: Vec<String> = (self.session.activity_codes().iter())
            .map(|(code_type, _)| quote(code_type))
            .collect();
        format!("[{}]", types.join(","))
    }

    /// Hide the leaves under the group header `value`, or show them again; `undefined` is the
    /// header of the leaves with no value. False when the rows are not grouped, no header has that
    /// value, or a bar press is active.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm-bindgen reads the optional JS string into an owned value"
    )]
    pub fn toggle_group(&mut self, value: Option<String>) -> bool {
        self.session.toggle_group(value.as_deref())
    }

    /// Keep only the tasks whose early dates intersect the `weeks` weeks that start at 00:00 on
    /// `start`, a date written `YYYY-MM-DD`. It combines with the critical-only and longest-path
    /// filters and with collapse: a row shows when it passes every one. False, and nothing
    /// changes, when `start` is not a date, `weeks` is 0 or above 520, there is no outline, or a
    /// bar press is active. A successful change bumps [`Session::stamp`], so the task table
    /// redraws.
    pub fn set_lookahead(&mut self, start: &str, weeks: u32) -> bool {
        match self.session.parse_lookahead_day(start) {
            Some(start) => self.session.set_lookahead(start, weeks),
            None => false,
        }
    }

    /// Pan so 00:00 on `day`, written `YYYY-MM-DD`, sits a fifth of the way in from the left, and
    /// scroll so the first row, in table order, whose bar starts on or after that day is the top
    /// row. The zoom stays. False, and nothing changes, when `day` is not a date or a bar press or
    /// drag is in progress.
    pub fn go_to_date(&mut self, day: &str) -> bool {
        self.session.go_to_date(day)
    }

    /// Remove the lookahead window. False when it cannot change now (no outline, or a bar press
    /// is active). A successful change bumps [`Session::stamp`], so the task table redraws.
    pub fn clear_lookahead(&mut self) -> bool {
        self.session.clear_lookahead()
    }
}
