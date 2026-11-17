//! The JSON that crosses the WASM boundary has one shape on each side, and the two agree
//! (gs#161, gs#254). `Session::status_json`, `table_rows_json` and `task_details_json`
//! (`crates/app/src/json.rs`) write the JSON; `packages/gantt/src/api.ts` types it by hand. Renaming a key
//! on either side compiles and fails only at run time, so this test captures the key sets from
//! real sessions (`captures.rs`) and compares them with `api.ts`, exactly, both ways:
//!
//! - an object's keys, and whether each is optional (a key a capture lacks must be `?` in
//!   `api.ts`, and a `?` key must be one some capture lacks);
//! - the cells of a table row, by position: how many, and which JSON kinds each takes.
//!
//! The `mutations_*` tests rename a key in a captured key set and in the text of `api.ts` in turn
//! and require the comparison to notice, so it is shown to catch a break on either side.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

mod captures;
mod ts_side;

use std::collections::BTreeSet;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use app::Session;
use captures::{Captured, Kind, Shape, capture};
use ts_side::{Member, Members, TsShapes};

const API_TS: &str = "packages/gantt/src/api.ts";

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// How the two sides differ for one object shape, as messages.
fn diff(name: &str, rust: &Shape, ts: &Members) -> Vec<String> {
    let always = rust.always();
    let mut out = Vec::new();
    for key in &rust.any {
        match ts.get(key) {
            None => out.push(format!(
                "{name}: json.rs writes `{key}` but api.ts declares no such member"
            )),
            Some(Member { optional, .. }) => {
                let written = always.contains(key);
                if written && *optional {
                    out.push(format!(
                        "{name}.{key}: api.ts marks it optional but json.rs always writes it"
                    ));
                } else if !written && !optional {
                    out.push(format!(
                        "{name}.{key}: json.rs does not always write it but api.ts marks it required"
                    ));
                }
            }
        }
    }
    for key in ts.keys().filter(|k| !rust.any.contains(*k)) {
        out.push(format!(
            "{name}: api.ts declares `{key}` but json.rs never writes it"
        ));
    }
    out
}

fn members<'a>(ts: &'a TsShapes, name: &str) -> Result<&'a Members, String> {
    ts.interfaces
        .get(name)
        .or_else(|| ts.inline.get(name))
        .ok_or_else(|| format!("api.ts declares no `{name}` (interface or inline object type)"))
}

fn cells_agree(rust: &[BTreeSet<Kind>], ts: &[BTreeSet<Kind>]) -> bool {
    rust == ts
}

fn kinds(set: &BTreeSet<Kind>) -> String {
    format!("{set:?}")
}

/// The keys of `status.baseline` from a session with a baseline set and a longer task since (the
/// state with no baseline writes `null`, which has no keys to compare).
fn baseline_shape() -> Shape {
    let layout = app::outline::demo_layout(60);
    let outline = app::outline::Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
        .expect("the demo layout has a valid outline");
    let timeline = timeline::Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let mut session = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        || 0.0,
    )
    .with_outline(outline)
    .expect("the outline matches the demo schedule");
    session
        .edit(app::edit::Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let task = session.outline().unwrap().preorder()[1];
    let minutes = session.schedule().tasks()[task.0 as usize].duration + 480;
    let edit = app::edit::Edit::SetDuration { task, minutes };
    session.edit(edit).expect("a longer duration applies");
    let status: serde_json::Value =
        serde_json::from_str(&session.status_json(&app::RenderFacts::default()))
            .expect("status_json is valid JSON");
    let keys: BTreeSet<String> = status["baseline"]
        .as_object()
        .expect("a baseline in force is written as an object")
        .keys()
        .cloned()
        .collect();
    Shape {
        any: keys.clone(),
        always: Some(keys),
    }
}

/// Every difference between the captures and `api.ts`, as messages. Panics when the reader meets
/// a form it does not know (that fails the test too).
fn problems(rust: &Captured, api_ts: &str) -> Vec<String> {
    let ts = ts_side::read(api_ts);
    let mut out = Vec::new();

    let mut pairs: Vec<(&str, &Shape)> = rust.status.iter().map(|(n, s)| (*n, s)).collect();
    pairs.push(("TaskDetails", &rust.task_details));
    let baseline = baseline_shape();
    pairs.push(("BaselineSummary", &baseline));
    for (name, shape) in pairs {
        match members(&ts, name) {
            Ok(m) => out.extend(diff(name, shape, m)),
            Err(e) => out.push(e),
        }
    }

    // The status members that hold another shape must name it, or the comparison above would
    // check a shape the status never carries.
    if let Ok(status) = members(&ts, "Status") {
        for (key, ty) in [
            ("hover", "Hover"),
            ("drag", "Drag"),
            ("selected", "Selected"),
            ("link", "Link"),
            ("baseline", "BaselineSummary"),
            ("moved", "Moved"),
            ("slide", "SlideStatus"),
        ] {
            if !status.get(key).is_some_and(|m| m.ty.contains(ty)) {
                out.push(format!("Status.{key} is not typed `{ty}` in api.ts"));
            }
        }
        if !status
            .get("lookahead")
            .is_some_and(|m| m.ty.starts_with('{'))
        {
            out.push("Status.lookahead is not an inline object type in api.ts".to_string());
        }
    }

    if !cells_agree(&rust.table_row, &ts.table_row) {
        out.push(format!(
            "TableRow: json.rs writes cells {:?}, api.ts declares {:?}",
            rust.table_row.iter().map(kinds).collect::<Vec<_>>(),
            ts.table_row.iter().map(kinds).collect::<Vec<_>>()
        ));
    }
    out
}

#[test]
fn rust_and_typescript_declare_the_same_json_shapes() {
    let found = problems(&capture(), &read(API_TS));
    assert!(
        found.is_empty(),
        "the JSON contract is broken:\n{}",
        found.join("\n")
    );
}

/// Whether the comparison notices: it reports a difference, or the reader refuses the text.
fn noticed(rust: &Captured, api_ts: &str) -> bool {
    catch_unwind(AssertUnwindSafe(|| !problems(rust, api_ts).is_empty())).unwrap_or(true)
}

/// `text` with every `from` replaced by `to`; fails if `from` is not there, so a mutation
/// that stopped matching cannot pass as "the comparison noticed nothing wrong".
fn mutate(text: &str, from: &str, to: &str) -> String {
    assert!(
        text.contains(from),
        "the mutation target {from:?} is not in the file any more"
    );
    text.replace(from, to)
}

/// `shape` with the key `from` renamed to `to`, in both of its sets.
fn rename(shape: &mut Shape, from: &str, to: &str) {
    assert!(shape.any.remove(from), "the capture has no key {from}");
    shape.any.insert(to.to_string());
    if let Some(always) = &mut shape.always
        && always.remove(from)
    {
        always.insert(to.to_string());
    }
}

/// `shape` with `key` no longer always present (or, if it was not, always present).
fn flip_always(shape: &mut Shape, key: &str) {
    let always = shape.always.as_mut().expect("a captured shape");
    if !always.remove(key) {
        assert!(shape.any.contains(key), "the capture has no key {key}");
        always.insert(key.to_string());
    }
}

#[test]
fn mutations_of_a_captured_key_set_are_noticed() {
    type Mutation = (&'static str, fn(&mut Captured));
    let (rust, api_ts) = (capture(), read(API_TS));
    assert!(
        !noticed(&rust, &api_ts),
        "the unmutated captures already disagree"
    );
    let mutations: [Mutation; 12] = [
        ("Status.rowCount renamed", |c| {
            rename(c.status.get_mut("Status").unwrap(), "rowCount", "rowCnt");
        }),
        ("Status.lookahead.on renamed", |c| {
            rename(
                c.status.get_mut("Status.lookahead").unwrap(),
                "on",
                "enabled",
            );
        }),
        ("SlideStatus.copyMs renamed", |c| {
            rename(c.status.get_mut("SlideStatus").unwrap(), "copyMs", "copy");
        }),
        ("Hover.float renamed", |c| {
            rename(c.status.get_mut("Hover").unwrap(), "float", "flt");
        }),
        ("Drag.recomputeMs renamed", |c| {
            rename(
                c.status.get_mut("Drag").unwrap(),
                "recomputeMs",
                "recompute",
            );
        }),
        ("Selected.backCount renamed", |c| {
            rename(
                c.status.get_mut("Selected").unwrap(),
                "backCount",
                "backCnt",
            );
        }),
        ("Selected.forwardCapped renamed", |c| {
            rename(
                c.status.get_mut("Selected").unwrap(),
                "forwardCapped",
                "forwardCap",
            );
        }),
        ("TaskDetails.floatText renamed", |c| {
            rename(&mut c.task_details, "floatText", "floatTxt");
        }),
        ("TaskDetails.successors renamed", |c| {
            rename(&mut c.task_details, "successors", "followers");
        }),
        ("TaskDetails.summary always written", |c| {
            flip_always(&mut c.task_details, "summary");
        }),
        ("a table cell takes another kind", |c| {
            c.table_row[3].insert(Kind::Bool);
        }),
        ("a table row loses a cell", |c| {
            c.table_row.pop();
        }),
    ];
    for (name, mutation) in mutations {
        let mut mutated = rust.clone();
        mutation(&mut mutated);
        assert!(noticed(&mutated, &api_ts), "{name} went unnoticed");
    }
}

/// Whether a reader refuses the text by panicking, the fail-closed outcome.
fn refuses(read: impl FnOnce()) -> bool {
    catch_unwind(AssertUnwindSafe(read)).is_err()
}

/// The `api.ts` reader: a name declared twice is refused, not merged by overwriting.
#[test]
fn ts_reader_refuses_a_duplicate_interface() {
    let api_ts = read(API_TS);
    assert!(
        !refuses(|| drop(ts_side::read(&api_ts))),
        "api.ts is refused unmutated"
    );
    let twice = format!("{api_ts}\n{api_ts}");
    assert!(
        refuses(|| drop(ts_side::read(&twice))),
        "the reader accepted every interface declared twice"
    );
}

#[test]
fn mutations_in_api_ts_are_noticed() {
    let (rust, api_ts) = (capture(), read(API_TS));
    for (from, to) in [
        ("rowCount: number", "rowCnt: number"),
        (
            "{ on: boolean; start: string",
            "{ on: boolean; begin: string",
        ),
        ("float?: number", "flt?: number"),
        ("copyMs: number | null", "copy: number | null"),
        ("recomputeMs: number", "recompute: number"),
        ("backCount: number", "backCnt: number"),
        ("forwardCapped: boolean", "forwardCap: boolean"),
        ("floatText: string", "floatTxt: string"),
        ("successors: string[]", "followers: string[]"),
        ("summary?: boolean", "summary: boolean"),
        ("float?: number", "float: number"),
        ("criticalOnly: boolean", "criticalOnly?: boolean"),
        (
            "number | null, boolean, 0 | 1 | 2,",
            "number, boolean, 0 | 1 | 2,",
        ),
        (
            "[number, string, string,",
            "[number, string, string, string,",
        ),
        ("hover: Hover | null", "hover: Pointer | null"),
        ("finish_days: number", "finish_day: number"),
        ("removed: number", "dropped: number"),
        ("baseline: BaselineSummary | null", "baseline: Moved | null"),
    ] {
        assert!(
            noticed(&rust, &mutate(&api_ts, from, to)),
            "api.ts: {from} -> {to} went unnoticed"
        );
    }
}
