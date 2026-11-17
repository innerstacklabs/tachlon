//! `load_bytes`: a task name is kept byte for byte, a `" / "` in it included, and the outline path
//! is derived from the parents (gs#159 PR 7).

use crate::common;

use app::meta::outline_path;
use common::xer_text::{loaded, rewrite, text};
use import::{ProjectSelection, parse_xer};
use xer::synth::{Spec, write};

#[test]
fn an_xer_activity_and_a_wbs_name_keep_their_separator_byte_for_byte() {
    let spec = Spec::small(7);
    let plain = text(&spec);
    let activity = "Pour / cure";
    let area = "Area  /  East / 2";
    let patched = rewrite(&plain, "TASK", "task_name", |row, old| {
        (row == 3).then(|| format!("{old} / {activity}"))
    });
    let patched = rewrite(&patched, "PROJWBS", "wbs_name", |row, old| {
        (row == 1).then(|| format!("{old}{area}"))
    });
    assert_ne!(patched, plain);

    let bytes = patched.as_bytes();
    let opened = loaded(bytes, &ProjectSelection::Unspecified);
    let imported = parse_xer(bytes, &ProjectSelection::Unspecified).expect("import");
    assert_eq!(opened.meta.len(), imported.tasks.len());

    let mut with_separator = 0;
    for (index, (meta, task)) in opened.meta.iter().zip(&imported.tasks).enumerate() {
        // Nothing but a control character changes: the generated names hold none.
        assert_eq!(meta.name, task.name, "task {index}");
        if meta.name.contains(" / ") {
            with_separator += 1;
        }
        // The path is the names of the ancestors and the task, joined by the separator.
        let mut names = vec![meta.name.as_str()];
        let mut parent = meta.parent;
        while let Some(id) = parent {
            names.push(opened.meta[id.0 as usize].name.as_str());
            parent = opened.meta[id.0 as usize].parent;
        }
        names.reverse();
        assert_eq!(outline_path(&opened.meta, index), names.join(" / "));
    }
    assert!(
        with_separator >= 2,
        "the activity and the WBS name keep the separator: {with_separator}"
    );
    assert!(
        opened
            .meta
            .iter()
            .any(|meta| meta.name.ends_with(&format!(" / {activity}"))),
        "the activity name is kept whole"
    );
}

#[test]
fn a_generated_xer_loads_leaf_names_and_the_path_extends_the_parent() {
    let opened = loaded(&write(&Spec::small(7)), &ProjectSelection::Unspecified);
    let mut nested = 0;
    for (index, meta) in opened.meta.iter().enumerate() {
        let path = outline_path(&opened.meta, index);
        match meta.parent {
            None => assert_eq!(path, meta.name),
            Some(parent) => {
                nested += 1;
                let parent_path = outline_path(&opened.meta, parent.0 as usize);
                assert_eq!(path, format!("{parent_path} / {}", meta.name));
            }
        }
    }
    assert!(nested > 0, "the file nests tasks");
}
