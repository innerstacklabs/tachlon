//! The Predecessors column reads each row's links on every drag frame: a move leaves the links
//! where they were, so it reads the same text and builds no index of every task's links.

use super::*;
use crate::edit::Edit;

#[test]
fn a_move_keeps_the_predecessors_text_and_builds_no_index() {
    let layout = outline::demo_layout(8);
    let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
    let mut s = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        test_clock,
    );
    for (i, meta) in s.meta.iter_mut().enumerate() {
        meta.code = Some(i.to_string());
    }
    let (_, link) = s
        .schedule
        .live_dependencies()
        .next()
        .expect("the demo has links");
    let (pred, task) = (link.pred, link.succ);
    let kept = |s: &Session| {
        s.predecessors
            .0
            .lock()
            .expect("unpoisoned")
            .as_ref()
            .map(|k| k.0)
    };
    let before = s.predecessors_text(task).expect("a live task with IDs");
    assert!(before.contains(&pred.0.to_string()));
    let (key, revision) = (kept(&s), s.revision());
    let es = s.solution().dates[pred.0 as usize].es;
    s.edit(Edit::Move {
        task: pred,
        snet: Some(es + 480),
    })
    .expect("a leaf moves");
    assert_ne!(s.revision(), revision, "the move is a change");
    assert_eq!(s.predecessors_text(task).as_deref(), Some(before.as_str()));
    assert_eq!(
        kept(&s),
        key,
        "no index of every task's links was built for the move"
    );
}
