//! `load_bytes`: the activity ID (`meta.code`) of each opened task (gs#87 PR 1).

use crate::common;

use app::find::matches;
use common::xer_text::{loaded, rewrite, text};
use import::{ProjectSelection, parse_xer};
use xer::synth::{Spec, write};

#[test]
fn meta_code_is_the_p6_activity_id_for_xer_and_none_for_mspdi() {
    let spec = Spec::small(7);
    let opened = loaded(&write(&spec), &ProjectSelection::Unspecified);
    let imported = parse_xer(&write(&spec), &ProjectSelection::Unspecified).expect("import");

    let mut with_code = 0;
    for (meta, task) in opened.meta.iter().zip(&imported.tasks) {
        assert_eq!(meta.code, task.code);
        if meta.code.is_some() {
            with_code += 1;
        }
    }
    assert_eq!(with_code, spec.activities);
    let outline_summaries = opened.meta.len() - spec.activities;
    assert!(outline_summaries > 0);
    assert!(
        opened
            .meta
            .iter()
            .filter(|meta| meta.code.is_none())
            .count()
            == outline_summaries,
        "WBS summaries carry no activity ID"
    );
    assert!(
        opened
            .meta
            .iter()
            .filter_map(|meta| meta.code.as_deref())
            .all(|code| code.starts_with("A1-"))
    );

    for fixture in [
        include_str!("../../import/tests/fixtures/constraints.xml"),
        include_str!("../../import/tests/fixtures/holidays.xml"),
        include_str!("../../import/tests/fixtures/links.xml"),
        include_str!("../../import/tests/fixtures/summary.xml"),
    ] {
        let opened = loaded(fixture.as_bytes(), &ProjectSelection::All);
        assert!(opened.meta.iter().all(|meta| meta.code.is_none()));
    }
    assert!(app::demo(50).3.iter().all(|meta| meta.code.is_none()));
}

#[test]
fn the_id_column_text_is_the_code_and_empty_for_a_summary_and_has_codes_follows_the_file() {
    let spec = Spec::small(7);
    let opened = loaded(&write(&spec), &ProjectSelection::Unspecified);
    assert!(app::meta::has_codes(&opened.meta), "an XER file has codes");
    for meta in &opened.meta {
        match &meta.code {
            Some(code) => assert_eq!(meta.code_text(), code, "an activity shows its ID"),
            None => assert_eq!(meta.code_text(), "", "a summary row has an empty ID cell"),
        }
    }
    // The first activity's cell is the ID the importer read, not a neighbour's.
    let imported = parse_xer(&write(&spec), &ProjectSelection::Unspecified).expect("import");
    for (meta, task) in opened.meta.iter().zip(&imported.tasks) {
        assert_eq!(meta.code_text(), task.code.as_deref().unwrap_or(""));
    }

    // No codes: an MSPDI file, the demo and an empty list show no ID column and do not panic.
    let mspdi = loaded(
        include_str!("../../import/tests/fixtures/summary.xml").as_bytes(),
        &ProjectSelection::All,
    );
    assert_ne!(mspdi.meta, [] as [app::meta::TaskMeta; 0]);
    assert!(!app::meta::has_codes(&mspdi.meta), "MSPDI has no codes");
    assert!(mspdi.meta.iter().all(|meta| meta.code_text().is_empty()));
    assert!(
        !app::meta::has_codes(&app::demo(50).3),
        "the demo has no codes"
    );
    assert!(!app::meta::has_codes(&[]));
}

#[test]
fn an_activity_id_is_shown_as_the_file_wrote_it_including_odd_characters() {
    let spec = Spec::small(7);
    let text = rewrite(&text(&spec), "TASK", "task_code", |row, old| match row {
        0 => Some("  A 1/<b>&é ".to_string()),
        _ => Some(old.to_string()),
    });
    let opened = loaded(text.as_bytes(), &ProjectSelection::Unspecified);
    let codes: Vec<&str> = opened
        .meta
        .iter()
        .map(app::meta::TaskMeta::code_text)
        .filter(|code| !code.is_empty())
        .collect();
    assert_eq!(codes.len(), spec.activities);
    assert!(
        codes.iter().any(|code| code.contains("A 1/<b>&é")),
        "the ID is passed through untouched (the shell writes it with textContent): {codes:?}"
    );
}

#[test]
fn find_matches_the_activity_id_as_well_as_the_name() {
    let opened = loaded(&write(&Spec::small(7)), &ProjectSelection::Unspecified);
    let outline = Some(&opened.outline);

    let by_code = matches(&opened.meta, outline, "a1-11");
    assert_eq!(by_code.len(), 1, "A1-11 is one activity");
    assert_eq!(
        opened.meta[by_code[0] as usize].code.as_deref(),
        Some("A1-11")
    );
    assert!(
        !opened.meta[by_code[0] as usize]
            .name
            .to_lowercase()
            .contains("a1-11"),
        "the name does not hold the ID, so the match came from the code"
    );

    // The name still matches, and a query that is in neither matches nothing.
    let by_name = matches(&opened.meta, outline, "activity 11");
    assert_eq!(by_name, by_code);
    assert_eq!(
        matches(&opened.meta, outline, "no such text"),
        [] as [u32; 0]
    );

    // A prefix of the ID finds every activity of the project, in outline order.
    let all = matches(&opened.meta, outline, "A1-");
    assert_eq!(all.len(), 40);
}
