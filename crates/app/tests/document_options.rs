//! A schedule document keeps the schedule options (decision 0018): one `options` key that holds
//! only the members that differ from their defaults, and no key for a schedule on defaults.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use super::document_round_trip::{refusal, session_of};
use app::{
    document::{Extras, read, write},
    load_bytes,
};
use engine::{ScheduleOptions, StartedLateStart, UnstartedRemaining};
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

/// The text of a four-activity document saved under `options`.
fn saved(options: ScheduleOptions) -> String {
    let xer = synth_xer(&Spec::with_activities(7, 4));
    let mut loaded = load_bytes(&xer, &ProjectSelection::Unspecified, &|| 0.0).unwrap();
    loaded.schedule.set_options(options);
    let extras = Extras {
        name: "document",
        notes: &[],
        source: None,
    };
    write(&session_of(loaded), &extras)
}

#[test]
fn both_options_survive_a_save_and_a_reopen() {
    let set = ScheduleOptions {
        started_late_start: StartedLateStart::Project,
        unstarted_remaining: UnstartedRemaining::On,
    };
    let text = saved(set);
    assert!(text.ends_with(
        ",\"options\":{\"startedLateStart\":\"project\",\"unstartedRemaining\":\"on\"}}"
    ));
    let loaded = read(text.as_str(), &|| 0.0).unwrap().loaded;
    assert_eq!(loaded.schedule.options(), set);
    let one = ScheduleOptions {
        unstarted_remaining: UnstartedRemaining::On,
        ..ScheduleOptions::default()
    };
    assert!(saved(one).ends_with(",\"options\":{\"unstartedRemaining\":\"on\"}}"));
}

#[test]
fn a_schedule_on_the_defaults_writes_no_options_key() {
    let text = saved(ScheduleOptions::default());
    assert!(!text.contains("options"), "{text}");
    let loaded = read(text.as_str(), &|| 0.0).unwrap().loaded;
    assert_eq!(loaded.schedule.options(), ScheduleOptions::default());
}

#[test]
fn an_unknown_member_or_value_is_refused_by_name() {
    let text = saved(ScheduleOptions::default());
    let with = |options: &str| format!("{},\"options\":{options}}}", &text[..text.len() - 1]);
    let error = |options: &str| refusal(&with(options)).to_string();
    assert!(error("{\"startedLate\":\"p6\"}").contains("options.startedLate: unknown member"));
    assert!(
        error("{\"unstartedRemaining\":\"yes\"}")
            .contains("options.unstartedRemaining: unknown value")
    );
    assert!(error("[]").contains("options: expected an object"));
}
