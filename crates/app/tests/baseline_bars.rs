//! The comparison bars in a frame (gs#703): `Session::frame_scene` draws one under each baselined
//! leaf while a baseline is set, at the baseline's dates, whatever the bars do.

use crate::common;

use app::{Loaded, Session, edit::Edit, load_bytes};
use common::xer_text::rewrite;
use engine::TaskId;
use import::ProjectSelection;
use std::cell::Cell;
use timeline::scene::{Rect, span_extent};
use timeline::{Scene, Timeline};

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

const EDITED: TaskId = TaskId(16);

thread_local! {
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    NOW.with(Cell::get)
}

fn measure(_task: u32, text: &str, px: f32) -> f32 {
    text.len() as f32 * px * 0.5
}

/// The ledger file with no actual dates (an activity with actuals is fixed at them) in a 1200 x
/// 1200 session, so all 43 rows show, fitted to the schedule.
fn ledger() -> Session {
    NOW.with(|n| n.set(0.0));
    let text = String::from_utf8(LEDGER_XER.to_vec()).expect("UTF-8");
    let text = rewrite(&text, "TASK", "act_start_date", |_, _| Some(String::new()));
    let bytes = rewrite(&text, "TASK", "act_end_date", |_, _| Some(String::new())).into_bytes();
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(&bytes, &ProjectSelection::Unspecified, &clock).expect("the file loads");
    let mut session = Session::new(
        schedule,
        solution,
        Timeline::new(order, 1200.0, 1200.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("a valid outline")
    .with_origin(origin);
    let dates = &session.solution().dates;
    let first = dates.iter().map(|d| d.es).min().expect("tasks") as f64;
    let last = dates.iter().map(|d| d.ef).max().expect("tasks") as f64;
    session.fit(first, last);
    session
}

fn strips(scene: &Scene, session: &Session) -> Vec<Rect> {
    let colour = session.theme().baseline;
    scene
        .rects
        .iter()
        .filter(|r| r.color == colour)
        .copied()
        .collect()
}

#[test]
fn the_frame_draws_comparison_bars_only_while_a_baseline_is_set() {
    let mut session = ledger();
    assert_eq!(
        strips(&session.frame_scene(measure), &session),
        [] as [timeline::scene::Rect; 0]
    );
    session
        .edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let scene = session.frame_scene(measure);
    let leaves = session
        .timeline()
        .order()
        .iter()
        .filter(|&&t| session.timeline().summaries().get(t).is_none())
        .count();
    let drawn = strips(&scene, &session).len();
    assert!(
        drawn > 0 && drawn == leaves,
        "{drawn} strips, {leaves} leaves"
    );
    // Under the bars: every strip comes before the first bar.
    let first_bar = scene.bar_rects.iter().map(|&(_, i)| i).min().expect("bars");
    let last_strip = scene
        .rects
        .iter()
        .rposition(|r| r.color == session.theme().baseline)
        .expect("strips");
    assert!(last_strip < first_bar);
    session.undo().expect("undoes");
    assert_eq!(
        strips(&session.frame_scene(measure), &session),
        [] as [timeline::scene::Rect; 0]
    );
}

#[test]
fn a_comparison_bar_stays_at_the_baseline_while_the_bar_slides() {
    let mut session = ledger();
    session
        .edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let idle = strips(&session.frame_scene(measure), &session);
    session
        .edit(Edit::SetDuration {
            task: EDITED,
            minutes: 4800,
        })
        .expect("applies");
    session.advance_motion();
    let report = session.move_report().expect("the edit moved others");
    let total = timeline::slide::total_ms(report.edited.iter().chain(&report.was).count());
    NOW.with(|n| n.set(total / 2.0));
    session.advance_motion();
    assert!(session.slide_active(), "the slide runs at the midpoint");
    let scene = session.frame_scene(measure);
    // The bars moved: some displayed date is not the baseline's.
    let baseline = session.baseline().expect("set");
    let v = &session.timeline().viewport;
    let displaced = session.display_dates().iter().enumerate().any(|(i, d)| {
        baseline
            .task(TaskId(i as u32))
            .is_some_and(|b| span_extent(v, d.es, d.ef).0 != span_extent(v, b.es, b.ef).0)
    });
    assert!(displaced, "a bar is off its baseline");
    // The strips are where the baseline put them, as before the edit.
    let now = strips(&scene, &session);
    assert_ne!(now, [] as [timeline::scene::Rect; 0]);
    let xs = |s: &[Rect]| -> Vec<(f32, f32)> { s.iter().map(|r| (r.pos[0], r.size[0])).collect() };
    assert_eq!(xs(&now), xs(&idle));
}
