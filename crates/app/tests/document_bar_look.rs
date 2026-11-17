//! A task keeps its color and label in the document (gantt#1875): the reader takes them, the
//! writer writes them only when set, the bar takes the color and shows the label, and the table
//! keeps the name.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use super::document_round_trip::{refusal, session_of};
use app::{
    Session,
    document::{Extras, read, write},
    load_bytes,
};
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

const LOOK: &str = r##","label":"Pour \"A\"","color":"#1e90ff""##;

fn text_of(session: &Session) -> String {
    let extras = Extras {
        name: "look",
        notes: &[],
        source: None,
    };
    write(session, &extras)
}

fn frame(session: &Session) -> timeline::Scene {
    session.frame_scene(|_, text, px| text.len() as f32 * px * 0.5)
}

/// A four-activity document whose first bar off the critical path carries a label and a color,
/// and that task's name.
fn document() -> (String, String) {
    let xer = synth_xer(&Spec::with_activities(7, 4));
    let session = session_of(load_bytes(&xer, &ProjectSelection::Unspecified, &|| 0.0).unwrap());
    let dates = &session.solution().dates;
    let task = frame(&session)
        .bar_labels
        .iter()
        .map(|l| l.task)
        .find(|&t| !dates[t as usize].is_critical())
        .expect("a named bar off the critical path");
    let name = session.task_label(task).to_string();
    let key = format!("\"name\":\"{name}\"");
    let text = text_of(&session).replacen(&key, &format!("{key}{LOOK}"), 1);
    (text, name)
}

#[test]
fn a_task_keeps_its_label_and_color_through_a_read_and_a_write() {
    let (text, _) = document();
    assert!(text.contains(LOOK));
    let loaded = read(text.as_str(), &|| 0.0).unwrap().loaded;
    let meta = loaded.meta.iter().find(|m| m.label.is_some()).unwrap();
    assert_eq!(meta.color, Some([0x1e, 0x90, 0xff, 255]));
    assert_eq!(text_of(&session_of(loaded)), text);
}

#[test]
fn the_bar_shows_the_label_in_the_color_and_the_table_keeps_the_name() {
    let (text, name) = document();
    let loaded = read(text.as_str(), &|| 0.0).unwrap().loaded;
    let task = loaded.meta.iter().position(|m| m.label.is_some()).unwrap() as u32;
    let session = session_of(loaded);
    let scene = frame(&session);
    let label = scene.bar_labels.iter().find(|l| l.task == task).unwrap();
    assert_eq!(label.text, "Pour \"A\"");
    let blue = [30.0 / 255.0, 144.0 / 255.0, 1.0, 1.0];
    assert!(scene.bar_colored.contains(&(task, blue)));
    assert_eq!(session.task_label(task), name);
}

#[test]
fn a_blank_label_shows_the_name() {
    let (text, name) = document();
    for blank in ["", " \\t "] {
        let blank = text.replacen(
            r#""label":"Pour \"A\"""#,
            &format!(r#""label":"{blank}""#),
            1,
        );
        let loaded = read(blank.as_str(), &|| 0.0).unwrap().loaded;
        let task = loaded.meta.iter().position(|m| m.label.is_some()).unwrap() as u32;
        assert_eq!(session_of(loaded).bar_label(task), name);
    }
}

#[test]
fn a_color_that_is_not_rrggbb_is_refused_by_name() {
    let (text, _) = document();
    for bad in ["#1e90f", "1e90ff", "#1e90fg", "#+e90ff"] {
        let error = refusal(&text.replacen("#1e90ff", bad, 1)).to_string();
        assert!(error.contains("color: expected \"#rrggbb\""), "{error}");
    }
}
