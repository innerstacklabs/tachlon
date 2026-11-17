//! `Session::frame_scene` (gs#482): the one place a frame's draw list is composed. `App::frame`
//! compiles for wasm32 only, so these tests are the native check of what it draws.

use crate::common;

use app::Session;
use common::{MINUTES_PER_PX, demo_session, zoom_to};
use engine::LinkType;
use timeline::Scene;

/// A fixed label width: the renderer supplies the real one.
fn measure(_task: u32, text: &str, px: f32) -> f32 {
    text.len() as f32 * px * 0.5
}

/// A 1000 x 400 session over `demo(200)`, zoomed to 100 minutes per pixel at the first row,
/// where the bars are wide enough to carry names.
fn fixture() -> Session {
    let (mut session, _) = demo_session(200);
    let t0 = session
        .solution()
        .dates
        .iter()
        .map(|d| d.es)
        .min()
        .expect("tasks") as f64;
    zoom_to(&mut session, MINUTES_PER_PX, t0);
    session
}

/// The four calls `App::frame` made before `frame_scene` existed, by hand: the scene, the arrows,
/// the move outlines, the comparison bars and link overlay, then the bar labels.
fn by_hand(session: &Session) -> Scene {
    let mut scene = session.scene();
    let timeline = session.timeline();
    timeline::scene::add_arrows_highlighted(
        &mut scene,
        &timeline.viewport,
        timeline.dpr,
        &session.solution().dates,
        session.links(),
        session.highlight(),
        session.selected_link_drawn(),
        session.theme(),
    );
    session.add_move_outlines(&mut scene);
    session.add_baseline_bars(&mut scene);
    session.add_link_overlay(&mut scene);
    timeline::labels::add_bar_labels(
        &mut scene,
        &timeline.viewport,
        timeline.dpr,
        timeline::labels::Rows {
            order: timeline.order(),
            dates: &session.solution().dates,
            summaries: timeline.summaries(),
            highlight: session.highlight(),
            theme: session.theme(),
            terminal_reach: session.terminal_reach(),
            rings: &[],
        },
        |task| session.task_label(task),
        measure,
    );
    scene
}

fn assert_composed(session: &Session) {
    let framed = session.frame_scene(measure);
    let expected = by_hand(session);
    // The fixture must draw each part, or the equality below checks nothing. They are read from
    // the hand composition, so that a pass `frame_scene` drops fails the equality, not this.
    assert!(expected.bars > 0, "no bars");
    assert!(expected.arrows > 0, "no arrows");
    assert!(!expected.bar_labels.is_empty(), "no bar labels");
    assert_eq!(framed.rects.len(), expected.rects.len(), "rect count");
    assert_eq!(framed.rects, expected.rects, "rects");
    assert_eq!(framed.bar_labels, expected.bar_labels, "labels");
    assert_eq!(framed, expected);
}

#[test]
fn the_scene_equals_the_hand_composition() {
    assert_composed(&fixture());
}

#[test]
fn a_selected_task_dims_the_rest_the_same_way() {
    let mut session = fixture();
    let rows = session.timeline().viewport.visible_rows();
    let task = session.timeline().order()[rows.start + 3];
    session.select(task);
    session.refresh_selection();
    assert!(session.highlight().is_some(), "the selection highlights");
    assert_composed(&session);
}

#[test]
fn a_bars_right_end_is_its_outgoing_arrows_start() {
    let session = fixture();
    let scene = session.frame_scene(measure);
    let timeline = session.timeline();
    let placed = session
        .links()
        .visible(&timeline.viewport, &session.solution().dates);
    let first_bar = scene.bar_rects.iter().map(|&(_, i)| i).min().expect("bars");
    assert!(first_bar > scene.bar_start, "no arrows under the bars");
    let mut checked = 0;
    for p in placed
        .iter()
        .filter(|p| p.link.kind == LinkType::FS && p.xa >= 0.0 && p.xa < timeline.viewport.width)
    {
        let Some(&(_, bar)) = scene.bar_rects.iter().find(|&&(t, _)| t == p.link.pred) else {
            continue;
        };
        let bar = scene.rects[bar];
        let right = bar.pos[0] + bar.size[0];
        let centre = bar.pos[1] + bar.size[1] / 2.0;
        // Both snap to whole pixels (dpr 1): the arrow starts at the rounded end, and the drawn bar
        // ends within a pixel of it.
        let (_, end) = timeline::scene::bar_extent(
            &timeline.viewport,
            &session.solution().dates[p.link.pred as usize],
        );
        assert!(
            (f64::from(right) - end).abs() <= 1.0,
            "task {}: the bar ends at {right}, its extent at {end}",
            p.link.pred
        );
        // The arrow's first segment leaves that end at the row centre. Arrows sit in the draw list
        // from `bar_start` up to the first bar.
        let leaves = scene.rects[scene.bar_start..first_bar].iter().any(|r| {
            f64::from(r.pos[0]) == end.round()
                && r.size[1] <= 1.5
                && (r.pos[1] + r.size[1] / 2.0 - centre).abs() <= 1.0
        });
        assert!(
            leaves,
            "no arrow leaves the right end ({end}) of task {}",
            p.link.pred
        );
        checked += 1;
    }
    assert!(checked > 0, "the fixture shows no finish-to-start link");
}
