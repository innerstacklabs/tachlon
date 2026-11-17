//! A print layout as data (gs#703): the print setup a person saved, as JSON text, to apply to
//! another schedule. [`Session::layout`] reads the row filter and the Lookahead window off the
//! session; the columns, the date window, the paper and the name are the page's, so it hands them
//! in. [`Session::apply_layout`] sets the filter and the window and names every part it could not
//! apply, so a layout never fits a schedule silently.

use crate::check_filter::CheckKind;
use crate::document::fields::{fields, int, list, opt, small, text_of};
use crate::document::json_read::parse_streaming;
use crate::json::{quote, string_array};
use crate::{PageSize, RowFilter, Session, status_filter::StatusKind};

/// What a saved print setup holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub name: String,
    /// `letter` or `a4`.
    pub paper: String,
    /// The list columns to print, by header name; `None` is every column the file has.
    pub columns: Option<Vec<String>>,
    /// The row filter: its `Status.filter` name, and its values (a code's type and value, a status,
    /// a check or a resource); none for the other filters.
    pub filter: (String, Vec<String>),
    /// The Lookahead window: its first day (`YYYY-MM-DD`) and its length in weeks.
    pub lookahead: Option<(String, u32)>,
    /// The printed date window `[from, to)`, as `App.print_pages` takes it.
    pub window: Option<(String, String)>,
}

impl Layout {
    /// The layout as JSON text, keys in a fixed order.
    #[must_use]
    pub fn to_json(&self) -> String {
        let columns = self
            .columns
            .as_deref()
            .map_or("null".to_owned(), string_array);
        let lookahead = self
            .lookahead
            .as_ref()
            .map_or("null".to_owned(), |(day, weeks)| {
                format!(r#"{{"start":{},"weeks":{weeks}}}"#, quote(day))
            });
        let window = self
            .window
            .as_ref()
            .map_or("null".to_owned(), |(from, to)| {
                format!(r#"{{"from":{},"to":{}}}"#, quote(from), quote(to))
            });
        let (kind, values) = &self.filter;
        format!(
            r#"{{"version":1,"name":{},"paper":{},"columns":{columns},"filter":{{"kind":{},"values":{}}},"lookahead":{lookahead},"window":{window}}}"#,
            quote(&self.name),
            quote(&self.paper),
            quote(kind),
            string_array(values),
        )
    }

    /// The layout `text` holds, or why it is not one: a message that starts with the key at fault.
    /// Every key must be there and none may be unknown.
    ///
    /// # Errors
    ///
    /// A message for text that is not JSON, or not a version 1 layout.
    pub fn from_json(text: &str) -> Result<Layout, String> {
        let root = parse_streaming(text, &mut |_, _, _, _, _| {})?;
        let [version, name, paper, columns, filter, lookahead, window] = fields(
            &root,
            &[
                "version",
                "name",
                "paper",
                "columns",
                "filter",
                "lookahead",
                "window",
            ],
        )?;
        if int(version, "version")? != 1 {
            return Err("version: only 1 is read".to_owned());
        }
        let texts = |value: &crate::document::json_read::Value| {
            let items = list(value, "values")?;
            items
                .iter()
                .map(|item| text_of(item, "values"))
                .collect::<Result<Vec<_>, _>>()
        };
        let [kind, values] = fields(filter, &["kind", "values"])?;
        Ok(Layout {
            name: text_of(name, "name")?,
            paper: text_of(paper, "paper")?,
            columns: opt(columns, texts)?,
            filter: (text_of(kind, "kind")?, texts(values)?),
            lookahead: opt(lookahead, |value| {
                let [start, weeks] = fields(value, &["start", "weeks"])?;
                Ok((text_of(start, "start")?, small(weeks, "weeks")?))
            })?,
            window: opt(window, |value| {
                let [from, to] = fields(value, &["from", "to"])?;
                Ok((text_of(from, "from")?, text_of(to, "to")?))
            })?,
        })
    }
}

impl Session {
    /// The print setup now: the session's row filter and Lookahead window with the page's `name`,
    /// `paper`, `columns` and date `window`.
    ///
    /// # Errors
    ///
    /// A message for a paper that is not `letter` or `a4`, and for a window that is not two days
    /// with `to` after `from`.
    pub fn layout(
        &self,
        name: &str,
        paper: &str,
        columns: Option<Vec<String>>,
        window: Option<(String, String)>,
    ) -> Result<Layout, String> {
        let filter = match &self.row_filter {
            RowFilter::ActivityCode(code) => {
                ("code", vec![code.code_type.clone(), code.value.clone()])
            }
            RowFilter::Status(kind) => ("status", vec![kind.name().to_owned()]),
            RowFilter::Check(kind) => ("check", vec![kind.name().to_owned()]),
            RowFilter::Resource(resource) => ("resource", vec![resource.to_string()]),
            other => (other.clone().name(), Vec::new()),
        };
        let layout = Layout {
            name: name.to_owned(),
            paper: paper.to_owned(),
            columns,
            filter: (filter.0.to_owned(), filter.1),
            lookahead: self
                .lookahead
                .map(|w| (self.origin.day_text(w.start), w.weeks)),
            window,
        };
        let problems = self.sheet_problems(&layout);
        if problems.is_empty() {
            Ok(layout)
        } else {
            Err(problems.join("; "))
        }
    }

    /// Set the row filter and the Lookahead window to the layout's, and say which parts of it the
    /// schedule cannot take: a filter that needs a baseline the file lacks, a code or a resource no
    /// task has, a column the file has none of, an unusable paper, window or Lookahead. A part that
    /// is not applied changes nothing (a filter stays as it was); an empty list means all of it
    /// was.
    pub fn apply_layout(&mut self, layout: &Layout) -> Vec<String> {
        let mut report = self.sheet_problems(layout);
        if let Some(columns) = &layout.columns {
            let header = self.rows_cells(Vec::new(), Some(columns)).swap_remove(0);
            report.extend(
                columns
                    .iter()
                    .filter(|name| !header.contains(name))
                    .map(|name| format!("column {name}: not applied, the file has no such column")),
            );
        }
        let (kind, values) = &layout.filter;
        if !self.set_layout_filter(kind, values) {
            let label = [kind.as_str()]
                .into_iter()
                .chain(values.iter().map(String::as_str));
            report.push(format!(
                "filter {}: not applied, {}",
                label.collect::<Vec<_>>().join(" "),
                self.why_not(kind)
            ));
        }
        let (applied, label) = match &layout.lookahead {
            None => (self.clear_lookahead(), "off".to_owned()),
            Some((day, weeks)) => (
                self.parse_lookahead_day(day)
                    .is_some_and(|start| self.set_lookahead(start, *weeks)),
                format!("{day}, {weeks} weeks"),
            ),
        };
        if !applied {
            report.push(format!(
                "lookahead {label}: not applied, it is not a day and 1 to 520 weeks, or the schedule has no outline or a bar press is active"
            ));
        }
        report
    }

    /// The paper and the date window of `layout`, each as a line when it cannot be used.
    fn sheet_problems(&self, layout: &Layout) -> Vec<String> {
        let mut found = Vec::new();
        if PageSize::parse(&layout.paper).is_none() {
            found.push(format!(
                "paper {}: not applied, use letter or a4",
                layout.paper
            ));
        }
        if let Some((from, to)) = &layout.window {
            let days = (self.parse_lookahead_day(from), self.parse_lookahead_day(to));
            if !matches!(days, (Some(a), Some(b)) if a < b) {
                found.push(format!(
                    "window {from} to {to}: not applied, from and to are days and to is after from"
                ));
            }
        }
        found
    }

    fn set_layout_filter(&mut self, kind: &str, values: &[String]) -> bool {
        match (kind, values) {
            ("off", []) => self.set_row_filter(RowFilter::Off),
            ("critical", []) => self.set_critical_only(true),
            ("near", []) => self.set_near_critical(true),
            ("longest", []) => self.set_longest_path(true),
            ("slipped", []) => self.set_slipped_only(true),
            ("float", []) => self.set_float_lost(true),
            ("joined", []) => self.set_joined_critical_only(true),
            ("left", []) => self.set_left_critical_only(true),
            ("moved", []) => self.set_moved_only(true),
            ("constraint", []) => self.set_start_constraint_filter(true),
            ("code", [code_type, value]) => self.set_activity_code_filter(code_type, value),
            ("status", [name]) => {
                StatusKind::from_name(name).is_some_and(|kind| self.set_status_filter(Some(kind)))
            }
            ("check", [name]) => {
                CheckKind::from_name(name).is_some_and(|kind| self.set_check_filter(Some(kind)))
            }
            ("resource", [name]) => self.set_resource_filter(Some(name)),
            _ => false,
        }
    }

    /// Why a filter of `kind` could not be turned on.
    fn why_not(&self, kind: &str) -> &'static str {
        match kind {
            _ if self.outline.is_none() || self.busy() => {
                "the schedule has no outline or a bar press is active"
            }
            "slipped" | "joined" | "left" => "the file has no baseline",
            "float" => "the file has no baseline with float",
            "moved" => "no edit has moved a task yet",
            "code" => "no task in the file carries that code",
            "resource" => "no task in the file names that resource",
            "status" | "check" => "this build does not know that value",
            _ => "this build does not know that filter",
        }
    }
}
