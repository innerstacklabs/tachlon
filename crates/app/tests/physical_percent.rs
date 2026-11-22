//! A task's physical percent survives a save (tl#2080): a document writes `physical_percent` only
//! for a task that has one, reads it back, and refuses one over 100 percent.

use super::document_round_trip::{refusal, session_of};
use app::document::{Extras, read, write};
use app::load_bytes;
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

/// A saved document of six activities in which `A1-1` holds `physical`.
fn saved(physical: Option<u16>) -> String {
    let xer = synth_xer(&Spec::with_activities(7, 6));
    let mut loaded = load_bytes(&xer, &ProjectSelection::Unspecified, &|| 0.0).expect("loads");
    loaded
        .meta
        .iter_mut()
        .find(|meta| meta.code.as_deref() == Some("A1-1"))
        .expect("A1-1 is a task")
        .physical = physical;
    let extras = Extras {
        name: "physical",
        notes: &[],
        source: None,
    };
    write(&session_of(loaded), &extras)
}

#[test]
fn a_physical_percent_is_kept_through_save_and_reopen() {
    let document = read(saved(Some(1250)), &|| 0.0).expect("reads");
    let task = document
        .loaded
        .meta
        .iter()
        .position(|meta| meta.code.as_deref() == Some("A1-1"))
        .expect("A1-1 is a task");
    assert_eq!(document.loaded.meta[task].physical, Some(1250));
    let reopened = session_of(document.loaded);
    let task = u32::try_from(task).expect("a small index");
    assert_eq!(reopened.task_physical(task), "12.5%");
}

#[test]
fn a_document_with_no_physical_percent_has_none_of_the_key() {
    assert!(!saved(None).contains("physical_percent"));
}

#[test]
fn a_physical_percent_over_one_hundred_percent_is_refused() {
    let text = saved(Some(1250)).replace("\"physical_percent\":1250", "\"physical_percent\":10001");
    assert!(
        refusal(&text)
            .to_string()
            .contains("physical_percent: 10001 is over 10000")
    );
}
