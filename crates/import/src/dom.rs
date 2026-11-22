use std::borrow::Cow;
use std::fmt;

use quick_xml::Reader;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesRef, Event};

use crate::ImportError;

/// An element of the document. Its name and its text borrow from the input; the text is owned
/// only when it was assembled from several pieces (entities, CDATA) or had its line ends changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node<'x> {
    pub name: &'x str,
    pub text: Cow<'x, str>,
    pub children: Vec<Node<'x>>,
}

impl<'x> Node<'x> {
    pub fn child(&self, name: &str) -> Option<&Node<'x>> {
        self.children.iter().find(|child| child.name == name)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node<'x>> {
        self.children.iter().filter(move |child| child.name == name)
    }

    pub fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name).map(|child| child.text.trim())
    }
}

/// The element names the MS Project reader looks up (`Node::child`, `child_text` and
/// `children_named` in `lib.rs`, `calendars`, `resources`, `tasks` and their submodules). `parse` keeps an element only when its name is here, or when it is the root;
/// every other element is checked for well-formedness and then dropped without a string built for it.
fn is_read(name: &str) -> bool {
    matches!(
        name,
        "Active"
            | "ActualCost"
            | "ActualDuration"
            | "ActualFinish"
            | "ActualStart"
            | "Assignment"
            | "Assignments"
            | "BaseCalendarUID"
            | "Baseline"
            | "Calendar"
            | "CalendarUID"
            | "Calendars"
            | "ConstraintDate"
            | "ConstraintType"
            | "Cost"
            | "DayType"
            | "DayWorking"
            | "DaysOfWeek"
            | "Deadline"
            | "Duration"
            | "DurationFormat"
            | "EnteredByOccurrences"
            | "Exception"
            | "Exceptions"
            | "Finish"
            | "First"
            | "FixedCost"
            | "FromDate"
            | "FromTime"
            | "IsBaseCalendar"
            | "IsNull"
            | "LagFormat"
            | "LevelingDelay"
            | "LinkLag"
            | "Manual"
            | "Milestone"
            | "Month"
            | "MonthDay"
            | "Name"
            | "Notes"
            | "Number"
            | "Occurrences"
            | "OutlineLevel"
            | "PercentComplete"
            | "Period"
            | "PhysicalPercentComplete"
            | "PredecessorLink"
            | "PredecessorUID"
            | "Project"
            | "RemainingDuration"
            | "Resource"
            | "ResourceUID"
            | "Resources"
            | "Resume"
            | "Second"
            | "Start"
            | "StartDate"
            | "StatusDate"
            | "Stop"
            | "Summary"
            | "Task"
            | "TaskUID"
            | "Tasks"
            | "TimePeriod"
            | "ToDate"
            | "ToTime"
            | "Type"
            | "UID"
            | "WBS"
            | "WeekDay"
            | "WeekDays"
            | "WorkWeek"
            | "WorkWeeks"
            | "WorkingTime"
            | "WorkingTimes"
    )
}

/// A child of the root that the reader takes one element at a time: it is handed each `Task`,
/// `Calendar` or `Assignment` as soon as the element closes and the document keeps none of them,
/// so the whole file is never a tree. Only the first such child counts, as `child` finds only the
/// first.
#[derive(Debug, Clone, Copy)]
enum Section {
    Tasks,
    Calendars,
    Assignments,
}

impl Section {
    fn named(name: &str) -> Option<Self> {
        match name {
            "Tasks" => Some(Self::Tasks),
            "Calendars" => Some(Self::Calendars),
            "Assignments" => Some(Self::Assignments),
            _ => None,
        }
    }

    fn wants(self, name: &str) -> bool {
        name == match self {
            Self::Tasks => "Task",
            Self::Calendars => "Calendar",
            Self::Assignments => "Assignment",
        }
    }

    fn item(self, node: Node<'_>) -> Item<'_> {
        match self {
            Self::Tasks => Item::Task(node),
            Self::Calendars => Item::Calendar(node),
            Self::Assignments => Item::Assignment(node),
        }
    }
}

/// An element the reader takes out of the document as it closes (see [`Section`]).
#[derive(Debug)]
pub(crate) enum Item<'x> {
    Calendar(Node<'x>),
    Assignment(Node<'x>),
    Task(Node<'x>),
}

/// One open element. `node` is `None` for an element nothing reads and for everything inside it.
/// `name` is its qualified name as written, a slice of the input. `stream` is set on the first
/// `Tasks`, `Calendars` and `Assignments` child of the root.
#[derive(Debug)]
struct Frame<'x> {
    name: &'x str,
    node: Option<Node<'x>>,
    stream: Option<Section>,
}

/// What opening `name` makes of it: the section it streams, and whether the root keeps it. A
/// second `Tasks`, `Calendars` or `Assignments` under the root is dropped whole.
fn open_section(name: &str, depth: usize, opened: &mut [bool; 3]) -> (Option<Section>, bool) {
    let section = Section::named(name).filter(|_| depth == 1);
    let Some(section) = section else {
        return (None, true);
    };
    let seen = &mut opened[section as usize];
    if std::mem::replace(seen, true) {
        (None, false)
    } else {
        (Some(section), true)
    }
}

fn xml_error(message: impl fmt::Display, position: u64) -> ImportError {
    ImportError::Xml {
        message: format!("{message} at byte {position}"),
    }
}

/// The part of `xml` that `part` (bytes of an event the reader built from `xml`) is, found from
/// the bytes' own address, so a reader that skips a byte order mark cannot shift it. `None` when
/// `part` is not a slice of `xml`.
fn borrowed<'x>(xml: &'x str, part: &[u8]) -> Option<&'x str> {
    let start = (part.as_ptr() as usize).checked_sub(xml.as_ptr() as usize)?;
    let found = xml.get(start..start.checked_add(part.len())?)?;
    (found.as_bytes() == part).then_some(found)
}

fn qualified_name<'x>(xml: &'x str, name: &[u8], position: u64) -> Result<&'x str, ImportError> {
    borrowed(xml, name).ok_or_else(|| xml_error("element name is not text", position))
}

fn local_name(qualified: &str) -> &str {
    qualified
        .bytes()
        .rposition(|byte| byte == b':')
        .map_or(qualified, |colon| &qualified[colon + 1..])
}

/// A node for `name` when the reader keeps it: `parent_kept` is false inside a dropped element.
fn open_node(name: &str, parent_kept: bool, is_root: bool) -> Option<Node<'_>> {
    (is_root || (parent_kept && is_read(name))).then(|| Node {
        name,
        text: Cow::Borrowed(""),
        // Room for the children the file will give it, so the list grows once, not by doubling:
        // a task has about a dozen.
        children: Vec::with_capacity(if name == "Task" { 12 } else { 0 }),
    })
}

/// Hands a closed element to `sink` when its parent streams it, else attaches it to its parent.
fn route<'x>(
    node: Option<Node<'x>>,
    stack: &mut [Frame<'x>],
    root: &mut Option<Node<'x>>,
    position: u64,
    sink: &mut impl FnMut(&Node<'x>, Item<'x>),
) -> Result<(), ImportError> {
    match (stack.last().and_then(|frame| frame.stream), node) {
        (Some(section), Some(node)) if section.wants(node.name) => {
            if let Some(head) = stack.first().and_then(|frame| frame.node.as_ref()) {
                sink(head, section.item(node));
            }
            Ok(())
        }
        (_, node) => attach_node(node, stack, root, position),
    }
}

fn attach_node<'x>(
    node: Option<Node<'x>>,
    stack: &mut [Frame<'x>],
    root: &mut Option<Node<'x>>,
    position: u64,
) -> Result<(), ImportError> {
    if let Some(parent) = stack.last_mut() {
        if let (Some(node), Some(parent)) = (node, parent.node.as_mut()) {
            parent.children.push(node);
        }
    } else if root.is_some() {
        return Err(xml_error("content follows the root element", position));
    } else {
        *root = node;
    }
    Ok(())
}

fn append_text<'x>(
    text: Cow<'x, str>,
    stack: &mut [Frame<'x>],
    root_complete: bool,
    position: u64,
) -> Result<(), ImportError> {
    if let Some(frame) = stack.last_mut() {
        if let Some(node) = frame.node.as_mut() {
            if node.text.is_empty() {
                node.text = text;
            } else {
                node.text.to_mut().push_str(&text);
            }
        }
    } else if !text.trim().is_empty() {
        let message = if root_complete {
            "content follows the root element"
        } else {
            "text appears before the root element"
        };
        return Err(xml_error(message, position));
    }
    Ok(())
}

fn decoded_reference(reference: &BytesRef<'_>, position: u64) -> Result<String, ImportError> {
    if let Some(character) = reference
        .resolve_char_ref()
        .map_err(|error| xml_error(error, position))?
    {
        return Ok(character.to_string());
    }

    let name = reference
        .decode()
        .map_err(|error| xml_error(error, position))?;
    resolve_predefined_entity(&name)
        .map(str::to_owned)
        .ok_or_else(|| xml_error(format_args!("unknown entity &{name};"), position))
}

/// Reads `xml` and returns the root with the children it keeps, except that every `Task`,
/// `Calendar` and `Assignment` of the first `Tasks`, `Calendars` and `Assignments` goes to `sink`
/// when it closes, with the root as it stands then, and is not kept.
pub(crate) fn parse<'x>(
    xml: &'x str,
    mut sink: impl FnMut(&Node<'x>, Item<'x>),
) -> Result<Node<'x>, ImportError> {
    let mut reader = Reader::from_str(xml);
    let mut stack = Vec::<Frame<'_>>::with_capacity(64);
    let mut root = None;
    let mut opened = [false; 3];

    loop {
        let event = reader
            .read_event()
            .map_err(|error| xml_error(error, reader.error_position()))?;
        let position = reader.buffer_position();
        match event {
            Event::Start(start) => {
                if stack.len() >= 64 {
                    return Err(ImportError::TooDeep);
                }
                if stack.is_empty() && root.is_some() {
                    return Err(xml_error("content follows the root element", position));
                }
                let name = qualified_name(xml, start.name().as_ref(), position)?;
                let parent_kept = stack.last().is_none_or(|frame| frame.node.is_some());
                let local = local_name(name);
                let (stream, kept) = open_section(local, stack.len(), &mut opened);
                let node = open_node(local, parent_kept && kept, stack.is_empty());
                stack.push(Frame { name, node, stream });
            }
            Event::Empty(empty) => {
                if stack.len() >= 64 {
                    return Err(ImportError::TooDeep);
                }
                let name = qualified_name(xml, empty.name().as_ref(), position)?;
                let parent_kept = stack.last().is_none_or(|frame| frame.node.is_some());
                let local = local_name(name);
                let (_, kept) = open_section(local, stack.len(), &mut opened);
                let node = open_node(local, parent_kept && kept, stack.is_empty());
                route(node, &mut stack, &mut root, position, &mut sink)?;
            }
            Event::End(end) => {
                let Some(frame) = stack.pop() else {
                    return Err(xml_error("closing tag has no opening tag", position));
                };
                if frame.name.as_bytes() != end.name().as_ref() {
                    return Err(xml_error("mismatched closing tag", position));
                }
                route(frame.node, &mut stack, &mut root, position, &mut sink)?;
            }
            Event::Text(text) => {
                // The text is the input's own bytes unless a carriage return needs its line end
                // normalized.
                let own = match borrowed(xml, &text) {
                    Some(raw) if !raw.as_bytes().contains(&b'\r') => Cow::Borrowed(raw),
                    _ => {
                        let decoded = text
                            .xml10_content()
                            .map_err(|error| xml_error(error, position))?;
                        Cow::Owned(decoded.into_owned())
                    }
                };
                append_text(own, &mut stack, root.is_some(), position)?;
            }
            Event::CData(cdata) => {
                let decoded = cdata
                    .xml10_content()
                    .map_err(|error| xml_error(error, position))?;
                append_text(
                    Cow::Owned(decoded.into_owned()),
                    &mut stack,
                    root.is_some(),
                    position,
                )?;
            }
            Event::GeneralRef(reference) => {
                let decoded = decoded_reference(&reference, position)?;
                append_text(Cow::Owned(decoded), &mut stack, root.is_some(), position)?;
            }
            Event::Eof => {
                if !stack.is_empty() {
                    return Err(xml_error("unclosed element", position));
                }
                return root.ok_or_else(|| xml_error("missing root element", position));
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
}

#[cfg(test)]
mod tests;
