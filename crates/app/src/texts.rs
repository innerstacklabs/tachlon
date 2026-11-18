//! The sentences the app refuses an edit with and the words of a printed page (gantt#1875). Each has a key a host sets it by in
//! `App.set_locale`'s `texts`, English defaults, and `{name}`-style holes the app fills. A refusal's
//! `kind` does not change with the text. The table belongs to a chart's `Session`, so two charts on
//! one page can speak two languages: an error says which sentence and its values ([`Worded::text`]),
//! and the chart that refused picks the words.

use crate::document::fields::text_of;
use crate::document::json_read::Value;
use std::collections::HashMap;

/// One sentence: the key a host sets it by, and its English.
pub(crate) struct Text {
    key: &'static str,
    english: &'static str,
}

macro_rules! texts {
    ($($name:ident $key:literal $english:literal;)*) => {
        $(pub(crate) const $name: Text = Text { key: $key, english: $english };)*
        const ALL: &[&Text] = &[$(&$name),*];
    };
}

texts! {
    UNKNOWN_TASK "unknownTask" "unknown {task}";
    EMPTY_KEY "emptyKey" "a key cannot be empty";
    HAS_KEY "hasKey" "this record already holds a different key";
    KEY_TAKEN "keyTaken" "another record already holds this key";
    EMPTY_NAME "emptyName" "a name cannot be empty";
    INVALID_NAME "invalidName" "a name cannot contain \" / \" or a control character";
    NO_OUTLINE "noOutline" "this schedule has no outline";
    BUSY "busy" "Finish the drag first.";
    NO_SELECTION "noSelection" "Select a row first.";
    NO_LINK_SELECTED "noLinkSelected" "Select a link first.";
    FILTER_ACTIVE "filterActive" "Clear the row filter or window to insert a row.";
    INSERT_GROUPED "insertGrouped" "Turn off the grouping to insert a row.";
    HAS_CHILDREN_ONE "hasChildrenOne" "\"{name}\" is a summary with {count} sub-task. Delete or move its sub-tasks first.";
    HAS_CHILDREN_MANY "hasChildrenMany" "\"{name}\" is a summary with {count} sub-tasks. Delete or move its sub-tasks first.";
    BAD_PLACE "badPlace" "that task cannot be restored there";
    NO_PREVIOUS_SIBLING "noPreviousSibling" "This row has no sibling above it.";
    NO_NEXT_SIBLING "noNextSibling" "This row has no sibling below it.";
    ALREADY_TOP_LEVEL "alreadyTopLevel" "This row is already at the top level.";
    REPARENT_FILTERED "reparentFiltered" "Clear the row filter or window to indent, outdent or move a row.";
    REPARENT_GROUPED "reparentGrouped" "Turn off the grouping to indent, outdent or move a row.";
    BAD_DATA_DATE "badDataDate" "Write the data date as YYYY-MM-DD.";
    BAD_DATE "badDate" "Write the date as YYYY-MM-DD.";
    BAD_PERCENT "badPercent" "Write the percent complete as a whole number from 0 to 100, such as 75 or 75%.";
    FINISH_BEFORE_START "finishBeforeStart" "The actual finish is before the actual start.";
    ACTUAL_AFTER_DATA_DATE "actualAfterDataDate" "An actual date cannot be after the data date. Move the data date first, or write a date on or before it.";
    HAS_ACTUALS "hasActuals" "This task has an actual start or finish, so it cannot be dragged. Clear its actual dates first.";
    PROGRESS_ON_SUMMARY "progressOnSummary" "\"{name}\" is a summary. Record progress on its sub-tasks instead.";
    DATA_DATE_THROUGH_EDIT "dataDateThroughEdit" "the data date is set through the edit history";
    LINK_TO_SUMMARY "linkToSummary" "\"{name}\" is a summary. Link its sub-tasks instead.";
    HAS_LINKS "hasLinks" "\"{name}\" has links, and a summary takes none. Remove its links first, or link its sub-tasks instead.";
    ALREADY_LINKED "alreadyLinked" "\"{pred}\" is already linked to \"{succ}\".";
    UNKNOWN_LINK "unknownLink" "unknown {link}";
    PREDECESSOR_UNKNOWN "predecessorUnknown" "\"{item}\" is not the ID of a task.";
    PREDECESSOR_ITSELF "predecessorItself" "\"{item}\" is this task, and a task cannot follow itself.";
    PREDECESSOR_TWICE "predecessorTwice" "\"{item}\" is in the list more than once.";
    PREDECESSOR_NO_IDS "predecessorNoIds" "This schedule has no Activity IDs to type.";
    PREDECESSOR_LINK "predecessorLink" "\"{item}\": {error}";
    NO_BASELINE "noBaseline" "There is no baseline to clear.";
    BASELINE_ON_SUMMARY "baselineOnSummary" "\"{name}\" is a summary. Re-baseline its sub-tasks instead.";
    NOT_IN_BASELINE "notInBaseline" "\"{name}\" is not in the baseline. Use Add new.";
    SEVERAL_ROWS "severalRows" "Delete works on one row. Select a single row to delete it.";
    READ_ONLY "readOnly" "This chart is read-only, so nothing in it can be changed.";
    REFUSED "refused" "This change is not allowed.";
    ROLLBACK_GONE "rollbackGone" "The chart can no longer go back to before that change. Load the saved schedule again.";
    BEFORE_EDIT_FAILED "beforeEditFailed" "This change was not made: the page's before-edit function failed.";
    LINK_CYCLE "linkCycle" "\"{pred}\" to \"{succ}\" would make a loop: {chain}.";
    LINK_CYCLE_ELIDED "linkCycleElided" "{first} → {second} → {third} → … ({count} tasks) → {last} → {first}";
    START_SUMMARY "startSummary" "\"{name}\" is a summary. Its start comes from its sub-tasks; set theirs instead.";
    START_BAD_DATE "startBadDate" "Write the start as YYYY-MM-DD or YYYY-MM-DD HH:MM.";
    START_ALREADY_LATER "startAlreadyLater" "The links already start this task on or after that date; nothing changed.";
    START_UNCHANGED "startUnchanged" "The start is already set to that; nothing changed.";
    START_NOTHING_TO_REMOVE "startNothingToRemove" "This task has no start constraint to remove.";
    FINISH_SUMMARY "finishSummary" "\"{name}\" is a summary. Its finish comes from its sub-tasks; set theirs instead.";
    FINISH_DAY_BEFORE_START "finishDayBeforeStart" "That day is before the task starts on {start}. Write a day on or after it.";
    DURATION_EMPTY "durationEmpty" "type a duration such as 3d, 4h or 1d 4h";
    DURATION_NEGATIVE "durationNegative" "a duration cannot be negative";
    DURATION_INVALID "durationInvalid" "not a duration: use d, h and m, such as 3d, 4h or 1d 4h (a lone number is days)";
    DURATION_TOO_LONG "durationTooLong" "that duration is too long";
    LINK_TEXT_KIND "linkTextKind" "start with FS, SS, FF or SF, such as SS+2d";
    LINK_TEXT_SIGN "linkTextSign" "write the lag with a sign, such as FS+2d or FF-1d";
    LINK_TEXT_LAG "linkTextLag" "not a lag: {error}";
    PRINT_DATA_DATE "printDataDate" "Data date: {date}";
    PRINT_LAYOUT "printLayout" "Layout: {name}";
    PRINT_FILTER "printFilter" "Filter: {filter}";
    PRINT_LOOKAHEAD_ONE "printLookaheadOne" "Lookahead: {start}, {weeks} week";
    PRINT_LOOKAHEAD_MANY "printLookaheadMany" "Lookahead: {start}, {weeks} weeks";
    PRINT_DATES "printDates" "Dates: {from} to {to}";
    PRINT_PRINTED "printPrinted" "Printed {date}";
    PRINT_PAGE "printPage" "Page {page} of {pages}";
    PRINT_LEFT_OUT "printLeftOut" "{count} rows not printed. Select a range to print them.";
    FILTER_OFF "filterOff" "off";
    FILTER_CRITICAL "filterCritical" "critical";
    FILTER_NEAR "filterNear" "near";
    FILTER_LONGEST "filterLongest" "longest";
    FILTER_SLIPPED "filterSlipped" "slipped";
    FILTER_FLOAT "filterFloat" "float";
    FILTER_JOINED "filterJoined" "joined";
    FILTER_LEFT "filterLeft" "left";
    FILTER_MOVED "filterMoved" "moved";
    FILTER_CODE "filterCode" "code";
    FILTER_STATUS "filterStatus" "status";
    FILTER_CHECK "filterCheck" "check";
    FILTER_CONSTRAINT "filterConstraint" "constraint";
    FILTER_RESOURCE "filterResource" "resource";
    FILTER_CODE_FORMAT "filterCodeFormat" "{filter} {type} = {value}";
    FILTER_STATUS_FORMAT "filterStatusFormat" "{filter} {state}";
    FILTER_CHECK_FORMAT "filterCheckFormat" "{filter} {check}";
    LEGEND_TASK "legendTask" "Task";
    LEGEND_CRITICAL "legendCritical" "Critical";
    LEGEND_DEADLINE "legendDeadline" "Deadline";
    LEGEND_START_CONSTRAINT "legendStartConstraint" "Start constraint";
    LEGEND_FINISH_CONSTRAINT "legendFinishConstraint" "Finish constraint";
    LEGEND_SUMMARY "legendSummary" "Summary";
    LEGEND_DEPENDENCY "legendDependency" "Dependency";
    LEGEND_DATA_DATE "legendDataDate" "Data date";
    LEGEND_NON_WORKING_DAY "legendNonWorkingDay" "Non-working day";
    LEGEND_PROPOSAL_GHOST "legendProposalGhost" "Proposal ghost";
    LEGEND_JUST_MOVED "legendJustMoved" "Just-moved outline";
    HEAD_ID "headId" "ID";
    HEAD_WBS "headWbs" "WBS";
    HEAD_TASK "headTask" "Task";
    HEAD_DUR "headDur" "Dur";
    HEAD_START "headStart" "Start";
    HEAD_FINISH "headFinish" "Finish";
    HEAD_FLOAT "headFloat" "Float";
    HEAD_START_VARIANCE "headStartVariance" "Start variance";
    HEAD_FINISH_VARIANCE "headFinishVariance" "Finish variance";
    HEAD_FLOAT_VARIANCE "headFloatVariance" "Float variance";
    HEAD_ACTUAL_START "headActualStart" "Actual start";
    HEAD_ACTUAL_FINISH "headActualFinish" "Actual finish";
    HEAD_REMAINING "headRemaining" "Remaining";
    HEAD_PERCENT_COMPLETE "headPercentComplete" "% complete";
    HEAD_START_VAR "headStartVar" "Start var.";
    HEAD_FINISH_VAR "headFinishVar" "Finish var.";
    HEAD_FLOAT_VAR "headFloatVar" "Float var.";
    MOVED_DATA_DATE_MOVED "movedDataDateMoved" "data date moved";
    MOVED_DATA_DATE_ONE "movedDataDateOne" "data date {days} day";
    MOVED_DATA_DATE_MANY "movedDataDateMany" "data date {days} days";
    MOVED_DATA_DATE_CLEARED "movedDataDateCleared" "data date cleared";
    MOVED_DATA_DATE_SET "movedDataDateSet" "data date set";
    MOVED_TASKS_ONE "movedTasksOne" "{count} task moved";
    MOVED_TASKS_MANY "movedTasksMany" "{count} tasks moved";
    MOVED_FINISH_ONE "movedFinishOne" "finish {days} day";
    MOVED_FINISH_MANY "movedFinishMany" "finish {days} days";
    MOVED_CRITICAL "movedCritical" "critical {joined} joined, {left} left";
    CARD_NO_WORKING_DAYS "cardNoWorkingDays" "no working days";
    CARD_WEEK_ONE "cardWeekOne" "{days} day a week, {hours} h a day";
    CARD_WEEK_MANY "cardWeekMany" "{days} days a week, {hours} h a day";
    CARD_FLOAT_ONE "cardFloatOne" "{days} calendar day";
    CARD_FLOAT_MANY "cardFloatMany" "{days} calendar days";
    CARD_MISSED_UNDER_DAY "cardMissedUnderDay" "{date} (missed by <1d)";
    CARD_MISSED "cardMissed" "{date} (missed by {days}d)";
    CARD_MORE "cardMore" "+ {count} more";
    COMPARE_NO_SHARED_ID "compareNoSharedId" "no task has the same ID in both files";
    COMPARE_ONLY_OLDER "compareOnlyOlder" "Only in the older file: {count}";
    COMPARE_ONLY_HERE "compareOnlyHere" "Only in this file: {count}";
    COMPARE_MORE "compareMore" "and {count} more";
    PERCENT_ALONE "percentAlone" "it sets the duration, the actual dates and the remaining; change it alone";
    DRAG_FINISHED "dragFinished" "{name} is finished: its dates are its actuals. Change Actual finish in the table to move it.";
    DRAG_PUT_BACK "dragPutBack" "{name}: its duration could not be put back. {error}";
    DRAG_RESIZE_STARTED "dragResizeStarted" "{name} has started: change Remaining or % complete in the table to move its finish.";
    DRAG_MOVE_STARTED "dragMoveStarted" "{name} has started: its start is its actual start. Change Remaining or % complete in the table to move its finish.";
}

/// The sentences a host set for one chart, by key; a key not here reads in English.
#[derive(Clone, Debug, Default)]
pub struct Texts(HashMap<&'static str, String>);

/// An error that is a sentence: it names which one, with its values, and the words come from the
/// [`Texts`] of the chart that refused. Its `Display` is the English.
pub trait Worded {
    /// The sentence in `texts`' words, in English where `texts` has none.
    fn text(&self, texts: &Texts) -> String;
}

/// `impl Display` for a [`Worded`] type: its English.
macro_rules! english_display {
    ($error:ty) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&crate::texts::Worded::text(self, &Default::default()))
            }
        }
    };
}
pub(crate) use english_display;

impl crate::Session {
    /// `error` in the words of this chart's `set_locale`.
    #[must_use]
    pub fn say(&self, error: &impl Worded) -> String {
        error.text(&self.texts)
    }
}

impl Texts {
    /// `text` as the host set it, or in English, with each `{hole}` of `holes` filled once: a
    /// value that holds a `{hole}` is not filled again.
    pub(crate) fn say(&self, text: &Text, holes: &[(&str, &str)]) -> String {
        let mut rest = self.0.get(text.key).map_or(text.english, String::as_str);
        let mut out = String::with_capacity(rest.len());
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let tail = &rest[open + 1..];
            let hole = holes.iter().find(|(name, _)| {
                tail.strip_prefix(name)
                    .is_some_and(|after| after.starts_with('}'))
            });
            if let Some((name, value)) = hole {
                out.push_str(value);
                rest = &tail[name.len() + 1..];
            } else {
                out.push('{');
                rest = tail;
            }
        }
        out.push_str(rest);
        out
    }

    /// `one` when `single`, else `many`, filled as [`Texts::say`] does: a sentence that counts.
    pub(crate) fn say_by(
        &self,
        single: bool,
        one: &Text,
        many: &Text,
        holes: &[(&str, &str)],
    ) -> String {
        self.say(if single { one } else { many }, holes)
    }

    /// The members of `texts`, an object of sentences by key. A key the table does not have, and a
    /// value that is not a string, are refused.
    pub(crate) fn read(texts: &Value) -> Result<Self, String> {
        let Value::Object(members) = texts else {
            return Err("texts: expected an object".to_string());
        };
        let mut table = HashMap::new();
        for (key, value) in members {
            let text = ALL
                .iter()
                .find(|text| text.key == key)
                .ok_or_else(|| format!("texts.{key}: unknown key"))?;
            table.insert(text.key, text_of(value, &format!("texts.{key}"))?);
        }
        Ok(Self(table))
    }
}

/// The English table as JSON, `{"emptyName":"a name cannot be empty",..}`: what `App.default_texts`
/// returns, for a host to translate.
#[must_use]
pub fn english_json() -> String {
    let members: Vec<String> = ALL
        .iter()
        .map(|text| format!("{}:{}", crate::quote(text.key), crate::quote(text.english)))
        .collect();
    format!("{{{}}}", members.join(","))
}
