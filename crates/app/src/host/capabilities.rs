//! The host capability profile (gantt#2090, decision 0019, rule 5): the gestures and groups of fields
//! the host's database cannot take, told once at mount. Nothing is off until the host says so.

mod values;

use self::values::Stores;
use super::Plan;
use crate::document::fields::fields_allowing;
use crate::document::json_read::{Value, parse_streaming};
use crate::press::{Before, Press};
use crate::texts::{self as t, Text};
use crate::{Session, edit::Edit, edit::EditError};
use engine::TaskId;

/// One thing a host can turn off: a gesture, or a group of fields. Every [`Edit`] is in one group.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    BarMove,
    Resize,
    Link,
    Rows,
    Outline,
    Paste,
    Names,
    Dates,
    Links,
    Progress,
    Baseline,
}

/// Each item once: its place in the profile, and the words a refusal names it with.
const TABLE: [(Item, &str, &str); 11] = [
    (Item::BarMove, "gestures.barMove", "moving a bar"),
    (Item::Resize, "gestures.resize", "resizing a bar"),
    (Item::Link, "gestures.link", "drawing a link"),
    (Item::Rows, "editable.rows", "adding and deleting rows"),
    (Item::Outline, "editable.outline", "changing the outline"),
    (Item::Paste, "editable.paste", "pasting rows"),
    (Item::Names, "editable.names", "changing names"),
    (
        Item::Dates,
        "editable.dates",
        "changing dates and durations",
    ),
    (Item::Links, "editable.links", "changing links"),
    (
        Item::Progress,
        "editable.progress",
        "changing progress and the data date",
    ),
    (Item::Baseline, "editable.baseline", "changing the baseline"),
];

/// The sentence for the refusal that names `what`, one of the words in [`TABLE`].
pub(crate) fn not_allowed(what: &str) -> &'static Text {
    match what {
        "moving a bar" => &t::NOT_ALLOWED_BAR_MOVE,
        "resizing a bar" => &t::NOT_ALLOWED_RESIZE,
        "drawing a link" => &t::NOT_ALLOWED_LINK,
        "adding and deleting rows" => &t::NOT_ALLOWED_ROWS,
        "changing the outline" => &t::NOT_ALLOWED_OUTLINE,
        "pasting rows" => &t::NOT_ALLOWED_PASTE,
        "changing names" => &t::NOT_ALLOWED_NAMES,
        "changing dates and durations" => &t::NOT_ALLOWED_DATES,
        "changing links" => &t::NOT_ALLOWED_LINKS,
        "changing progress and the data date" => &t::NOT_ALLOWED_PROGRESS,
        _ => &t::NOT_ALLOWED_BASELINE,
    }
}

/// The items a host turned off, one bit each, and the values it stores; none is the whole chart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Capabilities {
    off: u16,
    stores: Stores,
}

impl Capabilities {
    /// The profile in `json`, `{"gestures":{"barMove":false},"editable":{"rows":false}}`: each
    /// member is on unless it says `false`. `kinds`, `perTask`, `durationStep`, `lagStep` and
    /// `dates` say what the host stores, and each is as unlimited as it can be when it is left out.
    fn read(json: &str) -> Result<Self, String> {
        let root = parse_streaming(json, &mut |_, _, _, _, _| {})?;
        let values = ["kinds", "perTask", "durationStep", "lagStep", "dates"];
        fields_allowing(
            &root,
            &[],
            &[&["gestures", "editable"][..], &values].concat(),
        )?;
        let mut off = 0;
        for section in ["gestures", "editable"] {
            let members = match root.get(section) {
                None => continue,
                Some(Value::Object(members)) => members,
                Some(_) => return Err(format!("{section}: expected an object")),
            };
            for (key, value) in members {
                let name = format!("{section}.{key}");
                let row = TABLE.iter().find(|row| row.1 == name);
                let (item, ..) = row.ok_or_else(|| format!("{name}: unknown key"))?;
                let Value::Bool(on) = value else {
                    return Err(format!("{name}: expected true or false"));
                };
                off |= u16::from(!on) << *item as u16;
            }
        }
        let stores = Stores::read(&root)?;
        Ok(Self { off, stores })
    }

    /// The words for the first of `items` the profile turned off.
    fn refused(self, items: impl IntoIterator<Item = Item>) -> Option<&'static str> {
        let off = |item: &Item| self.off >> *item as u16 & 1 == 1;
        let item = items.into_iter().find(off)?;
        TABLE.iter().find(|row| row.0 == item).map(|row| row.2)
    }
}

/// The group `edit` changes.
fn group(edit: &Edit) -> Item {
    match edit {
        Edit::Insert { .. } | Edit::Delete { .. } | Edit::Restore { .. } => Item::Rows,
        Edit::Reparent { .. } => Item::Outline,
        Edit::Rename { .. } => Item::Names,
        Edit::Move { .. } | Edit::SetDuration { .. } | Edit::SetLimit { .. } => Item::Dates,
        Edit::AddLink { .. }
        | Edit::RemoveLink { .. }
        | Edit::RestoreLink { .. }
        | Edit::SetLink { .. } => Item::Links,
        Edit::SetProgress { .. } | Edit::SetDataDate { .. } => Item::Progress,
        Edit::SetBaseline { .. } | Edit::ClearBaseline => Item::Baseline,
    }
}

impl Session {
    /// Tell the chart what the host's database can take, replacing any profile it had; `{}` allows
    /// everything. A change the profile turned off is refused at the gate on every path, undo and
    /// redo included, with [`EditError::NotAllowed`]. `reopen` keeps the profile.
    ///
    /// # Errors
    /// A message that starts with the member at fault, for text that is not a profile; the
    /// profile stays as it was.
    pub fn set_capabilities(&mut self, json: &str) -> Result<(), String> {
        self.host.capabilities = Capabilities::read(json)?;
        Ok(())
    }

    /// The profile's check for the action `plan` describes: its gesture, then the group of each edit
    /// it would make (the ones an undo, a redo or a discard replays included).
    pub(super) fn refuse_unallowed(&self, plan: &Plan) -> Result<(), EditError> {
        let capabilities = self.host.capabilities;
        if capabilities == Capabilities::default() {
            return Ok(());
        }
        let gesture = match *plan {
            Plan::Drag(Press {
                before: Before::Snet(_),
                ..
            }) => Some(Item::BarMove),
            Plan::Drag(_) => Some(Item::Resize),
            Plan::Action("linkDrag", ..) | Plan::Link(..) => Some(Item::Link),
            Plan::Paste(_) => Some(Item::Paste),
            _ => self.replays_data_date(plan).then_some(Item::Progress),
        };
        let edits = match *plan {
            Plan::Edit(edit) => vec![edit.clone()],
            Plan::Group(edits, _) => edits.to_vec(),
            Plan::Drag(press) => self.drag_edit(press).into_iter().collect(),
            Plan::Replay(_, source) => self.replayed(source),
            Plan::Discard(depth) => self.discarded(depth),
            Plan::Dialog(task, text) => self.dialog_edits(task, text),
            Plan::Paste([_, ..]) => {
                let selected = self.selection.as_ref().map(|s| TaskId(s.task));
                let place = self.place_below(selected);
                vec![Edit::Insert {
                    place,
                    name: String::new(),
                }]
            }
            _ => Vec::new(),
        };
        let drawn =
            matches!(plan, Plan::Action("linkDrag", ..) | Plan::Link(..)).then_some(Item::Links);
        let pasted = matches!(plan, Plan::Paste(_)).then_some(Item::Rows);
        let items = gesture
            .into_iter()
            .chain(drawn)
            .chain(pasted)
            .chain(edits.iter().map(group));
        if let Some(what) = capabilities.refused(items) {
            return Err(EditError::NotAllowed(what));
        }
        let stored = self.unstorable(capabilities.stores, &edits);
        stored.map_or(Ok(()), |what| Err(EditError::NotStored(what)))
    }
}
