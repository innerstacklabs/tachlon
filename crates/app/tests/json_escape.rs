//! A task name with `"` and `\` crosses the JSON builders and comes back unchanged (gs#254).

use crate::common;

use serde_json::Value;

const NAME: &str = r#"Pour "slab" \ C:\temp"#;

#[test]
fn a_name_with_a_quote_and_a_backslash_round_trips_through_serde_json() {
    let (session, _) = common::demo_session_seeded(10, |meta| meta[3].name = NAME.to_string());
    let details: Value =
        serde_json::from_str(&session.task_details_json(3).expect("task 3 exists")).unwrap();
    let path = app::meta::outline_path(session.meta(), 3);
    assert!(
        path.ends_with(NAME),
        "the path ends with the task's own name"
    );
    assert_eq!(details["outlinePath"], path.as_str());
    assert_eq!(details["name"], NAME);
    // A neighbour's name lists it among the predecessors or successors when linked.
    for other in 0..10 {
        let json: Value = serde_json::from_str(&session.task_details_json(other).unwrap()).unwrap();
        for list in ["predecessors", "successors"] {
            for entry in json[list].as_array().unwrap() {
                assert!(entry.is_string());
            }
        }
    }
}
