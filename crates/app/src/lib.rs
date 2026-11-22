//! The Gantt browser app: one WebAssembly module that owns the engine's
//! [`engine::Schedule`] and the [`timeline::Timeline`] (plan D2), so per-frame reads
//! of task dates are plain Rust memory. JavaScript keeps the DOM, the events
//! and the `requestAnimationFrame` loop, and calls the small API in `web`
//! (wasm32 only).

#![cfg_attr(not(test), warn(clippy::too_many_lines))]

use timeline::Summaries;

mod bar_look;
pub mod baseline;
mod blank;
mod card_text;
pub mod check_filter;
pub mod checks;
mod clear_filters;
mod compare;
mod constraint;
mod constraint_filter;
mod critical;
mod data_date;
pub mod dates;
mod deadline;
pub mod document;
pub mod edit;
mod explain;
pub mod export;
pub mod find;
mod fit;
mod fit_project;
mod float_filter;
pub mod fly;
pub mod frame_fault;
pub mod group;
mod health;
pub mod history;
pub mod host;
mod insert_many;
mod json;
mod keep_place;
pub mod layout;
pub mod levels;
mod list_csv;
mod list_xlsx;
mod load;
mod load_order;
mod locale;
pub mod lookahead;
pub mod meta;
pub mod moved;
pub mod names;
mod nav;
mod near_critical;
mod offscreen;
pub mod open_report;
pub mod outline;
mod path_filter;
mod path_line;
mod press;
mod print;
mod progress_rollup;
mod progress_text;
mod proposal;
pub mod range;
mod range_text;
mod read;
mod related_line;
mod reparent;
mod resource_filter;
mod resources;
mod reveal;
mod row_filter;
mod rows;
mod screen_neighbor;
mod select;
mod session;
pub mod status_filter;
mod structure;
mod texts;
mod variance;
mod view;
mod what_moves;
mod whatif;

pub use compare::{CompareError, CompareReport, FILE_ONLY_MAX};
pub use fit::fitted_timeline;
pub use import::is_xer;
pub use json::{RenderFacts, quote};
pub use load::{
    Loaded, OpenError, OpenTimings, demo, load_bytes, load_bytes_with_span, parse_selection,
    timed_load_bytes,
};
pub use moved::MoveReport;
pub use offscreen::Offscreen;
pub use press::{CLICK_MAX_MS, CLICK_MAX_PX, DRAG_THRESHOLD_PX, DragInfo, Gesture};
pub use print::PageSize;
pub use reparent::Siblings;
pub use resources::{Assigned, FULL_UNITS};
pub use row_filter::RowFilter;
pub use select::{CHAIN_CAP, Selection, TaskDetails};
pub use session::Session;
pub use structure::Place;
pub use texts::{Texts, Worded, english_json};
pub use what_moves::WHAT_MOVES_MAX;
pub use whatif::{WhatIfError, WhatIfSummary};

#[cfg(target_arch = "wasm32")]
mod web;

/// Generator seed for the demo schedule (fixed, so every page load shows the same schedule).
pub const SEED: u64 = 20_261_020;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::Outline;
    use crate::press::{Before, Press};
    use crate::view::shading_calendar;
    use engine::TaskId;
    use timeline::{Theme, Timeline};
    mod before;
    mod predecessors_memo;
    mod reopen_fields;

    fn test_clock() -> f64 {
        0.0
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "an instant in a test schedule, far under 2^53 minutes, so f64 holds it exactly"
    )]
    fn summary_selection_session() -> (Session, TaskId, Vec<TaskId>, TaskId) {
        let layout = outline::demo_layout(60);
        let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
            .expect("the demo outline is valid");
        let summary = outline
            .preorder()
            .iter()
            .copied()
            .find(|&task| outline.is_summary(task) && outline.parent(task).is_some())
            .expect("the demo has a nested summary");
        let descendants = outline.subtree(summary)[1..].to_vec();
        let outside = outline.parent(summary).expect("the summary is nested");
        let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
        let mut session = Session::new(
            layout.schedule,
            layout.solution,
            timeline,
            layout.meta,
            test_clock,
        )
        .with_outline(outline)
        .expect("the outline matches its schedule");
        let row = session
            .timeline()
            .row_of_task(summary.0)
            .expect("the expanded summary has a row");
        let span = session
            .timeline()
            .summaries()
            .get(summary.0)
            .expect("the summary has a span");
        session.fit(span.0 as f64, span.1 as f64);
        session.scroll_to_row(row);
        (session, summary, descendants, outside)
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "an instant in a test schedule, far under 2^53 minutes, so f64 holds it exactly"
    )]
    fn summary_point(session: &Session, summary: TaskId) -> (f64, f64) {
        let row = session
            .timeline()
            .row_of_task(summary.0)
            .expect("the summary has a visible row");
        let (start, end) = session
            .timeline()
            .summaries()
            .get(summary.0)
            .expect("the summary has a span");
        let x = session
            .timeline()
            .viewport
            .x_of(f64::midpoint(start as f64, end as f64));
        let y =
            session.timeline().viewport.row_top(row) + session.timeline().viewport.row_height / 2.0;
        assert_eq!(
            session.timeline().hit_test(&session.solution().dates, x, y),
            Some(summary.0),
            "[summary-select] test point must hit the summary"
        );
        (x, y)
    }

    fn click_summary(session: &mut Session, summary: TaskId) {
        let (x, y) = summary_point(session, summary);
        session.pointer_down(x, y, false);
        session.pointer_up();
    }

    #[test]
    fn demo_schedule_has_rows_for_every_task_and_a_critical_path() {
        let (s, sol, order, _) = demo(10_000);
        assert_eq!(s.tasks().len(), 10_000);
        assert_eq!(order.len(), 10_000);
        let mut seen = order.clone();
        seen.sort_unstable();
        assert!(seen.iter().enumerate().all(|(i, &t)| t as usize == i));
        assert!(
            order
                .windows(2)
                .all(|w| sol.dates[w[0] as usize].es <= sol.dates[w[1] as usize].es)
        );
        let critical = sol.dates.iter().filter(|d| d.is_critical()).count();
        assert!(critical > 0 && critical < 10_000, "critical {critical}");
    }

    #[test]
    fn an_outline_sets_preorder_rows_and_summaries_toggle() {
        fn clock() -> f64 {
            0.0
        }

        let layout = outline::demo_layout(60);
        let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
            .expect("the demo outline is valid");
        let expected: Vec<u32> = outline.preorder().iter().map(|task| task.0).collect();
        let summary = outline
            .preorder()
            .iter()
            .copied()
            .find(|&task| outline.is_summary(task))
            .expect("the demo has a summary");
        let hidden = outline.subtree(summary).len() - 1;
        let leaf = outline
            .preorder()
            .iter()
            .copied()
            .find(|&task| !outline.is_summary(task))
            .expect("the demo has a leaf");
        let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
        let mut session = Session::new(
            layout.schedule,
            layout.solution,
            timeline,
            layout.meta,
            clock,
        )
        .with_outline(outline)
        .expect("the outline matches its schedule");

        assert_eq!(session.timeline().order(), expected);
        assert!(session.toggle(summary));
        assert_eq!(session.timeline().order().len(), expected.len() - hidden);
        assert!(session.is_collapsed(summary));
        assert!(session.toggle(summary));
        assert_eq!(session.timeline().order(), expected);
        assert!(!session.is_collapsed(summary));
        assert!(!session.toggle(leaf));

        let flat = outline::demo_layout(60);
        let timeline = Timeline::new(flat.order, 1000.0, 400.0, 1.0);
        let mut session = Session::new(flat.schedule, flat.solution, timeline, flat.meta, clock);
        assert!(!session.toggle(TaskId(0)));
    }

    #[test]
    fn a_short_click_on_a_summary_selects_and_highlights_its_subtree() {
        let (mut session, summary, descendants, _) = summary_selection_session();
        click_summary(&mut session, summary);

        let selected = session.selection().expect("the summary is selected");
        assert_eq!(
            selected.task, summary.0,
            "[summary-select] the summary task must be selected"
        );
        assert!(
            selected.back.is_empty(),
            "[summary-select] a summary has no driving predecessors"
        );
        assert!(
            selected.forward.is_empty(),
            "[summary-select] a summary has no driven successors"
        );
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the test schedule has 60 tasks (demo_layout(60))"
        )]
        for task in 0..session.schedule().tasks().len() as u32 {
            let in_subtree = task == summary.0 || descendants.iter().any(|item| item.0 == task);
            assert_eq!(
                selected.highlight().has_task(task),
                in_subtree,
                "[summary-select] highlight membership differs for task {task}"
            );
        }
    }

    #[test]
    fn a_moved_summary_press_pans_without_changing_the_selection() {
        let (mut session, summary, _, outside) = summary_selection_session();
        session.select(outside.0);
        let (x, y) = summary_point(&session, summary);
        let before = session.timeline().viewport.t0;

        session.pointer_down(x, y, false);
        assert!(
            session.timeline().dragging(),
            "[summary-select] a summary press must start a pan"
        );
        assert_eq!(
            session.timeline().bar_drag(),
            None,
            "[summary-select] a summary press must not start a bar drag"
        );
        session.pointer_move(x + CLICK_MAX_PX + 1.0, y);
        assert_ne!(
            session.timeline().viewport.t0,
            before,
            "[summary-select] moving a summary press must pan the viewport"
        );
        session.pointer_up();

        assert_eq!(
            session.selection().expect("the old selection remains").task,
            outside.0,
            "[summary-select] a moved summary press must not select or clear"
        );
    }

    #[test]
    fn clear_and_an_empty_click_clear_a_summary_selection() {
        let (mut session, summary, _, _) = summary_selection_session();
        click_summary(&mut session, summary);
        session.clear_selection();
        assert!(
            session.selection().is_none(),
            "[summary-select] clear_selection must clear the summary"
        );

        click_summary(&mut session, summary);
        let empty = (session.timeline().viewport.width / 2.0, 5.0);
        assert_eq!(
            session
                .timeline()
                .hit_test(&session.solution().dates, empty.0, empty.1,),
            None,
            "[summary-select] the axis-band point must be empty"
        );
        session.pointer_down(empty.0, empty.1, false);
        session.pointer_up();
        assert!(
            session.selection().is_none(),
            "[summary-select] an empty click must clear the summary"
        );
    }

    #[test]
    fn a_collapsed_summary_selection_keeps_hidden_descendants_highlighted() {
        let (mut session, summary, descendants, _) = summary_selection_session();
        assert!(
            session.toggle(summary),
            "[summary-select] the summary must collapse"
        );
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the test schedule has 60 tasks (demo_layout(60)), so a row is below 60"
        )]
        let row = session
            .timeline()
            .row_of_task(summary.0)
            .expect("the collapsed summary has a row") as u32;
        assert!(
            descendants
                .iter()
                .all(|task| session.timeline().row_of_task(task.0).is_none()),
            "[summary-select] collapsed descendants must be hidden"
        );

        click_summary(&mut session, summary);
        let selected = session.selection().expect("the summary is selected");
        assert_eq!(
            selected.row, row,
            "[summary-select] the selection row must be the summary row"
        );
        assert!(
            selected.highlight().has_task(summary.0),
            "[summary-select] the collapsed summary must be highlighted"
        );
        assert!(
            descendants
                .iter()
                .all(|task| selected.highlight().has_task(task.0)),
            "[summary-select] hidden descendants must remain highlighted"
        );
    }
}
