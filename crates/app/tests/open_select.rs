//! `load_bytes`: choosing a project from a multi-project file, and the project-list error
//! (gs#87 PR 1).

use crate::common;

use app::OpenError;
use common::xer_text::{FIRST_PROJECT, load, loaded, text, three_projects};
use import::{ProjectSelection, XerProject};
use xer::synth::{Spec, write};

/// Put the rows of `table` in the order `order` names (indexes into the original order).
fn reorder_rows(text: &str, table: &str, order: &[usize]) -> String {
    let mut out = String::new();
    let mut in_table = false;
    let mut rows: Vec<&str> = Vec::new();
    for line in text.split_inclusive('\n') {
        if in_table && line.starts_with("%R\t") {
            rows.push(line);
            continue;
        }
        if in_table && !line.starts_with("%F\t") {
            for &index in order {
                out.push_str(rows[index]);
            }
            rows.clear();
            in_table = false;
        }
        if line.trim_end() == format!("%T\t{table}") {
            in_table = true;
        }
        out.push_str(line);
    }
    assert!(!in_table, "{table} is not the last table");
    out
}

#[test]
fn a_multi_project_file_with_no_selection_lists_its_projects_in_file_order() {
    let spec = three_projects();
    // File order is 1002, 1000, 1001: neither ascending nor the order of the generator.
    let bytes = reorder_rows(&text(&spec), "PROJECT", &[2, 0, 1]).into_bytes();

    let Err(OpenError::ChooseProject(projects)) = load(&bytes, &ProjectSelection::Unspecified)
    else {
        panic!("three projects and no selection must ask the caller to choose");
    };
    let activities = spec.activities;
    assert_eq!(
        projects,
        vec![
            XerProject {
                proj_id: FIRST_PROJECT + 2,
                short_name: Some("Project 3".to_string()),
                activities,
            },
            XerProject {
                proj_id: FIRST_PROJECT,
                short_name: Some("Café Project 1".to_string()),
                activities,
            },
            XerProject {
                proj_id: FIRST_PROJECT + 1,
                short_name: Some("Project 2".to_string()),
                activities,
            },
        ]
    );
}

#[test]
fn the_selection_picks_one_project_all_of_them_or_fails_with_a_message() {
    let spec = three_projects();
    let bytes = write(&spec);
    let per_project = {
        let one = loaded(&write(&Spec::small(9)), &ProjectSelection::Unspecified);
        one.schedule.tasks().len()
    };

    let all = loaded(&bytes, &ProjectSelection::All);
    assert_eq!(all.schedule.tasks().len(), 3 * per_project);

    let second = loaded(&bytes, &ProjectSelection::Only(vec![FIRST_PROJECT + 1]));
    assert_eq!(second.schedule.tasks().len(), per_project);
    assert!(
        second
            .meta
            .iter()
            .filter_map(|meta| meta.code.as_deref())
            .all(|code| code.starts_with("A2-")),
        "only project 2's activities are open"
    );

    match load(&bytes, &ProjectSelection::Only(vec![4242])) {
        Err(OpenError::Message(text)) => assert!(text.contains("4242"), "{text}"),
        other => panic!(
            "an unknown project is a message, got {:?}",
            other.map(|_| ())
        ),
    }
}

#[test]
fn a_single_project_file_needs_no_selection_and_mspdi_ignores_it() {
    let one = loaded(&write(&Spec::small(5)), &ProjectSelection::Unspecified);
    assert_ne!(one.schedule.tasks(), []);

    let mspdi = include_str!("../../import/tests/fixtures/links.xml").as_bytes();
    for selection in [
        ProjectSelection::Unspecified,
        ProjectSelection::All,
        ProjectSelection::Only(vec![7]),
    ] {
        let opened = loaded(mspdi, &selection);
        assert_eq!(opened.schedule.tasks().len(), 6);
    }
}

#[test]
fn a_failed_load_is_a_value_and_the_same_bytes_load_after_it() {
    let bytes = write(&three_projects());
    let first = load(&bytes, &ProjectSelection::Unspecified);
    assert!(matches!(first, Err(OpenError::ChooseProject(_))));
    assert_ne!(first.err().expect("an error").to_string(), "");

    // Nothing was kept between the two calls.
    let a = loaded(&bytes, &ProjectSelection::All);
    let b = loaded(&bytes, &ProjectSelection::All);
    assert_eq!(a.solution, b.solution);
    assert_eq!(a.unsupported, b.unsupported);
}

#[test]
fn every_truncation_with_no_selection_is_a_schedule_or_an_error_never_a_panic() {
    let bytes = write(&three_projects());
    let step = (bytes.len() / 300).max(1);
    let mut cut = 0;
    while cut < bytes.len() {
        for selection in [ProjectSelection::Unspecified, ProjectSelection::All] {
            if let Err(error) = load(&bytes[..cut], &selection) {
                let shown = error.to_string();
                assert!(!shown.trim().is_empty(), "empty error at {cut} bytes");
            }
        }
        cut += step;
    }
}
