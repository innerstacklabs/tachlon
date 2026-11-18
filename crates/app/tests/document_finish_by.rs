//! A schedule document keeps the project's must-finish-by date (gantt#2074): one optional
//! `must_finish_by` key, in minutes from `origin`, written only when a date is set and refused
//! when it is outside the engine's date range (decision 0020).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use super::document_round_trip::{refusal, session_of};
use app::{
    document::{Extras, read, write},
    load_bytes,
};
use engine::{Instant, MAX_DATE, MIN_DATE};
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

/// The text of a four-activity document saved with `finish_by` set.
fn saved(finish_by: Option<Instant>) -> String {
    let xer = synth_xer(&Spec::with_activities(7, 4));
    let mut loaded = load_bytes(&xer, &ProjectSelection::Unspecified, &|| 0.0).unwrap();
    loaded.schedule.set_finish_by(finish_by).unwrap();
    let extras = Extras {
        name: "document",
        notes: &[],
        source: None,
    };
    write(&session_of(loaded), &extras)
}

#[test]
fn the_date_survives_a_save_and_a_reopen() {
    let text = saved(Some(30_000));
    assert!(text.ends_with(",\"must_finish_by\":30000}"));
    let loaded = read(text.as_str(), &|| 0.0).unwrap().loaded;
    assert_eq!(loaded.schedule.finish_by(), Some(30_000));
    assert!(!saved(None).contains("must_finish_by"));
}

#[test]
fn a_date_out_of_range_or_not_a_number_is_refused_by_name() {
    let text = saved(None);
    let with = |value: &str| format!("{},\"must_finish_by\":{value}}}", &text[..text.len() - 1]);
    let error = |value: &str| refusal(&with(value)).to_string();
    assert!(
        error(&(MAX_DATE + 1).to_string()).contains("must_finish_by: the schedule has a must-finish-by date of 527040001, outside what the engine accepts")
    );
    assert!(
        error(&(MIN_DATE - 1).to_string()).contains("must_finish_by: the schedule has a must-finish-by date of -527040001, outside what the engine accepts")
    );
    assert!(error("\"soon\"").contains("must_finish_by: expected an integer"));
    assert!(error("null").contains("must_finish_by: expected an integer"));
    assert!(error("1,\"must_finish_by\":2").contains("must_finish_by: given twice"));
}
