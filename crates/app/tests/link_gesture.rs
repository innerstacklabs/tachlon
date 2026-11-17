//! The link gesture and link selection (gs#346, gs#49 PR 4), as native pointer sequences in the
//! style of `drag_frames.rs`: `web::App` drives the same `Session`, so what passes here is what the
//! browser runs, minus the canvas. A press with Alt (or on the terminal) drags a link out of a
//! bar; the release adds it through `Session::edit`, so it is one history step.

use crate::common;

use app::{
    RenderFacts, Session,
    edit::{Edit, EditError},
    meta::TaskMeta,
    outline::Outline,
};
use common::{clock, zoom_to};
use engine::{Calendar, DepId, Dependency, LinkType, Minutes, Schedule, Task, TaskId};
use timeline::Timeline;
use timeline::scene::Rect;

const HOUR: Minutes = 60;
const A: u32 = 0;
const B: u32 = 1;
const C: u32 = 2;
const D: u32 = 3;
/// A summary over a task `E` (row 5).
const S: u32 = 4;

/// Leaves A, B, C and D, then the summary S over E, in rows 0 to 5, 24/7 at one minute per pixel.
/// A and B are linked, and so are C and D.
pub(crate) fn session() -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
    let calendar = schedule.add_calendar(calendar);
    let tasks = [
        ("A", None, HOUR),
        ("B", None, HOUR),
        ("C", None, 3 * HOUR),
        ("D", None, HOUR),
        ("S", None, HOUR),
        ("E", Some(S), HOUR),
    ];
    let mut meta: Vec<TaskMeta> = Vec::new();
    for (name, parent, duration) in tasks {
        let task = Task {
            duration,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("a task");
        meta.push(TaskMeta {
            name: name.to_string(),
            parent: parent.map(TaskId),
            outline_level: if parent.is_some() { 2 } else { 1 },
            code: None,
            ..TaskMeta::default()
        });
    }
    for (pred, succ) in [(A, B), (C, D)] {
        schedule.add_dependency(fs(pred, succ)).expect("a link");
    }
    let solution = schedule.compute().expect("acyclic");
    let wbs: Vec<TaskId> = (0..tasks.len() as u32).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let timeline = Timeline::new((0..tasks.len() as u32).collect(), 1_000.0, 400.0, 1.0);
    let mut session = Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline fits");
    zoom_to(&mut session, 1.0, 0.0);
    session
}

fn fs(pred: u32, succ: u32) -> Dependency {
    Dependency {
        pred: TaskId(pred),
        succ: TaskId(succ),
        kind: LinkType::FS,
        lag: 0,
    }
}

fn add(pred: u32, succ: u32) -> Edit {
    Edit::AddLink {
        pred: TaskId(pred),
        succ: TaskId(succ),
        kind: LinkType::FS,
        lag: 0,
    }
}

/// A point inside the bar of `task`, a few pixels from its start.
pub(crate) fn on(s: &Session, task: u32) -> (f64, f64) {
    let row = s.timeline().row_of_task(task).expect("a row");
    let start = match s.summary_span(task) {
        Some((start, _)) => start,
        None => s.solution().dates[task as usize].es,
    };
    let v = &s.timeline().viewport;
    (
        v.x_of(start as f64) + 5.0,
        v.row_top(row) + v.row_height / 2.0,
    )
}

/// A point on a row below the last task: nothing is there.
fn empty(s: &Session) -> (f64, f64) {
    let v = &s.timeline().viewport;
    (900.0, v.row_top(9) + 5.0)
}

fn live(s: &Session) -> usize {
    s.schedule().live_dependencies().count()
}

/// Press at `from` with Alt, move to each point of `via`, and leave the pointer down.
fn drag(s: &mut Session, from: (f64, f64), via: &[(f64, f64)]) {
    s.pointer_down(from.0, from.1, true);
    for &(x, y) in via {
        s.pointer_move(x, y);
    }
}

/// An Alt drag from `pred` to `to`, released.
pub(crate) fn link(s: &mut Session, pred: u32, to: (f64, f64)) {
    let from = on(s, pred);
    drag(s, from, &[to]);
    s.pointer_up();
}

/// What a refused release must leave alone.
fn state(s: &Session) -> impl PartialEq + std::fmt::Debug + use<> {
    (
        s.revision(),
        s.stamp(),
        s.history_len(),
        s.solution().dates.clone(),
        s.schedule().dependencies().len(),
        live(s),
    )
}

fn status(s: &Session) -> serde_json::Value {
    serde_json::from_str(&s.status_json(&RenderFacts::default())).expect("status is JSON")
}

/// The rectangles `add_link_overlay` adds to the scene: the preview line or the terminal.
fn overlay(s: &Session) -> Vec<Rect> {
    let mut scene = s.scene();
    let before = scene.rects.len();
    s.add_link_overlay(&mut scene);
    scene.rects.split_off(before)
}

fn color_count(s: &Session, color: [f32; 4]) -> usize {
    overlay(s).iter().filter(|r| r.color == color).count()
}

/// Terminals drawn: the only rounded rectangle the overlay holds.
fn terminals(s: &Session) -> usize {
    overlay(s).iter().filter(|r| r.radius > 0.0).count()
}

#[test]
fn an_alt_drag_adds_one_link_in_one_history_step() {
    let mut s = session();
    let before = live(&s);
    let from = on(&s, A);
    let to = on(&s, C);
    drag(&mut s, from, &[to]);
    assert!(s.linking(), "past the threshold");
    assert_eq!(live(&s), before, "nothing is added until the release");
    assert_eq!(s.history_len(), (0, 0));
    s.pointer_up();

    assert!(!s.linking());
    assert_eq!(live(&s), before + 1);
    assert_eq!(s.history_len(), (1, 0), "exactly one step");
    let dep = s.selected_link().expect("the new link is selected");
    let added = s.schedule().dependencies()[dep.0 as usize];
    assert_eq!((added.pred, added.succ), (TaskId(A), TaskId(C)));
    assert_eq!(added.kind, LinkType::FS);
    assert_eq!(s.link_refusal(), None);
    // C waits for A now; the solution is a fresh solve of the schedule.
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());

    assert!(s.undo().expect("undoes"));
    assert_eq!(live(&s), before);
    assert_eq!(
        s.selected_link(),
        None,
        "undo of an add clears the selection"
    );
    assert!(s.redo().expect("redoes"));
    assert_eq!(
        s.selected_link(),
        Some(dep),
        "redo keeps the link id and selects it"
    );
}

#[test]
fn under_the_threshold_an_alt_click_selects_the_task() {
    let mut s = session();
    let (x, y) = on(&s, C);
    s.pointer_down(x, y, true);
    s.pointer_move(x + 2.0, y);
    assert!(!s.linking(), "two pixels is under the threshold");
    s.pointer_up();
    assert_eq!(s.selection().map(|selection| selection.task), Some(C));
    assert_eq!((live(&s), s.history_len()), (2, (0, 0)));
    assert_eq!(s.revision(), 0);
}

#[test]
fn a_release_on_nothing_changes_nothing() {
    let mut s = session();
    let before = state(&s);
    // On empty canvas, on the row of no task, and on the dragged bar itself.
    let away = empty(&s);
    link(&mut s, A, away);
    assert_eq!(state(&s), before);
    assert_eq!(s.link_refusal(), None, "a drop on nothing is not a refusal");
    let (x, y) = on(&s, A);
    drag(&mut s, (x, y), &[(x + 20.0, y + 80.0), (x + 4.0, y)]);
    assert!(s.linking());
    s.pointer_up();
    assert_eq!(state(&s), before);
    assert_eq!(s.link_refusal(), None);
    assert_eq!(s.selected_link(), None);
}

#[test]
fn a_release_on_a_summary_a_repeat_or_a_loop_is_refused_with_the_edit_message() {
    for (pred, onto, text) in [
        (A, S, "\"S\" is a summary. Link its sub-tasks instead."),
        (A, B, "\"A\" is already linked to \"B\"."),
        (D, C, "\"D\" to \"C\" would make a loop: C → D → C."),
    ] {
        let mut s = session();
        let before = state(&s);
        let to = on(&s, onto);
        link(&mut s, pred, to);
        assert_eq!(state(&s), before, "{text}");
        assert_eq!(s.link_refusal(), Some(text));
        // The same words the edit gives, so a page that shows one shows the other.
        let error = s.edit(add(pred, onto)).expect_err("refused");
        assert_eq!(error.to_string(), text);
        assert_eq!(
            status(&s)["link"]["refusal"],
            text,
            "the status carries the refusal"
        );
        // The next press clears it.
        let (x, y) = on(&s, pred);
        s.pointer_down(x, y, false);
        s.pointer_up();
        assert_eq!(s.link_refusal(), None);
    }
}

#[test]
fn the_preview_turns_refused_before_the_release() {
    let mut s = session();
    let refused = s.theme().link_refused;
    let from = on(&s, A);
    let to = on(&s, C);
    drag(&mut s, from, &[to]);
    assert_eq!(color_count(&s, refused), 0, "a free drop is not refused");
    s.pointer_move(on(&s, S).0, on(&s, S).1);
    assert!(color_count(&s, refused) > 0, "a summary would be refused");
    s.pointer_move(on(&s, C).0, on(&s, C).1);
    assert_eq!(color_count(&s, refused), 0);
    let dragging = &status(&s)["link"]["drag"];
    assert_eq!(
        (dragging["pred"].as_u64(), dragging["target"].as_u64()),
        (Some(A.into()), Some(C.into()))
    );
    assert!(dragging["refused"].is_null());
    // A repeat of a link, and a link back along a chain, turn it too.
    s.pointer_move(on(&s, B).0, on(&s, B).1);
    assert!(color_count(&s, refused) > 0, "A to B repeats a link");
    assert_eq!(
        status(&s)["link"]["drag"]["refused"],
        "\"A\" is already linked to \"B\"."
    );
    s.pointer_cancel();
    let from = on(&s, D);
    let to = on(&s, C);
    drag(&mut s, from, &[to]);
    assert!(color_count(&s, refused) > 0, "D to C would make a loop");
    s.pointer_cancel();
}

#[test]
fn pointercancel_mid_drag_adds_nothing() {
    let mut s = session();
    let before = state(&s);
    let from = on(&s, A);
    let to = on(&s, C);
    drag(&mut s, from, &[to]);
    assert!(s.linking());
    s.pointer_cancel();
    assert!(!s.linking());
    assert_eq!(state(&s), before);
    assert_eq!(s.selected_link(), None);
    assert!(status(&s)["link"]["drag"].is_null());
    // A release that follows the cancel has nothing to commit.
    s.pointer_up();
    assert_eq!(state(&s), before);
}

#[test]
fn the_loop_search_runs_once_per_target_change() {
    let mut s = session();
    assert_eq!(s.link_checks(), 0);
    let from = on(&s, A);
    s.pointer_down(from.0, from.1, true);
    s.pointer_move(from.0 + 2.0, from.1);
    assert_eq!(s.link_checks(), 0, "under the threshold there is no target");
    // Twenty moves along C's bar: one target, one search.
    let (cx, cy) = on(&s, C);
    for step in 0..20 {
        s.pointer_move(cx + f64::from(step), cy);
    }
    assert_eq!(s.link_checks(), 1);
    // Off to D is a new target; the summary is refused without a search; the same bar again
    // after empty canvas is a new target change.
    s.pointer_move(on(&s, D).0, on(&s, D).1);
    assert_eq!(s.link_checks(), 2);
    s.pointer_move(on(&s, S).0, on(&s, S).1);
    s.pointer_move(on(&s, B).0, on(&s, B).1);
    assert_eq!(s.link_checks(), 2, "a summary and a repeat need no search");
    s.pointer_move(empty(&s).0, empty(&s).1);
    s.pointer_move(cx, cy);
    assert_eq!(s.link_checks(), 3);
    s.pointer_up();
    assert_eq!(
        s.link_checks(),
        3,
        "the release adds the link on the cached verdict"
    );
    assert_eq!(s.history_len(), (1, 0));
}

/// A point on the arrow of link `dep`, found the way a pointer would: where the link hit test says.
fn on_arrow(s: &Session, dep: DepId) -> (f64, f64) {
    let dates = &s.solution().dates;
    (0..1000)
        .flat_map(|x| (48..300).map(move |y| (f64::from(x), f64::from(y))))
        .find(|&(x, y)| s.timeline().hit_link(s.links(), dates, x, y) == Some(dep.0))
        .expect("the arrow is on the canvas")
}

fn click(s: &mut Session, (x, y): (f64, f64)) {
    s.pointer_down(x, y, false);
    s.pointer_up();
}

#[test]
fn clicking_an_arrow_selects_it_and_clicking_a_bar_clears_it() {
    let mut s = session();
    let arrow = on_arrow(&s, DepId(0));
    click(&mut s, arrow);
    assert_eq!(s.selected_link(), Some(DepId(0)));
    assert_eq!(status(&s)["link"]["selected"], 0);
    assert_eq!(
        s.selected_link_drawn(),
        Some((0, 2.0)),
        "drawn at the selected-w width"
    );
    assert_eq!(s.revision(), 0, "a selection is not an edit");

    let bar = on(&s, C);
    click(&mut s, bar);
    assert_eq!(s.selected_link(), None);
    assert_eq!(s.selection().map(|selection| selection.task), Some(C));

    // The other arrow, then empty canvas.
    let other = on_arrow(&s, DepId(1));
    click(&mut s, other);
    assert_eq!(s.selected_link(), Some(DepId(1)));
    assert_eq!(
        s.selection().map(|selection| selection.task),
        Some(C),
        "a link and a task can both be selected"
    );
    let nothing = empty(&s);
    click(&mut s, nothing);
    assert_eq!((s.selected_link(), s.selection().is_none()), (None, true));

    // Clear selection (Escape) clears the link too, and so does the link's own clear.
    click(&mut s, arrow);
    s.clear_selection();
    assert_eq!(s.selected_link(), None);
    click(&mut s, arrow);
    s.clear_link_selection();
    assert_eq!(s.selected_link(), None);
}

#[test]
fn delete_with_a_link_selected_removes_the_link_and_not_the_row() {
    let mut s = session();
    s.select(A);
    let arrow = on_arrow(&s, DepId(0));
    click(&mut s, arrow);
    assert_eq!(s.selected_link(), Some(DepId(0)));
    assert_eq!(s.selection().map(|selection| selection.task), Some(A));

    assert_eq!(s.remove_selected_link().expect("removes"), DepId(0));
    assert_eq!(live(&s), 1);
    assert!(!s.schedule().is_dependency_live(DepId(0)));
    assert!(!s.schedule().is_deleted(TaskId(A)), "the row stays");
    assert_eq!(s.timeline().order().len(), 6);
    assert_eq!(s.selected_link(), None, "a removed link is not selected");
    assert_eq!(s.history_len(), (1, 0));

    assert!(s.undo().expect("undoes"));
    assert!(s.schedule().is_dependency_live(DepId(0)));
    assert_eq!(
        s.selected_link(),
        Some(DepId(0)),
        "a restored link is selected"
    );
    // Nothing selected: nothing to remove.
    s.clear_link_selection();
    let error = s.remove_selected_link().expect_err("no link selected");
    assert_eq!(error, EditError::NoLinkSelected);
}

#[test]
fn undo_and_edits_are_refused_mid_drag() {
    let mut s = session();
    s.edit(Edit::SetDuration {
        task: TaskId(D),
        minutes: 2 * HOUR,
    })
    .expect("an edit to undo");
    let from = on(&s, A);
    let to = on(&s, C);
    drag(&mut s, from, &[to]);
    let before = state(&s);

    assert!(!s.undo().expect("refused, not failed"));
    assert!(!s.redo().expect("refused, not failed"));
    assert_eq!(s.edit(add(B, D)), Err(EditError::Busy));
    assert_eq!(s.remove_selected_link(), Err(EditError::NoLinkSelected));
    assert_eq!(
        s.next_critical(),
        None,
        "the view does not jump under the drag"
    );
    assert!(!s.toggle(TaskId(S)), "no collapse mid-drag");
    assert_eq!(state(&s), before);

    s.pointer_cancel();
    assert!(s.undo().expect("undoes once the drag is over"));
}

#[test]
fn an_alt_press_on_a_summary_or_empty_canvas_starts_no_link() {
    let mut s = session();
    for at in [on(&s, S), empty(&s)] {
        s.pointer_down(at.0, at.1, true);
        s.pointer_move(at.0 + 30.0, at.1 + 30.0);
        assert!(!s.linking());
        s.pointer_up();
    }
    assert_eq!(live(&s), 2);
}

#[test]
fn the_terminal_is_drawn_for_the_hovered_bar_and_starts_a_link_without_alt() {
    let mut s = session();
    assert_eq!(terminals(&s), 0, "no hover, no selection");
    let (x, y) = on(&s, A);
    s.pointer_move(x, y);
    assert_eq!(terminals(&s), 1, "one terminal, on the hovered bar");
    let dot = overlay(&s)[0];
    let centre = (
        f64::from(dot.pos[0]) + f64::from(dot.size[0]) / 2.0,
        f64::from(dot.pos[1]) + f64::from(dot.size[1]) / 2.0,
    );

    // A press on the dot, no Alt, and a drag onto C.
    s.pointer_move(centre.0, centre.1);
    s.pointer_down(centre.0, centre.1, false);
    let (cx, cy) = on(&s, C);
    s.pointer_move(cx, cy);
    assert!(s.linking());
    assert_eq!(terminals(&s), 0, "the dot is hidden while dragging");
    s.pointer_up();
    let dep = s.selected_link().expect("the link is selected");
    let added = s.schedule().dependencies()[dep.0 as usize];
    assert_eq!((added.pred, added.succ), (TaskId(A), TaskId(C)));
    assert_eq!(s.history_len(), (1, 0));

    // The pointer leaves for nothing and the selected task shows its terminal.
    s.pointer_leave();
    assert_eq!(terminals(&s), 0);
    s.select(B);
    assert_eq!(terminals(&s), 1);
    // A summary has none.
    s.select(S);
    assert_eq!(terminals(&s), 0);
}

#[test]
fn a_press_beside_the_terminal_pans_and_a_bad_link_look_is_refused() {
    let mut s = session();
    let (x, y) = on(&s, A);
    s.pointer_move(x, y);
    s.pointer_down(x - 4.0, y + 40.0, false);
    assert!(!s.linking());
    s.pointer_up();
    assert!(s.set_link_look(0.0, 3.0, 2.0, 2.0).is_err());
    assert!(s.set_link_look(8.0, -1.0, 2.0, 2.0).is_err());
    assert!(s.set_link_look(8.0, 3.0, f64::NAN, 2.0).is_err());
    assert!(s.set_link_look(8.0, 3.0, 2.0, 0.0).is_err());
    assert!(s.set_link_look(10.0, 0.0, 3.0, 4.0).is_ok());
}

#[test]
fn a_selected_link_goes_when_it_leaves_force() {
    let mut s = session();
    assert!(s.select_link(DepId(1)));
    s.edit(Edit::Delete { task: TaskId(D) }).expect("deletes");
    assert_eq!(s.selected_link(), None, "the link left with its task");
    assert!(
        !s.select_link(DepId(1)),
        "a link that is not in force cannot be selected"
    );
    assert!(!s.select_link(DepId(99)));
    assert!(s.select_link(DepId(0)));
    assert_eq!(live(&s), 1);
}

#[path = "link_gesture/view_change.rs"]
mod view_change;

#[path = "link_gesture/from_the_page.rs"]
mod from_the_page;

#[test]
fn step_link_walks_the_selected_tasks_links_and_skips_removed_ones() {
    let mut s = session();
    assert!(!s.step_link(true), "no task selected");
    s.select(B);
    let add = Edit::AddLink {
        pred: TaskId(B),
        succ: TaskId(C),
        kind: LinkType::FS,
        lag: 0,
    };
    s.edit(add).expect("B to C");
    s.clear_link_selection();
    // B has two links (A to B, then B to C); C to D is not B's.
    assert!(s.step_link(true));
    assert_eq!(s.selected_link(), Some(DepId(0)));
    assert!(s.step_link(true));
    assert_eq!(s.selected_link(), Some(DepId(2)));
    assert!(s.step_link(true), "wraps past the last link");
    assert_eq!(s.selected_link(), Some(DepId(0)));
    assert!(s.step_link(false), "wraps before the first link");
    assert_eq!(s.selected_link(), Some(DepId(2)));
    s.clear_link_selection();
    assert!(s.step_link(false));
    assert_eq!(
        s.selected_link(),
        Some(DepId(2)),
        "backward from none is the last"
    );
    assert_eq!(s.selection().map(|selection| selection.task), Some(B));

    // A removed link is never stepped to, and a lone link stays selected.
    assert_eq!(s.remove_selected_link().expect("removes"), DepId(2));
    assert!(s.step_link(false));
    assert_eq!(s.selected_link(), Some(DepId(0)));
    assert!(s.step_link(true));
    assert_eq!(s.selected_link(), Some(DepId(0)));
    s.select(S);
    assert!(!s.step_link(true), "S has no link");
    assert_eq!(s.selected_link(), None, "selecting a task clears the link");
}
