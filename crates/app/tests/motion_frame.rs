//! The slide in a frame (gs#487, story S3.7 of gs#427): the bars, the arrows, the bar labels and
//! the link terminal of `Session::frame_scene` all read `Session::display_dates`, so a moved task
//! slides as one picture. `App::frame` compiles for wasm32 only; `frame_scene` is what it draws.
//! Time is a fake clock the test sets, so a frame is exact.

use crate::common;

use app::{Loaded, Session, edit::Edit, load_bytes};
use common::xer_text::rewrite;
use engine::{LinkType, TaskId};
use import::ProjectSelection;
use std::cell::Cell;
use timeline::{Scene, Timeline, labels::Placement, slide};

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

/// The fixture with no actual dates: an activity with actuals is fixed at them, and these tests
/// measure what an edit moves.
fn not_started(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).expect("UTF-8");
    let text = rewrite(&text, "TASK", "act_start_date", |_, _| Some(String::new()));
    rewrite(&text, "TASK", "act_end_date", |_, _| Some(String::new())).into_bytes()
}

/// The task whose longer duration moves 8 others (see `moved.rs`).
const EDITED: TaskId = TaskId(16);
const LONGER: i64 = 4800;

thread_local! {
    /// The fake clock. Each test runs on its own thread, so each has its own time.
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    NOW.with(Cell::get)
}

fn set_clock(ms: f64) {
    NOW.with(|now| now.set(ms));
}

/// A fixed label width: the renderer supplies the real one.
fn measure(_task: u32, text: &str, px: f32) -> f32 {
    text.len() as f32 * px * 0.5
}

/// The ledger file in a 1200 x 1200 session (all 43 rows show), zoomed so that a bar is wide enough to carry its name
/// and a slide moves a bar by several pixels.
fn ledger(reduced_motion: bool) -> Session {
    set_clock(0.0);
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(
        &not_started(LEDGER_XER),
        &ProjectSelection::Unspecified,
        &clock,
    )
    .expect("the file loads");
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
    session.set_reduced_motion(reduced_motion);
    let first = session
        .solution()
        .dates
        .iter()
        .map(|d| d.es)
        .min()
        .expect("tasks") as f64;
    let last = session
        .solution()
        .dates
        .iter()
        .map(|d| d.ef)
        .max()
        .expect("tasks") as f64;
    session.fit(first, last);
    session
}

/// The edit that moves the others, and the first frame that sees it.
fn edited(session: &mut Session) {
    session
        .edit(Edit::SetDuration {
            task: EDITED,
            minutes: LONGER,
        })
        .expect("applies");
    session.advance_motion();
}

/// The slide's length for the session's report.
fn total(session: &Session) -> f64 {
    let report = session.move_report().expect("the edit moved others");
    slide::total_ms(report.edited.iter().chain(&report.was).count())
}

/// The tasks the report moved.
fn moved(session: &Session) -> Vec<u32> {
    let report = session.move_report().expect("the edit moved others");
    report
        .edited
        .iter()
        .chain(&report.was)
        .map(|m| m.task.0)
        .collect()
}

/// The frame as `App::frame` draws it, with the oldest composition by hand: every part reads
/// `solution().dates`, which equals the display dates whenever no slide runs.
fn from_the_solution(session: &Session) -> Scene {
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

/// The bar the scene drew for `task`: its critical edge, drawn just before the body, when it has
/// one (tl#1215), else its body. A diamond (a negative radius) keeps its body: its edge grows past
/// its points.
fn drawn_bar(scene: &Scene, task: u32) -> Option<timeline::scene::Rect> {
    let &(_, index) = scene.bar_rects.iter().find(|&&(t, _)| t == task)?;
    let body = scene.rects[index];
    let edged = index > 0 && scene.bar_extras.contains(&(task, index - 1));
    Some(if edged && body.radius >= 0.0 {
        scene.rects[index - 1]
    } else {
        body
    })
}

/// The right end of the bar the scene drew for `task`.
fn bar_right(scene: &Scene, task: u32) -> Option<f32> {
    drawn_bar(scene, task).map(|rect| rect.pos[0] + rect.size[0])
}

/// The left end of the bar the scene drew for `task`.
fn bar_left(scene: &Scene, task: u32) -> Option<f32> {
    drawn_bar(scene, task).map(|rect| rect.pos[0])
}

/// Both ends of the bars of the moved tasks the scene draws, with a label beside or inside.
fn drawn_moved(scene: &Scene, moved: &[u32]) -> Vec<u32> {
    moved
        .iter()
        .copied()
        .filter(|&t| bar_right(scene, t).is_some())
        .collect()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test checks every arrow and label at the midpoint"
)]
fn at_the_midpoint_arrows_and_labels_use_the_dates_of_the_bars() {
    let mut session = ledger(false);
    let idle = session.frame_scene(measure);
    edited(&mut session);
    let tasks = moved(&session);
    set_clock(total(&session) / 2.0);
    session.advance_motion();
    assert!(session.slide_active(), "the slide runs at the midpoint");
    let scene = session.frame_scene(measure);
    let shown = drawn_moved(&scene, &tasks);
    assert!(shown.len() >= 2, "the fixture draws moved bars");

    // The slide moves the bars: the check below is not the idle picture.
    let slid = shown
        .iter()
        .filter(|&&t| {
            let then = session.solution().dates[t as usize].ef;
            let now = session.display_dates()[t as usize].ef;
            ((now - then) as f64 * session.timeline().viewport.px_per_minute).abs() > 3.0
        })
        .count();
    assert!(slid > 0, "a moved bar is at least 3 px from its new place");

    // Arrows: an arrow of a finish-to-start link leaves the drawn right end of its predecessor.
    let timeline = session.timeline();
    let first_bar = scene.bar_rects.iter().map(|&(_, i)| i).min().expect("bars");
    let mut arrows = 0;
    for p in session
        .links()
        .visible(&timeline.viewport, session.display_dates())
        .iter()
        .filter(|p| p.link.kind == LinkType::FS && tasks.contains(&p.link.pred))
    {
        let Some(right) = bar_right(&scene, p.link.pred) else {
            continue;
        };
        let &(_, index) = scene
            .bar_rects
            .iter()
            .find(|&&(t, _)| t == p.link.pred)
            .expect("a bar");
        let bar = scene.rects[index];
        let centre = bar.pos[1] + bar.size[1] / 2.0;
        let leaves = scene.rects[scene.bar_start..first_bar].iter().any(|r| {
            (r.pos[0] - right).abs() <= 1.0
                && r.size[1] <= 1.5
                && (r.pos[1] + r.size[1] / 2.0 - centre).abs() <= 1.0
        });
        assert!(
            leaves,
            "no arrow leaves the drawn right end ({right}) of task {}",
            p.link.pred
        );
        arrows += 1;
    }
    assert!(arrows > 0, "the fixture draws an arrow out of a moved bar");

    // Labels: a name sits at the left end of its drawn bar, or right of its drawn end and the terminal's
    // reach, by the padding the labels use.
    let pad = (timeline::labels::PAD_PX / 2.0) as f32;
    let reach = session.terminal_reach() as f32;
    let mut labels = 0;
    for label in &scene.bar_labels {
        let (Some(left), Some(right)) =
            (bar_left(&scene, label.task), bar_right(&scene, label.task))
        else {
            continue;
        };
        match label.placement {
            Placement::Inside => assert!(
                (label.x - (left.max(0.0) + pad)).abs() <= 1.0,
                "task {}: the name starts at {}, its bar starts at {left}",
                label.task,
                label.x
            ),
            Placement::Right => assert!(
                (label.x - (right + reach + pad)).abs() <= 1.0,
                "task {}: the name starts at {}, its bar ends at {right}",
                label.task,
                label.x
            ),
            Placement::Hidden => unreachable!("a hidden label is never stored"),
        }
        if tasks.contains(&label.task) {
            labels += 1;
        }
    }
    assert!(labels > 0, "the fixture labels a moved bar");
    assert_ne!(scene, idle, "the midpoint is not the idle picture");
}

#[test]
fn at_the_midpoint_the_link_terminal_sits_at_the_end_of_the_drawn_bar() {
    let mut session = ledger(false);
    edited(&mut session);
    let tasks = moved(&session);
    set_clock(total(&session) / 2.0);
    session.advance_motion();
    // The selected bar carries the terminal. Take a moved leaf whose bar is drawn.
    let probe = session.frame_scene(measure);
    // The one that is furthest from its new place, so that the slide shows.
    let away = |t: u32| {
        (session.display_dates()[t as usize].ef - session.solution().dates[t as usize].ef).abs()
    };
    let task = tasks
        .iter()
        .copied()
        .filter(|&t| {
            bar_right(&probe, t).is_some()
                && !session.outline().is_some_and(|o| o.is_summary(TaskId(t)))
        })
        .max_by_key(|&t| away(t))
        .expect("a moved leaf with a drawn bar");
    session.select(task);
    session.refresh_selection();
    let scene = session.frame_scene(measure);
    let right = bar_right(&scene, task).expect("the bar is drawn");
    // The terminal is the last quad of the draw list: the link overlay is the last part that adds
    // quads (the labels are text).
    let look = session.theme().link_terminal;
    let dot = *scene.rects.last().expect("quads");
    assert_eq!(dot.color, look, "the last quad is the terminal");
    // The gap between the bar's end and the dot, measured on the picture with no slide.
    let mut done = ledger(false);
    edited(&mut done);
    set_clock(1.0e9);
    done.advance_motion();
    assert!(!done.slide_active());
    done.select(task);
    done.refresh_selection();
    let idle = done.frame_scene(measure);
    let idle_right = bar_right(&idle, task).expect("the bar is drawn");
    let idle_dot = idle.rects.last().expect("quads");
    assert_eq!(idle_dot.color, look, "the last quad is the terminal");
    let gap = idle_dot.pos[0] - idle_right;
    assert!(
        (dot.pos[0] - (right + gap)).abs() <= 1.0,
        "the terminal is at {}, the drawn bar ends at {right} (a gap of {gap})",
        dot.pos[0]
    );
    assert!(
        (right - idle_right).abs() > 3.0,
        "the bar is mid-slide: it ends at {right}, and at {idle_right} when the slide is over"
    );
}

#[test]
fn at_the_end_the_frame_equals_the_idle_scene() {
    let mut session = ledger(false);
    edited(&mut session);
    set_clock(total(&session) - 1.0);
    session.advance_motion();
    assert!(
        session.slide_active(),
        "one millisecond before the end it runs"
    );
    set_clock(total(&session));
    session.advance_motion();
    assert!(!session.slide_active(), "the slide is over");
    assert_eq!(
        session.frame_scene(measure),
        from_the_solution(&session),
        "every part is back on the solution's dates"
    );
}

#[test]
fn with_reduced_motion_the_first_frame_is_the_idle_scene() {
    let mut session = ledger(true);
    edited(&mut session);
    assert!(!session.slide_active(), "reduced motion: no slide");
    assert_eq!(
        session.frame_scene(measure),
        from_the_solution(&session),
        "t = 0 draws the solution's dates"
    );
    // The same edit with motion on starts from the old places, which are not the idle picture.
    let mut moving = ledger(false);
    edited(&mut moving);
    assert!(moving.slide_active());
    assert_ne!(
        moving.frame_scene(measure),
        session.frame_scene(measure),
        "a slide at t = 0 draws the old places"
    );
}
