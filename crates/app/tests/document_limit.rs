//! A schedule document keeps each task's date limits (gantt#1875): `snlt`, `fnet`, `fnlt`, one
//! pin and `alap`, in a `limit` object that holds only the keys that are set.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use super::document_round_trip::{refusal, session_of};
use app::{
    Session,
    document::{Extras, read, write},
    load_bytes,
};
use engine::{Limit, Pin, TaskId};
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

/// The text of a four-activity document; its first two tasks hold a limit when `limits` is set.
fn saved(limits: bool) -> String {
    let xer = synth_xer(&Spec::with_activities(7, 4));
    let mut loaded = load_bytes(&xer, &ProjectSelection::Unspecified, &|| 0.0).unwrap();
    if limits {
        let all = Limit {
            snlt: Some(9000),
            fnet: Some(9100),
            fnlt: Some(9200),
            pin: Some(Pin::Finish(9300)),
            alap: true,
            resume: Some(9500),
        };
        let pin = Limit {
            pin: Some(Pin::Start(9400)),
            ..Limit::default()
        };
        loaded.schedule.set_limit(TaskId(0), all).unwrap();
        loaded.schedule.set_limit(TaskId(1), pin).unwrap();
        loaded.solution = loaded.schedule.compute().unwrap();
    }
    write_text(&session_of(loaded))
}

fn write_text(session: &Session) -> String {
    let extras = Extras {
        name: "limits",
        notes: &[],
        source: None,
    };
    write(session, &extras)
}

#[test]
fn a_limit_is_read_back_and_written_as_it_was() {
    let text = saved(true);
    let all = "\"limit\":{\"snlt\":9000,\"fnet\":9100,\"fnlt\":9200,\"pin_finish\":9300,\"resume\":9500,\"alap\":true}";
    assert!(text.contains(all) && text.contains("\"limit\":{\"pin_start\":9400}"));
    let loaded = read(text.as_str(), &|| 0.0).unwrap().loaded;
    let limits: Vec<Limit> = loaded.schedule.tasks().iter().map(|t| t.limit).collect();
    assert_eq!(limits[0].snlt, Some(9000));
    assert_eq!(limits[0].pin, Some(Pin::Finish(9300)));
    assert!(limits[0].alap);
    assert_eq!(limits[1].pin, Some(Pin::Start(9400)));
    assert!(limits[2..].iter().all(|l| *l == Limit::default()));
    assert_eq!(write_text(&session_of(loaded)), text);
}

#[test]
fn a_task_with_no_limit_writes_no_limit_key() {
    assert!(!saved(false).contains("\"limit\""));
}

#[test]
fn two_pin_keys_and_a_misspelt_key_are_refused_by_name() {
    let text = saved(true);
    let two = text.replacen(
        "\"pin_start\":9400",
        "\"pin_start\":9400,\"pin_finish\":9500",
        1,
    );
    assert!(
        refusal(&two)
            .to_string()
            .contains("limit.pin_finish: only one pin key")
    );
    let misspelt = text.replacen("\"snlt\"", "\"snlat\"", 1);
    assert!(
        refusal(&misspelt)
            .to_string()
            .contains("limit.snlat: unknown key")
    );
    let text_value = text.replacen("\"fnet\":9100", "\"fnet\":\"x\"", 1);
    assert!(
        refusal(&text_value)
            .to_string()
            .contains("limit.fnet: expected an integer")
    );
}
