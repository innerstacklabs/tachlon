//! A document is read without a copy of the whole file (gantt#1328): each task and link is built
//! as the JSON reader hands it over. These tests keep the schedule and the refusals what a read
//! of the whole tree gives, for the key orders that make the reader hold something back.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use super::document_round_trip::{changed, document, refusal};
use app::document::{Document, DocumentError, read};

const LAYOUT: [&str; 13] = [
    "format",
    "version",
    "name",
    "origin",
    "data_date",
    "lag_calendar",
    "calendars",
    "tasks",
    "links",
    "baseline",
    "view",
    "notes",
    "source",
];

/// The text of `text` with its top-level keys in the order of `keys`.
fn ordered(text: &str, keys: &[&str]) -> String {
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    let members: Vec<String> = keys
        .iter()
        .map(|key| format!("{key:?}:{}", value[*key]))
        .collect();
    format!("{{{}}}", members.join(","))
}

/// The layout with the keys of `first` moved to the front, in that order.
fn layout_with<'a>(first: &[&'a str]) -> Vec<&'a str> {
    let rest = LAYOUT.iter().filter(|key| !first.contains(key));
    first.iter().chain(rest).copied().collect()
}

/// The edited text of the round-trip document, its keys in the order the writer gives.
fn written(edit: &dyn Fn(&mut serde_json::Value)) -> String {
    ordered(&changed(edit), &LAYOUT)
}

fn same(a: &Document, b: &Document) {
    assert_eq!(a.loaded.schedule.tasks(), b.loaded.schedule.tasks());
    assert_eq!(
        a.loaded.schedule.dependencies(),
        b.loaded.schedule.dependencies()
    );
    assert_eq!(
        a.loaded.schedule.calendars().len(),
        b.loaded.schedule.calendars().len()
    );
    assert_eq!(a.loaded.meta, b.loaded.meta);
    assert_eq!(a.loaded.progress, b.loaded.progress);
    assert_eq!(a.loaded.solution, b.loaded.solution);
}

#[test]
fn links_before_the_tasks_and_calendars_after_them_read_the_same_schedule() {
    let (text, _) = document();
    let expected = read(&text, &|| 0.0).unwrap();
    assert!(expected.loaded.schedule.dependencies().len() > 1);
    for first in [
        &["links"][..],
        &["tasks", "links"],
        &["tasks", "calendars", "links"],
        &["tasks", "lag_calendar", "calendars"],
        &["links", "tasks", "calendars", "lag_calendar"],
        &["lag_calendar", "calendars", "links", "tasks"],
    ] {
        let text = ordered(&text, &layout_with(first));
        same(&read(&text, &|| 0.0).unwrap(), &expected);
    }
}

#[test]
fn a_bad_calendar_is_named_when_the_tasks_and_the_links_are_empty() {
    let calendar = |v: &mut serde_json::Value| v["calendars"][0]["week"] = serde_json::json!([]);
    let empty = |v: &mut serde_json::Value| {
        v["tasks"] = serde_json::json!([]);
        v["links"] = serde_json::json!([]);
        v["baseline"] = serde_json::Value::Null;
        v["view"]["collapsed"] = serde_json::json!([]);
    };
    let message = refusal(&written(&|v| {
        empty(v);
        calendar(v);
    }))
    .to_string();
    assert!(message.contains("calendars[0].week"), "{message}");
}

#[test]
fn a_syntax_error_after_a_bad_task_or_link_is_the_one_named() {
    let late = |edit: &dyn Fn(&mut serde_json::Value)| {
        let text = written(edit);
        let cut = &text[..text.len() - 1];
        refusal(cut)
    };
    let bad_task = late(&|v| v["tasks"][3]["duration"] = "long".into());
    assert!(matches!(bad_task, DocumentError::Syntax(_)), "{bad_task}");
    let bad_link = late(&|v| v["links"][0]["kind"] = "XX".into());
    assert!(matches!(bad_link, DocumentError::Syntax(_)), "{bad_link}");
    let bad_parent = late(&|v| v["tasks"][1]["parent"] = 99_999.into());
    assert!(
        matches!(bad_parent, DocumentError::Syntax(_)),
        "{bad_parent}"
    );
}

#[test]
fn the_first_refusal_in_reading_order_is_named_when_several_are_streamed() {
    let named = |edit: &dyn Fn(&mut serde_json::Value)| refusal(&written(edit)).to_string();
    let message = named(&|v| {
        v["calendars"][0]["week"] = serde_json::json!([]);
        v["tasks"][2]["duration"] = "long".into();
    });
    assert!(message.contains("calendars[0].week"), "{message}");
    let message = named(&|v| {
        v["tasks"][6]["duration"] = "long".into();
        v["tasks"][2]["progress"] = 101.into();
        v["links"][0]["kind"] = "XX".into();
    });
    assert!(message.contains("tasks[2].progress"), "{message}");
    let message = named(&|v| {
        v["tasks"][1]["parent"] = 99_999.into();
        v["links"][0]["kind"] = "XX".into();
    });
    assert!(message.contains("tasks[1].parent"), "{message}");
}

/// The reader borrows a string from the text and copies it only when it has an escape: a name with
/// escapes, a plain name and a key written with an escape all read as the text says.
#[test]
fn a_name_with_escapes_a_plain_name_and_an_escaped_key_read_as_written() {
    let escaped = "a \"quoted\" line\nthen \\ \u{e9} \u{1F600}";
    let named = changed(&|v| {
        v["tasks"][0]["name"] = escaped.into();
        v["tasks"][1]["name"] = "plain".into();
    });
    let meta = read(&named, &|| 0.0).unwrap().loaded.meta;
    assert_eq!((&*meta[0].name, &*meta[1].name), (escaped, "plain"));
    assert!(
        named.contains(r#""a \"quoted\" line\nthen"#),
        "the text has escapes"
    );
    let letter_a = ["\\", "u0061"].concat();
    let keyed = named.replace(r#""name":"#, &format!(r#""n{letter_a}me":"#));
    assert_ne!(keyed, named);
    assert_eq!(read(&keyed, &|| 0.0).unwrap().loaded.meta, meta);
}
