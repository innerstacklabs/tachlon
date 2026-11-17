//! gs#865: after opening a P6 file, the rows under each WBS node read in ascending order of the
//! Start column the page shows (the engine's start), not of the start the file states.

use crate::common;

use app::{Loaded, load_bytes};
use common::clock;
use engine::TaskId;
use import::ProjectSelection;

/// The fixture the web specs open: `xer::synth::Spec::small(3)` with a data date in January 2024.
fn fixture() -> Loaded {
    let bytes = include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");
    load_bytes(bytes, &ProjectSelection::Unspecified, &clock).expect("fixture loads")
}

/// The Start column's value for `task`, as minutes from the origin.
fn start(opened: &Loaded, task: TaskId) -> i64 {
    opened.solution.dates[task.0 as usize].es
}

#[test]
fn activities_under_each_wbs_node_never_start_before_the_row_above() {
    let opened = fixture();
    let mut compared = 0;
    for &parent in opened.outline.preorder() {
        let activities: Vec<TaskId> = opened
            .outline
            .children(parent)
            .iter()
            .copied()
            .filter(|task| opened.meta[task.0 as usize].code.is_some())
            .collect();
        for pair in activities.windows(2) {
            compared += 1;
            assert!(
                start(&opened, pair[0]) <= start(&opened, pair[1]),
                "under {:?}: {} starts at {} then {} at {}",
                opened.meta[parent.0 as usize].name,
                opened.meta[pair[0].0 as usize]
                    .code
                    .as_deref()
                    .unwrap_or("?"),
                start(&opened, pair[0]),
                opened.meta[pair[1].0 as usize]
                    .code
                    .as_deref()
                    .unwrap_or("?"),
                start(&opened, pair[1]),
            );
        }
    }
    assert!(
        compared >= 30,
        "the fixture holds many activities per node: {compared}"
    );
}

#[test]
fn an_msproject_file_keeps_its_tasks_in_file_order() {
    let bytes = include_bytes!("fixtures/baselined.xml");
    let opened = load_bytes(bytes, &ProjectSelection::Unspecified, &clock).expect("loads");
    let ids: Vec<u32> = opened
        .outline
        .preorder()
        .iter()
        .map(|task| task.0)
        .collect();
    assert_eq!(ids, (0..ids.len() as u32).collect::<Vec<_>>());
}
