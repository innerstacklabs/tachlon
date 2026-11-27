//! `Session::set_locale`'s `texts` (gantt#1875): an edit the chart refuses reads in that chart's
//! words, two charts on one page speak two languages, and the `kind` the host's code reads does not
//! change with them.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::{common, link_gesture, start_edit};

use app::{Session, edit::Edit};
use engine::TaskId;

/// A locale of the demo names with `texts` laid over the English sentences.
fn locale(texts: &str) -> String {
    format!(
        r#"{{"months":["a","b","c","d","e","f","g","h","i","j","k","l"],
        "weekdays":["a","b","c","d","e","f","g"],"weekStart":0,"texts":{texts}}}"#
    )
}

/// The refusal of an empty name: its `kind`, the chart's words, and the English `Display`.
fn refuse_empty_name(session: &mut Session) -> (&'static str, String, String) {
    let error = session
        .edit(Edit::Rename {
            task: TaskId(0),
            name: String::new(),
        })
        .unwrap_err();
    (error.kind(), session.say(&error), error.to_string())
}

#[test]
fn two_charts_refuse_the_same_edit_each_in_its_own_words() {
    let (mut german, _) = common::demo_session(10);
    let (mut french, _) = common::demo_session(10);
    german
        .set_locale(&locale(r#"{"emptyName":"Ein Name darf nicht leer sein"}"#))
        .unwrap();
    french
        .set_locale(&locale(r#"{"emptyName":"un nom ne peut pas être vide"}"#))
        .unwrap();

    let english = "a name cannot be empty";
    assert_eq!(
        refuse_empty_name(&mut german),
        (
            "emptyName",
            "Ein Name darf nicht leer sein".into(),
            english.into()
        )
    );
    assert_eq!(
        refuse_empty_name(&mut french),
        (
            "emptyName",
            "un nom ne peut pas être vide".into(),
            english.into()
        )
    );
}

#[test]
fn a_sentence_the_host_translates_gets_its_values() {
    let (mut session, _) = common::demo_session(10);
    session
        .set_locale(&locale(r#"{"unknownTask":"keine Aufgabe {task}"}"#))
        .unwrap();
    let error = session
        .edit(Edit::Rename {
            task: TaskId(9999),
            name: "x".into(),
        })
        .unwrap_err();
    assert_eq!(session.say(&error), "keine Aufgabe task 9999");
}

#[test]
fn a_texts_that_is_not_a_table_of_known_sentences_changes_nothing() {
    let (mut session, _) = common::demo_session(10);
    session
        .set_locale(&locale(r#"{"emptyName":"vide"}"#))
        .unwrap();
    for bad in [r#"{"noSuchKey":"x"}"#, r#"{"emptyName":3}"#, r#""text""#] {
        assert!(session.set_locale(&locale(bad)).is_err(), "{bad}");
    }
    assert_eq!(refuse_empty_name(&mut session).1, "vide");
}

#[test]
fn the_english_table_is_what_a_host_hands_back_untranslated() {
    let table = app::english_json();
    let keys: serde_json::Value = serde_json::from_str(&table).unwrap();
    assert_eq!(keys.as_object().unwrap().len(), 165);
    assert!(include_str!("../../../docs/WASM_API.md").contains("165 in all"));
    let (mut session, _) = common::demo_session(10);
    session
        .set_locale(&locale(r#"{"emptyName":"vide"}"#))
        .unwrap();
    assert_eq!(refuse_empty_name(&mut session).1, "vide", "the host's word");
    session.set_locale(&locale(&table)).unwrap();
    let (kind, said, english) = refuse_empty_name(&mut session);
    assert_eq!((kind, said), ("emptyName", english));
}

#[test]
fn two_charts_refuse_a_link_each_in_its_own_words() {
    let mut charts = ["hängt an", "est lié à"].map(|word| {
        let mut chart = link_gesture::session();
        let texts = format!(r#"{{"alreadyLinked":"{{pred}} {word} {{succ}}"}}"#);
        chart.set_locale(&locale(&texts)).unwrap();
        (word, chart)
    });
    for (word, chart) in &mut charts {
        let to = link_gesture::on(chart, 1);
        link_gesture::link(chart, 0, to);
        assert_eq!(chart.link_refusal(), Some(format!("A {word} B").as_str()));
    }
}

#[test]
fn a_task_dialog_refusal_reads_in_the_hosts_words() {
    let mut session = start_edit::session();
    let texts = r#"{"durationInvalid":"keine Dauer","percentAlone":"allein ändern"}"#;
    session.set_locale(&locale(texts)).unwrap();
    let mut refused = |fields: &[(&str, &str)]| session.set_task_fields(TaskId(0), fields);
    assert_eq!(
        refused(&[("duration", "soon")]),
        Err("Duration: keine Dauer".into())
    );
    let both = [("percent", "5"), ("duration", "1d")];
    assert_eq!(refused(&both), Err("Percent: allein ändern".into()));
}
