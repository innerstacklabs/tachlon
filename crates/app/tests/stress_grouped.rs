//! The grouped stress fixture (gantt#2040): the stress tasks under a root summary, coded `Phase`, so the rows group.

use crate::common::clock;
use app::{
    Session,
    outline::{Outline, stress_grouped_layout},
};
use timeline::{HIDDEN_ROW, Timeline};

#[test]
fn the_grouped_stress_fixture_groups_by_phase_into_one_header_per_value() {
    let (layout, head) = stress_grouped_layout(1000, 12);
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let session = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        clock,
    );
    let mut session = session.with_outline(outline).expect("a matching outline");
    assert!(head.0 < 1000, "the head is one of the stress tasks");
    assert!(
        session.set_group_by(Some("Phase")),
        "[grouped-accepts-phase]"
    );
    let rows = session.timeline().rows();
    let headers = rows.iter().filter(|&&row| row == HIDDEN_ROW).count();
    assert_eq!(headers, 12, "[grouped-headers]");
    assert_eq!(rows.len(), 1000 + 12, "[grouped-rows]");
}
