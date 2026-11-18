//! Why an edit was refused ([`EditError`]) and how each reason reads to a person.

use super::{DurationError, LinkTextError, PredecessorFault, links::loop_text};
use crate::outline::OutlineError;
use crate::texts::{
    ACTUAL_AFTER_DATA_DATE, ALREADY_LINKED, ALREADY_TOP_LEVEL, BAD_DATA_DATE, BAD_DATE,
    BAD_PERCENT, BAD_PLACE, BASELINE_ON_SUMMARY, BEFORE_EDIT_FAILED, BUSY, DATA_DATE_THROUGH_EDIT,
    EMPTY_KEY, EMPTY_NAME, FILTER_ACTIVE, FINISH_BEFORE_START, HAS_ACTUALS, HAS_CHILDREN_MANY,
    HAS_CHILDREN_ONE, HAS_KEY, HAS_LINKS, INSERT_GROUPED, INVALID_NAME, KEY_TAKEN, LINK_TO_SUMMARY,
    NO_BASELINE, NO_LINK_SELECTED, NO_NEXT_SIBLING, NO_OUTLINE, NO_PREVIOUS_SIBLING, NO_SELECTION,
    NOT_IN_BASELINE, PROGRESS_ON_SUMMARY, READ_ONLY, REFUSED, REPARENT_FILTERED, REPARENT_GROUPED,
    ROLLBACK_GONE, SEVERAL_ROWS, Texts, UNKNOWN_LINK, UNKNOWN_TASK, Worded, english_display,
};
use engine::{DepId, ScheduleError, TaskId};

/// Why an edit was refused. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The schedule refused it (unknown task, negative duration, no
    /// admissible start).
    Schedule(ScheduleError),
    /// The task has no metadata to rename.
    UnknownTask(TaskId),
    /// A name must have a visible character.
    EmptyName,
    /// A key must have a character.
    EmptyKey,
    /// The task or link already holds a different key.
    HasKey,
    /// Another record of the same kind holds the key, deleted or not.
    KeyTaken,
    /// A name may not hold the outline separator `" / "` or a control
    /// character.
    InvalidName,
    /// A structural edit needs an outline.
    NoOutline,
    /// A bar press is active; rows cannot change under the pointer.
    Busy,
    /// Nothing is selected.
    NoSelection,
    /// A link command with no link selected.
    NoLinkSelected,
    /// A row filter or lookahead window is on: the new row would not pass it and would vanish.
    FilterActive,
    /// The rows are grouped: the row above is not the outline sibling, so the place is a guess.
    InsertGrouped,
    /// A summary cannot be deleted: delete or move its sub-tasks first.
    HasChildren {
        task: TaskId,
        name: String,
        count: usize,
    },
    /// The task is not deleted, or the place names a task that is not live or not a sibling.
    BadPlace,
    /// The outline would be invalid; nothing changed.
    Outline(OutlineError),
    /// The link would close a loop. `names` are the loop's tasks as a person reads them, `succ`
    /// first and `pred` last; `path` is the engine's loop (`pred`, `succ`, then back to the task
    /// before `pred`).
    LinkCycle {
        names: Vec<String>,
        path: Vec<TaskId>,
    },
    /// A row with no previous sibling has nothing to indent under or move above.
    NoPreviousSibling,
    /// A row with no next sibling has nothing to move below.
    NoNextSibling,
    /// A top-level row cannot be outdented.
    AlreadyTopLevel,
    /// A row filter or lookahead window is on: the moved row could vanish.
    ReparentFiltered,
    /// The rows are grouped: the row above is not the outline sibling, so the parent is a guess.
    ReparentGrouped,
    /// A summary takes no links: link its sub-tasks.
    LinkToSummary { name: String },
    /// The row has a link and the edit would make it a summary: remove its links, or link its
    /// sub-tasks.
    HasLinks { name: String },
    /// A live link from `pred` to `succ` exists already.
    AlreadyLinked {
        pred: TaskId,
        succ: TaskId,
        pred_name: String,
        succ_name: String,
    },
    /// No such link.
    UnknownLink(DepId),
    /// The typed type and lag do not parse.
    LinkText(LinkTextError),
    /// An item of a typed list of predecessors is refused, `item` as it was typed.
    Predecessor {
        item: String,
        fault: PredecessorFault,
    },
    /// There is no baseline to clear.
    NoBaseline,
    /// A summary takes no baseline of its own: re-baseline its sub-tasks.
    BaselineOnSummary { name: String },
    /// The task was added after the baseline was set, so it has no entry to reset.
    NotInBaseline { name: String },
    /// Text for the data date is not a day written `YYYY-MM-DD`.
    BadDataDate,
    /// [`crate::Session::apply`] does not take a data-date edit: its inverse is one edit, which cannot
    /// hold a floor and a line that differ. [`crate::Session::edit`] keeps both in the history.
    DataDateThroughEdit,
    /// Text for an actual date is not a day written `YYYY-MM-DD`.
    BadDate,
    /// Text for a remaining duration does not parse.
    Duration(DurationError),
    /// The actual finish is earlier than the actual start.
    FinishBeforeStart,
    /// An actual start or finish typed later than the data date: work cannot have happened yet.
    ActualAfterDataDate,
    /// A task with an actual start or finish does not drag: its dates are what happened.
    HasActuals,
    /// Text for a percent complete is not a whole number from 0 to 100.
    BadPercent,
    /// A summary takes no progress of its own: record it on its sub-tasks.
    ProgressOnSummary { name: String },
    /// Delete deletes one row, and a range of several is selected.
    SeveralRows,
    /// The host switched the chart to read-only for this person: no way to change the schedule is open.
    ReadOnly,
    /// The host's before-edit function refused it: its reason, or the default sentence when empty.
    Refused(String),
    /// The host asked to go back to a change the chart can no longer go back to: it was saved, or the
    /// history or the unsaved list dropped it.
    RollbackGone,
    /// The host's before-edit function threw or answered with something it must not (a promise): nothing changed.
    BeforeEditFailed,
}

impl EditError {
    /// The reason as a name a program reads (`busy`, `noSelection`), one per variant. The host API
    /// (`packages/gantt/src/edit-result.ts`) maps these to its own names; the person's sentence is `Display`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Schedule(_) => "schedule",
            Self::UnknownTask(_) => "unknownTask",
            Self::EmptyName => "emptyName",
            Self::EmptyKey => "emptyKey",
            Self::HasKey => "hasKey",
            Self::KeyTaken => "keyTaken",
            Self::InvalidName => "invalidName",
            Self::NoOutline => "noOutline",
            Self::Busy => "busy",
            Self::NoSelection => "noSelection",
            Self::NoLinkSelected => "noLinkSelected",
            Self::FilterActive => "filterActive",
            Self::InsertGrouped => "insertGrouped",
            Self::HasChildren { .. } => "hasChildren",
            Self::BadPlace => "badPlace",
            Self::Outline(_) => "outline",
            Self::LinkCycle { .. } => "linkCycle",
            Self::NoPreviousSibling => "noPreviousSibling",
            Self::NoNextSibling => "noNextSibling",
            Self::AlreadyTopLevel => "alreadyTopLevel",
            Self::ReparentFiltered => "reparentFiltered",
            Self::ReparentGrouped => "reparentGrouped",
            Self::LinkToSummary { .. } => "linkToSummary",
            Self::HasLinks { .. } => "hasLinks",
            Self::AlreadyLinked { .. } => "alreadyLinked",
            Self::UnknownLink(_) => "unknownLink",
            Self::LinkText(_) => "linkText",
            Self::Predecessor { .. } => "predecessor",
            Self::NoBaseline => "noBaseline",
            Self::BaselineOnSummary { .. } => "baselineOnSummary",
            Self::NotInBaseline { .. } => "notInBaseline",
            Self::BadDataDate => "badDataDate",
            Self::DataDateThroughEdit => "dataDateThroughEdit",
            Self::BadDate => "badDate",
            Self::Duration(_) => "duration",
            Self::FinishBeforeStart => "finishBeforeStart",
            Self::ActualAfterDataDate => "actualAfterDataDate",
            Self::HasActuals => "hasActuals",
            Self::BadPercent => "badPercent",
            Self::ProgressOnSummary { .. } => "progressOnSummary",
            Self::SeveralRows => "severalRows",
            Self::ReadOnly => "readOnly",
            Self::Refused(_) => "refused",
            Self::RollbackGone => "rollbackGone",
            Self::BeforeEditFailed => "beforeEditFailed",
        }
    }
}

impl From<ScheduleError> for EditError {
    fn from(error: ScheduleError) -> Self {
        Self::Schedule(error)
    }
}

impl Worded for EditError {
    fn text(&self, texts: &Texts) -> String {
        let plain = |text| texts.say(text, &[]);
        let named = |text, name: &str| texts.say(text, &[("name", name)]);
        match self {
            Self::Schedule(error) => error.to_string(),
            Self::UnknownTask(task) => texts.say(&UNKNOWN_TASK, &[("task", &task.to_string())]),
            Self::EmptyName => plain(&EMPTY_NAME),
            Self::EmptyKey => plain(&EMPTY_KEY),
            Self::HasKey => plain(&HAS_KEY),
            Self::KeyTaken => plain(&KEY_TAKEN),
            Self::InvalidName => plain(&INVALID_NAME),
            Self::NoOutline => plain(&NO_OUTLINE),
            Self::Busy => plain(&BUSY),
            Self::NoSelection => plain(&NO_SELECTION),
            Self::NoLinkSelected => plain(&NO_LINK_SELECTED),
            Self::FilterActive => plain(&FILTER_ACTIVE),
            Self::InsertGrouped => plain(&INSERT_GROUPED),
            Self::HasChildren { name, count, .. } => texts.say(
                if *count == 1 {
                    &HAS_CHILDREN_ONE
                } else {
                    &HAS_CHILDREN_MANY
                },
                &[("name", name), ("count", &count.to_string())],
            ),
            Self::BadPlace => plain(&BAD_PLACE),
            Self::Outline(error) => error.to_string(),
            Self::NoPreviousSibling => plain(&NO_PREVIOUS_SIBLING),
            Self::NoNextSibling => plain(&NO_NEXT_SIBLING),
            Self::AlreadyTopLevel => plain(&ALREADY_TOP_LEVEL),
            Self::ReparentFiltered => plain(&REPARENT_FILTERED),
            Self::ReparentGrouped => plain(&REPARENT_GROUPED),
            Self::BadDataDate => plain(&BAD_DATA_DATE),
            Self::BadDate => plain(&BAD_DATE),
            Self::BadPercent => plain(&BAD_PERCENT),
            Self::Duration(error) => error.text(texts),
            Self::FinishBeforeStart => plain(&FINISH_BEFORE_START),
            Self::ActualAfterDataDate => plain(&ACTUAL_AFTER_DATA_DATE),
            Self::HasActuals => plain(&HAS_ACTUALS),
            Self::ProgressOnSummary { name } => named(&PROGRESS_ON_SUMMARY, name),
            Self::DataDateThroughEdit => plain(&DATA_DATE_THROUGH_EDIT),
            Self::LinkCycle { names, .. } => loop_text(texts, names),
            Self::LinkToSummary { name } => named(&LINK_TO_SUMMARY, name),
            Self::HasLinks { name } => named(&HAS_LINKS, name),
            Self::AlreadyLinked {
                pred_name,
                succ_name,
                ..
            } => texts.say(&ALREADY_LINKED, &[("pred", pred_name), ("succ", succ_name)]),
            Self::UnknownLink(dep) => texts.say(&UNKNOWN_LINK, &[("link", &dep.to_string())]),
            Self::LinkText(error) => error.text(texts),
            Self::Predecessor { item, fault } => fault.say(texts, item),
            Self::NoBaseline => plain(&NO_BASELINE),
            Self::BaselineOnSummary { name } => named(&BASELINE_ON_SUMMARY, name),
            Self::NotInBaseline { name } => named(&NOT_IN_BASELINE, name),
            Self::SeveralRows => plain(&SEVERAL_ROWS),
            Self::ReadOnly => plain(&READ_ONLY),
            Self::Refused(reason) if !reason.is_empty() => reason.clone(),
            Self::Refused(_) => plain(&REFUSED),
            Self::RollbackGone => plain(&ROLLBACK_GONE),
            Self::BeforeEditFailed => plain(&BEFORE_EDIT_FAILED),
        }
    }
}
english_display!(EditError);

impl std::error::Error for EditError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_is_one_name_of_ascii_letters() {
        let kinds = [
            EditError::Busy,
            EditError::NoSelection,
            EditError::FilterActive,
            EditError::NoOutline,
            EditError::NoBaseline,
            EditError::BadDataDate,
            EditError::HasChildren {
                task: TaskId(0),
                name: "Civil".into(),
                count: 2,
            },
        ]
        .map(|error| error.kind());
        assert_eq!(
            kinds,
            [
                "busy",
                "noSelection",
                "filterActive",
                "noOutline",
                "noBaseline",
                "badDataDate",
                "hasChildren"
            ]
        );
    }
}
