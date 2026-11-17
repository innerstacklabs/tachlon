//! What `Session` writes across the WASM boundary, captured from real sessions (gs#254).
//!
//! Each capture builds a session in one state, calls `status_json`, `table_rows_json` or
//! `task_details_json`, parses the text with `serde_json` and records the keys. A key is "always"
//! when every capture of its shape has it, so an optional key is one some state leaves out.

use crate::common;

use std::collections::{BTreeMap, BTreeSet};

use app::{RenderFacts, Session};
use engine::TaskId;
use serde_json::Value;

/// The kind of a JSON value, for the positions of a table row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Null,
    Bool,
    Number,
    String,
}

fn kind_of(value: &Value) -> Kind {
    match value {
        Value::Null => Kind::Null,
        Value::Bool(_) => Kind::Bool,
        Value::Number(_) => Kind::Number,
        Value::String(_) => Kind::String,
        other => panic!("a table row cell that is not a scalar: {other}"),
    }
}

/// The keys of one object shape, over all its captures.
#[derive(Debug, Default, Clone)]
pub struct Shape {
    /// Every key any capture has.
    pub any: BTreeSet<String>,
    /// The keys every capture has; `None` before the first capture.
    pub always: Option<BTreeSet<String>>,
}

impl Shape {
    fn add(&mut self, object: &Value) {
        let keys: BTreeSet<String> = object
            .as_object()
            .unwrap_or_else(|| panic!("not a JSON object: {object}"))
            .keys()
            .cloned()
            .collect();
        self.any.extend(keys.iter().cloned());
        self.always = Some(match self.always.take() {
            None => keys,
            Some(seen) => seen.intersection(&keys).cloned().collect(),
        });
    }

    pub fn always(&self) -> BTreeSet<String> {
        self.always.clone().unwrap_or_default()
    }
}

/// Everything captured, by the TypeScript interface that types it.
#[derive(Clone)]
pub struct Captured {
    /// `Status`, `Status.lookahead`, `Hover`, `Drag`, `Selected`, `SlideStatus`.
    pub status: BTreeMap<&'static str, Shape>,
    pub task_details: Shape,
    /// The kinds each position of a `TableRow` takes.
    pub table_row: Vec<BTreeSet<Kind>>,
}

fn facts() -> RenderFacts {
    RenderFacts {
        backend: "WebGPU",
        surface: (1000, 400),
        bars: 3,
        critical_bars: 1,
        arrows: 2,
        shaded: 4,
        data_date_x: Some(12.5),
        epoch: 1,
        frames: 7,
    }
}

fn status_of(session: &Session) -> Value {
    serde_json::from_str(&session.status_json(&facts())).expect("status_json is valid JSON")
}

/// Points the viewport at `task`'s bar and returns a canvas point on it.
fn aim(session: &mut Session, task: u32, start: i64) -> (f64, f64) {
    let row = session
        .timeline()
        .row_of_task(task)
        .expect("task has a row");
    common::zoom_to(session, 100.0, start as f64 - 500.0 * 100.0);
    session.scroll_to_row(row);
    let v = &session.timeline().viewport;
    (
        v.x_of(start as f64) + 2.0,
        v.row_top(row) + v.row_height / 2.0,
    )
}

/// The first task of `session`'s outline that is (or is not) a summary.
fn find_task(session: &Session, summary: bool) -> u32 {
    let outline = session.outline().expect("an outline session");
    let order = session.timeline().order();
    *order
        .iter()
        .find(|&&t| outline.is_summary(TaskId(t)) == summary)
        .expect("the demo outline has both kinds")
}

#[expect(
    clippy::too_many_lines,
    reason = "one function drives a session through every capture"
)]
pub fn capture() -> Captured {
    let mut status: BTreeMap<&'static str, Shape> = BTreeMap::new();
    let mut add = |name: &'static str, value: &Value| status.entry(name).or_default().add(value);
    let add_status = |add: &mut dyn FnMut(&'static str, &Value), s: &Value| {
        add("Status", s);
        add("Status.lookahead", &s["lookahead"]);
        add("SlideStatus", &s["slide"]);
        for (key, name) in [
            ("hover", "Hover"),
            ("drag", "Drag"),
            ("selected", "Selected"),
            ("link", "Link"),
            ("moved", "Moved"),
        ] {
            if !s[key].is_null() {
                add(name, &s[key]);
            }
        }
        if !s["link"]["drag"].is_null() {
            add("LinkDrag", &s["link"]["drag"]);
        }
    };

    // Everything off, then hover on a leaf, a selection and a window, on an outline session.
    let (mut outline, _) = common::outline_session(60);
    add_status(&mut add, &status_of(&outline));
    let leaf = find_task(&outline, false);
    let start = outline.solution().dates[leaf as usize].es;
    let (x, y) = aim(&mut outline, leaf, start);
    outline.pointer_move(x, y);
    let hovered = status_of(&outline);
    assert_eq!(
        hovered["hover"]["task"], leaf,
        "the pointer missed the leaf"
    );
    assert!(hovered["hover"].get("float").is_some());
    add_status(&mut add, &hovered);

    // The same bar's finish edge: the hover gains `edge`.
    let finish = outline.solution().dates[leaf as usize].ef;
    let v = &outline.timeline().viewport;
    let (edge_x, edge_y) = (v.x_of(finish as f64) - 2.0, y);
    outline.pointer_move(edge_x, edge_y);
    let edged = status_of(&outline);
    assert_eq!(edged["hover"]["edge"], true, "the pointer missed the edge");
    add_status(&mut add, &edged);

    let summary = find_task(&outline, true);
    let (start, _) = outline.summary_span(summary).expect("a summary has a span");
    let (x, y) = aim(&mut outline, summary, start);
    outline.pointer_move(x, y);
    let hovered = status_of(&outline);
    assert_eq!(
        hovered["hover"]["summary"], true,
        "the pointer missed the summary"
    );
    add_status(&mut add, &hovered);

    outline.select(leaf);
    let selected = status_of(&outline);
    assert!(selected["selected"].is_object(), "no selection captured");
    add_status(&mut add, &selected);

    assert!(outline.set_lookahead(0, 4), "the window did not apply");
    let windowed = status_of(&outline);
    assert_eq!(windowed["lookahead"]["on"], true);
    add_status(&mut add, &windowed);

    // A drag in progress, on a plain session.
    let (mut drag, task, es, point) = common::setup_with(true);
    drag.pointer_down(point.0, point.1, false);
    drag.pointer_move(point.0 + common::TRAVEL_PX, point.1);
    drag.apply_drag();
    let dragging = status_of(&drag);
    assert_eq!(dragging["drag"]["task"], task, "no drag captured");
    assert_ne!(dragging["drag"]["es"], es);
    add_status(&mut add, &dragging);
    drag.pointer_up();
    let released = status_of(&drag);
    assert!(released["moved"].is_object(), "no move report captured");
    add_status(&mut add, &released);

    // A link drag in progress (an Alt press moved off its bar), then its refusal after the release.
    let (mut linking, task, _, point) = common::setup_with(true);
    linking.pointer_down(point.0, point.1, true);
    linking.pointer_move(point.0 + 40.0, point.1 + 60.0);
    let linked = status_of(&linking);
    assert_eq!(
        linked["link"]["drag"]["pred"], task,
        "no link drag captured"
    );
    add_status(&mut add, &linked);
    linking.pointer_up();
    add_status(&mut add, &status_of(&linking));

    // The hover card and the table, for a leaf row and a summary row.
    let mut task_details = Shape::default();
    for task in [leaf, summary] {
        let json = outline.task_details_json(task).expect("a task in range");
        task_details.add(&serde_json::from_str(&json).expect("task_details_json is valid JSON"));
    }
    assert!(outline.task_details_json(u32::MAX).is_none());
    // With a baseline set and a leaf lengthened: the leaf's card gains the baseline keys, a summary's does not.
    let (mut baselined, _) = common::outline_session(60);
    baselined
        .edit(app::edit::Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let (baseline_leaf, baseline_summary) =
        (find_task(&baselined, false), find_task(&baselined, true));
    let minutes = baselined.schedule().tasks()[baseline_leaf as usize].duration + 480;
    baselined
        .edit(app::edit::Edit::SetDuration {
            task: TaskId(baseline_leaf),
            minutes,
        })
        .expect("a longer duration applies");
    for task in [baseline_leaf, baseline_summary] {
        let json = baselined.task_details_json(task).expect("a task in range");
        let details: Value = serde_json::from_str(&json).expect("task_details_json is valid JSON");
        assert_eq!(
            details.get("baselineVariance").is_some(),
            task == baseline_leaf,
            "only the leaf carries the baseline keys"
        );
        task_details.add(&details);
    }

    let mut table_row: Vec<BTreeSet<Kind>> = Vec::new();
    let mut kinds_seen = BTreeSet::new();
    let mut add_rows = |session: &Session| {
        let rows: Value = serde_json::from_str(&session.table_rows_json(0, u32::MAX))
            .expect("table_rows_json is valid JSON");
        for row in rows.as_array().expect("an array of rows") {
            let cells = row.as_array().expect("a row is an array");
            table_row.resize(cells.len().max(table_row.len()), BTreeSet::new());
            for (position, cell) in cells.iter().enumerate() {
                table_row[position].insert(kind_of(cell));
            }
            kinds_seen.insert(cells[5].as_u64().expect("row kind is a number"));
        }
    };
    add_rows(&outline);
    // With a baseline set the variance cells of a leaf are text, where they were `null` above.
    outline
        .edit(app::edit::Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    add_rows(&outline);
    assert!(
        table_row[6..]
            .iter()
            .all(|kinds| kinds.contains(&Kind::String)),
        "the table capture needs a baseline row with variance text"
    );
    assert!(
        kinds_seen.contains(&0) && kinds_seen.contains(&1),
        "the table capture needs a leaf and a summary row, saw {kinds_seen:?}"
    );

    Captured {
        status,
        task_details,
        table_row,
    }
}
