//! A proposal holds changes apart from the schedule: ghosts show where they would put the tasks, and the
//! person accepts them all as one undo step or rejects them all. The fixture is `what_moves`'s: A, B and C
//! (tasks 0 to 2) are a chain of one working day each, D is alone; day 0 is Monday 2026-01-05.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::what_moves::{session, sized};
use app::{Session, edit::Edit};
use engine::TaskId;
use serde_json::Value;
use timeline::labels::Placement;

fn answer(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}

/// The x of each ghost ring the proposal adds to a scene that shows every task.
pub(crate) fn ghosts(s: &mut Session) -> Vec<f64> {
    s.fit(0.0, 30.0 * 24.0 * 60.0);
    let mut scene = s.scene();
    assert!(
        scene.rects.iter().all(|r| r.stroke == 0.0),
        "no ring but the ghosts"
    );
    s.add_proposal_ghosts(&mut scene);
    scene
        .rects
        .iter()
        .filter(|r| r.stroke > 0.0)
        .map(|r| f64::from(r.pos[0]))
        .collect()
}

#[test]
fn a_proposal_is_drawn_as_ghosts_and_changes_nothing() {
    let mut live = session();
    let (solution, stamp) = (live.solution().clone(), live.stamp());
    assert!(
        ghosts(&mut live).is_empty(),
        "nothing proposed, nothing drawn"
    );
    let said = live.propose_duration(1, 3);
    assert_eq!(
        said,
        live.what_moves_longer(1, 2),
        "the answer is the copy solved in full"
    );
    assert_eq!(ghosts(&mut live).len(), 2, "B grows and C follows it");
    assert!(live.solution() == &solution && live.stamp() == stamp);
    assert!(live.history_len() == (0, 0) && !live.can_undo());
    assert_eq!(live.schedule().tasks()[1].duration, 8 * 60);
    assert_eq!(
        answer(&live.propose_move(3, "2026-01-12"))["moved"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(ghosts(&mut live).len(), 3, "D's ghost joins B's and C's");
}

#[test]
fn a_proposal_survives_an_edit_and_its_ghosts_follow_the_schedule() {
    let mut live = session();
    live.propose_duration(1, 3);
    let before = ghosts(&mut live);
    let longer = Edit::SetDuration {
        task: TaskId(0),
        minutes: 16 * 60,
    };
    live.edit(longer).unwrap();
    live.advance_motion();
    let after = ghosts(&mut live);
    assert_eq!(after.len(), before.len(), "the proposal is still held");
    assert!(after != before, "its ghosts moved with A's new length");
    assert_eq!(answer(&live.accept_proposal())["changes"], 1);
    let tasks = live.schedule().tasks();
    assert_eq!((tasks[0].duration, tasks[1].duration), (16 * 60, 24 * 60));
}

#[test]
fn accepting_is_one_undo_step_and_rejecting_changes_nothing() {
    let mut live = session();
    let start = live.solution().clone();
    live.propose_duration(1, 3);
    live.propose_duration(1, 2);
    live.propose_move(3, "2026-01-12");
    let accepted = answer(&live.accept_proposal());
    assert_eq!(
        accepted["changes"], 2,
        "a second change to one thing replaces the first"
    );
    assert_eq!(live.history_len(), (1, 0));
    assert_eq!(*live.solution(), live.schedule().clone().compute().unwrap());
    assert_ne!(live.solution(), &start);
    assert_eq!(live.undo(), Ok(true));
    assert!(
        live.solution() == &start && live.history_len() == (0, 1),
        "one undo puts both back"
    );
    assert_eq!(answer(&live.accept_proposal())["reason"], "noProposal");
    live.propose_duration(1, 3);
    assert!(live.reject_proposal());
    assert!(ghosts(&mut live).is_empty() && live.history_len() == (0, 1));
    assert!(!live.reject_proposal());
}

#[test]
fn accepting_one_change_is_one_undo_step_and_the_others_are_solved_again() {
    let mut live = session();
    let start = live.solution().clone();
    live.propose_duration(0, 2);
    live.propose_duration(1, 3);
    live.propose_move(3, "2026-01-12");
    let all = ghosts(&mut live);
    assert_eq!(all.len(), 4, "A, B, C and D");
    assert_eq!(
        answer(&live.accept_proposed("move", 0))["reason"],
        "noProposal"
    );
    assert_eq!(answer(&live.accept_proposed("duration", 0))["changes"], 1);
    assert_eq!(live.history_len(), (1, 0));
    assert_eq!(live.schedule().tasks()[0].duration, 16 * 60);
    assert_eq!(
        live.schedule().tasks()[1].duration,
        8 * 60,
        "B's change is still held"
    );
    let rest = ghosts(&mut live);
    assert_eq!(
        rest.len(),
        all.len() - 1,
        "A's ghost is gone, B's, C's and D's stay"
    );
    assert_eq!(rest, all[1..], "they stay where they were drawn");
    assert_eq!(live.undo(), Ok(true));
    assert!(live.solution() == &start && live.history_len() == (0, 1));
}

#[test]
fn rejecting_one_change_changes_nothing_and_the_others_are_solved_again() {
    let mut live = session();
    let (solution, stamp) = (live.solution().clone(), live.stamp());
    live.propose_duration(0, 2);
    live.propose_duration(1, 3);
    let both = ghosts(&mut live);
    assert!(!live.reject_proposed("move", 0), "no move of A is held");
    assert!(live.reject_proposed("duration", 0));
    let one = ghosts(&mut live);
    assert_eq!(one.len(), 2, "B grows and C follows it");
    assert!(
        one != both[1..],
        "B's ghost no longer waits for A's longer task"
    );
    assert!(live.solution() == &solution && live.stamp() == stamp);
    assert!(live.history_len() == (0, 0) && !live.can_undo());
    assert!(live.reject_proposed("duration", 1));
    assert!(ghosts(&mut live).is_empty() && !live.reject_proposal());
}

#[test]
fn a_change_to_a_task_that_has_started_is_refused_and_stays_held() {
    let mut live = session();
    live.propose_duration(0, 2);
    live.propose_duration(1, 3);
    live.set_progress_field_text(TaskId(0), "actual_start", "2026-01-05")
        .unwrap();
    let history = live.history_len();
    let said = answer(&live.accept_proposed("duration", 0));
    assert_eq!(said["reason"], "hasActuals");
    assert!(live.history_len() == history, "no undo step");
    assert_eq!(live.schedule().tasks()[0].duration, 8 * 60);
    assert!(live.reject_proposed("duration", 0), "it is still held");
}

#[test]
fn accepting_all_refuses_a_change_to_a_task_that_has_started_and_applies_nothing() {
    let mut live = session();
    live.propose_duration(0, 2);
    live.propose_duration(1, 3);
    live.set_progress_field_text(TaskId(0), "actual_start", "2026-01-05")
        .unwrap();
    let history = live.history_len();
    let said = answer(&live.accept_proposal());
    assert_eq!(said["reason"], "hasActuals");
    assert!(live.history_len() == history, "no undo step");
    let durations = |s: &Session| {
        (
            s.schedule().tasks()[0].duration,
            s.schedule().tasks()[1].duration,
        )
    };
    assert_eq!(durations(&live), (8 * 60, 8 * 60), "not even B's change");
    assert!(live.reject_proposed("duration", 0) && live.reject_proposed("duration", 1));
}

#[test]
fn a_proposal_is_refused_with_a_reason_and_the_held_changes_stay() {
    let mut live = session();
    live.propose_duration(1, 3);
    let reason = |json: String| answer(&json)["reason"].as_str().unwrap().to_string();
    assert_eq!(reason(live.propose_move(1, "soon")), "badDate");
    assert_eq!(reason(live.propose_duration(99, 1)), "unknownTask");
    live.set_progress_field_text(TaskId(3), "actual_start", "2026-01-05")
        .unwrap();
    assert_eq!(reason(live.propose_duration(3, 1)), "hasActuals");
    assert_eq!(
        answer(&live.accept_proposal())["changes"],
        1,
        "only B's change is held"
    );
}

#[test]
fn at_most_ten_changes_are_held() {
    let mut live = sized(6);
    let reason = |json: String| answer(&json)["reason"].as_str().map(str::to_string);
    for task in 0..6 {
        assert_eq!(reason(live.propose_duration(task, 2)), None);
    }
    for task in 0..4 {
        assert_eq!(reason(live.propose_move(task, "2026-01-12")), None);
    }
    let over = reason(live.propose_move(4, "2026-01-12"));
    assert_eq!(over.as_deref(), Some("tooMany"));
    let replaced = reason(live.propose_duration(0, 3));
    assert_eq!(replaced, None, "a held key is replaced at the cap");
    assert_eq!(answer(&live.accept_proposal())["changes"], 10);
}

#[test]
fn the_frame_draws_the_ghosts_until_they_are_rejected() {
    let mut live = session();
    live.fit(0.0, 30.0 * 24.0 * 60.0);
    let rings = |s: &Session| {
        let scene = s.frame_scene(|_, _, _| 0.0);
        scene.rects.iter().filter(|r| r.stroke > 0.0).count()
    };
    let bare = rings(&live);
    live.propose_duration(1, 3);
    assert_eq!(
        rings(&live),
        bare + 2,
        "B's and C's ghosts are in the frame"
    );
    live.reject_proposal();
    assert_eq!(rings(&live), bare);
}

#[test]
fn a_proposed_task_name_sits_beside_its_ghost_ring() {
    let mut live = session();
    live.fit(0.0, 30.0 * 24.0 * 60.0);
    let name_x = |s: &Session| {
        let scene = s.frame_scene(|_, _, _| 40.0);
        let ring = scene.rects.iter().find(|r| r.stroke > 0.0);
        let right = ring.map(|r| r.pos[0] + r.size[0]);
        (scene.bar_labels.iter().find(|l| l.task == 3)).map(|l| (l.x, right))
    };
    assert_eq!(name_x(&live).map(|(_, ring)| ring), Some(None));
    live.propose_move(3, "2026-01-12");
    let (x, ring) = name_x(&live).unwrap();
    assert!(x >= ring.unwrap(), "D's name at {x} is inside its ring");
}

#[test]
fn a_name_inside_its_bar_moves_only_when_the_ring_reaches_it() {
    let mut live = session();
    live.propose_duration(0, 3);
    live.accept_proposal();
    // C is on Friday and the view ends with its day, so no room is left beside its bar.
    live.fit(0.0, 4.0 * 24.0 * 60.0 + 17.0 * 60.0);
    let name = |s: &Session| {
        let scene = s.frame_scene(|_, _, _| 10.0);
        (scene.bar_labels.into_iter().find(|l| l.task == 2)).map(|l| (l.placement, l.x))
    };
    let before = name(&live).unwrap();
    assert_eq!(before.0, Placement::Inside);
    live.propose_move(2, "2026-01-09 10:00");
    assert_eq!(
        name(&live).unwrap().0,
        Placement::Right,
        "a ring past its left"
    );
    live.reject_proposal();
    live.propose_duration(0, 1);
    assert_eq!(name(&live), Some(before), "C's ring ends before its name");
    live.propose_move(2, "2026-01-09 10:00");
    live.edit(Edit::SetProgress {
        task: TaskId(2),
        actual_start: Some(live.solution().dates[2].es),
        actual_finish: Some(live.solution().dates[2].ef),
        remaining: None,
    })
    .unwrap();
    let scene = live.frame_scene(|_, _, _| 10.0);
    let l = scene.bar_labels.iter().find(|l| l.task == 2).unwrap();
    assert_eq!((l.placement, l.done), (Placement::Right, None));
}

#[test]
fn a_ghost_whose_bar_is_off_the_canvas_has_its_name_beside_it() {
    let mut live = session();
    live.propose_move(3, "2026-01-12");
    live.fit(3.0 * 24.0 * 60.0, 30.0 * 24.0 * 60.0);
    let scene = live.frame_scene(|_, _, _| 40.0);
    let ring = scene.rects.iter().find(|r| r.stroke > 0.0).unwrap();
    let name = scene.bar_labels.iter().find(|l| l.task == 3).unwrap();
    assert!(name.x >= ring.pos[0] + ring.size[0], "beside the ring");
}

#[test]
fn a_name_beside_a_ghost_ring_steps_clear_of_a_link_arrow() {
    let mut live = sized(5);
    live.propose_link(2, 4, "FS");
    live.accept_proposal();
    live.fit(0.0, 30.0 * 24.0 * 60.0);
    live.propose_move(3, "2026-01-07");
    let scene = live.frame_scene(|_, _, _| 40.0);
    let name = scene.bar_labels.iter().find(|l| l.task == 3).unwrap();
    let ink = [live.theme().link, live.theme().sel_edge];
    let under = |r: &timeline::scene::Rect| {
        ink.contains(&r.color)
            && (name.x..name.x + name.text_w).contains(&r.pos[0])
            && (r.pos[1]..r.pos[1] + r.size[1]).contains(&name.y)
    };
    assert!(!scene.rects.iter().any(under), "an arrow under it");
}

fn reason(json: &str) -> String {
    answer(json)["reason"].as_str().unwrap().to_string()
}

fn moved(json: &str) -> usize {
    answer(json)["moved"].as_array().unwrap().len()
}

fn links(s: &Session) -> usize {
    s.schedule().live_dependencies().count()
}

#[test]
fn a_proposed_link_is_drawn_as_ghosts_and_one_accept_adds_it_as_one_undo_step() {
    let mut live = session();
    let (start, stamp) = (live.solution().clone(), live.stamp());
    assert_eq!(moved(&live.propose_link(3, 0, "FS")), 3, "A waits for D");
    assert_eq!(ghosts(&mut live).len(), 3, "A, B and C");
    assert!(live.solution() == &start && live.stamp() == stamp && links(&live) == 2);
    assert_eq!(answer(&live.accept_proposal())["changes"], 1);
    assert_eq!((links(&live), live.history_len()), (3, (1, 0)));
    assert_eq!(*live.solution(), live.schedule().clone().compute().unwrap());
    assert_eq!(live.undo(), Ok(true));
    assert!(live.solution() == &start && links(&live) == 2);
}

#[test]
fn a_proposed_link_on_a_linked_pair_changes_its_type_and_lag() {
    let mut live = session();
    assert_eq!(moved(&live.propose_link(0, 1, "FS+2d")), 2, "B and C");
    assert_eq!(reason(&live.propose_link(0, 1, "FS")), "alreadyLinked");
    assert_eq!(answer(&live.accept_proposal())["changes"], 1);
    assert_eq!(live.schedule().dependencies()[0].lag, 16 * 60);
    assert_eq!(links(&live), 2, "a change, not a second link");
}

#[test]
fn a_proposed_unlink_moves_what_the_link_held_back_and_can_be_rejected() {
    let mut live = session();
    assert_eq!(moved(&live.propose_unlink(0, 1)), 2, "B and C start sooner");
    assert_eq!(ghosts(&mut live).len(), 2);
    assert!(live.reject_proposed_link(0, 1) && !live.reject_proposed_link(0, 1));
    assert!(ghosts(&mut live).is_empty() && links(&live) == 2);
    live.propose_unlink(0, 1);
    assert_eq!(answer(&live.accept_proposal())["changes"], 1);
    assert_eq!(links(&live), 1);
}

#[test]
fn a_second_change_to_a_pair_replaces_the_first() {
    let mut live = session();
    live.propose_unlink(0, 1);
    assert_eq!(moved(&live.propose_link(0, 1, "FS+2d")), 2, "B and C later");
    assert_eq!(answer(&live.accept_proposal())["changes"], 1);
    assert_eq!(
        (links(&live), live.schedule().dependencies()[0].lag),
        (2, 16 * 60)
    );
}

#[test]
fn a_link_that_moves_no_early_date_answers_nothing_moved_and_draws_nothing() {
    let mut live = session();
    assert_eq!(moved(&live.propose_link(0, 2, "FS")), 0);
    assert_eq!(ghosts(&mut live).len(), 0);
    assert_eq!(answer(&live.accept_proposal())["changes"], 1);
    assert_eq!(links(&live), 3);
}

#[test]
fn a_proposed_link_is_refused_with_a_reason_and_the_held_changes_stay() {
    let mut live = session();
    live.propose_duration(1, 3);
    live.propose_link(2, 3, "FS");
    assert_eq!(
        reason(&live.propose_link(3, 0, "FS")),
        "loop",
        "through a held link"
    );
    assert_eq!(reason(&live.propose_link(2, 0, "FS")), "loop");
    assert_eq!(reason(&live.propose_link(0, 1, "soon")), "badLinkText");
    assert_eq!(reason(&live.propose_link(0, 99, "FS")), "unknownTask");
    assert_eq!(reason(&live.propose_unlink(99, 0)), "unknownTask");
    assert_eq!(reason(&live.propose_unlink(0, 2)), "noLink");
    assert_eq!(reason(&live.propose_link(1, 1, "FS")), "other");
    assert_eq!(
        answer(&live.accept_proposal())["changes"],
        2,
        "both held still"
    );
}

#[test]
fn a_link_to_a_task_that_has_started_is_allowed() {
    let mut live = session();
    live.set_progress_field_text(TaskId(3), "actual_start", "2026-01-05")
        .unwrap();
    assert_eq!(reason(&live.propose_duration(3, 1)), "hasActuals");
    assert_eq!(answer(&live.propose_link(3, 0, "FS"))["ok"], true);
    assert_eq!(answer(&live.accept_proposal())["changes"], 1);
}

#[test]
fn one_link_is_accepted_by_its_pair_and_the_others_stay_held() {
    let mut live = session();
    live.propose_duration(1, 3);
    live.propose_link(3, 0, "FS");
    assert_eq!(reason(&live.accept_proposed_link(0, 3)), "noProposal");
    assert_eq!(answer(&live.accept_proposed_link(3, 0))["changes"], 1);
    assert_eq!((links(&live), live.history_len()), (3, (1, 0)));
    assert!(
        live.reject_proposed("duration", 1),
        "B's change is still held"
    );
}

#[test]
fn a_held_link_the_schedule_later_refuses_draws_no_ghost_and_accepts_nothing() {
    let mut live = session();
    live.propose_link(3, 0, "FS");
    live.edit(Edit::AddLink {
        pred: TaskId(3),
        succ: TaskId(0),
        kind: engine::LinkType::FS,
        lag: 0,
    })
    .unwrap();
    live.advance_motion();
    assert!(ghosts(&mut live).is_empty(), "the link is already there");
    let history = live.history_len();
    assert_eq!(reason(&live.accept_proposal()), "alreadyLinked");
    assert!(live.history_len() == history && links(&live) == 3);
    assert!(live.reject_proposed_link(3, 0), "it is still held");
}

#[test]
fn a_proposed_actual_finish_draws_a_ghost_moves_nothing_and_one_accept_is_one_undo_step() {
    let mut live = session();
    live.set_progress_field_text(TaskId(0), "actual_start", "2026-01-05 08:00")
        .unwrap();
    let (solution, stamp) = (live.solution().clone(), live.stamp());
    let said = live.propose_progress(0, "actual_finish", "2026-01-05 17:00");
    assert_eq!(moved(&said), 0, "no date moves");
    assert_eq!(ghosts(&mut live).len(), 1, "the task has a ghost anyway");
    assert!(live.solution() == &solution && live.stamp() == stamp);
    assert!(live.history_len() == (1, 0) && live.schedule().tasks()[0].actual_finish.is_none());
    assert_eq!(answer(&live.accept_proposed("progress", 0))["changes"], 1);
    assert_eq!(live.schedule().tasks()[0].actual_finish, Some(17 * 60));
    assert_eq!(live.history_len(), (2, 0));
    assert_eq!(live.undo(), Ok(true));
    assert!(live.schedule().tasks()[0].actual_finish.is_none());
}

#[test]
fn a_second_progress_proposal_replaces_the_first_and_percent_and_remaining_accept_alike() {
    let mut live = session();
    live.propose_progress(0, "remaining", "6h");
    live.propose_progress(0, "percent", "50");
    assert_eq!(answer(&live.accept_proposal())["changes"], 1, "one held");
    let a = &live.schedule().tasks()[0];
    assert_eq!((a.remaining, a.actual_start), (Some(4 * 60), Some(8 * 60)));
    assert_eq!(live.undo(), Ok(true));
    live.propose_progress(1, "remaining", "2h");
    assert_eq!(answer(&live.accept_proposed("progress", 1))["changes"], 1);
    assert_eq!(live.schedule().tasks()[1].remaining, Some(2 * 60));
    live.propose_progress(2, "remaining", "2h");
    assert!(live.reject_proposed("progress", 2) && ghosts(&mut live).is_empty());
    assert!(live.schedule().tasks()[2].remaining.is_none() && live.history_len() == (1, 0));
}

#[test]
fn a_progress_proposal_that_cannot_be_held_is_refused_with_a_reason_and_nothing_is_held() {
    let mut live = session();
    live.set_progress_field_text(TaskId(0), "actual_start", "2026-01-05 08:00")
        .unwrap();
    live.set_data_date_text("2026-01-06").unwrap();
    let mut outline = crate::common::collapse::session(60);
    let summary = crate::common::collapse::summaries(&outline)[0].0;
    let (mut busy, _, _, (x, y)) = crate::common::setup();
    busy.pointer_down(x, y, false);
    let said = [
        live.propose_progress(0, "actual_finish", "yesterday"),
        live.propose_progress(0, "actual_finish", "2026-01-05 07:00"),
        live.propose_progress(1, "actual_start", "2026-01-12"),
        live.propose_progress(1, "percent", "101"),
        outline.propose_progress(summary, "percent", "50"),
        busy.propose_progress(0, "percent", "50"),
    ];
    let reasons: Vec<String> = said.iter().map(|json| reason(json)).collect();
    let want = "badDate finishBeforeStart actualAfterDataDate badPercent summary busy";
    assert_eq!(reasons.join(" "), want);
    assert!(ghosts(&mut live).is_empty() && !live.reject_proposal());
}

#[test]
fn a_progress_proposal_is_planned_again_against_the_task_when_it_is_accepted() {
    let mut live = session();
    live.propose_progress(0, "actual_finish", "2026-01-05 17:00");
    live.propose_progress(1, "actual_start", "2026-01-07 08:00");
    let started = "2026-01-05 08:00";
    live.set_progress_field_text(TaskId(0), "actual_start", started)
        .unwrap();
    live.set_data_date_text("2026-01-06").unwrap();
    let said = live.accept_proposed("progress", 1);
    assert_eq!(reason(&said), "actualAfterDataDate");
    assert!(live.schedule().tasks()[1].actual_start.is_none());
    assert!(live.reject_proposed("progress", 1), "it stays held");
    assert_eq!(answer(&live.accept_proposal())["changes"], 1);
    let a = &live.schedule().tasks()[0];
    assert_eq!(
        (a.actual_start, a.actual_finish),
        (Some(8 * 60), Some(17 * 60))
    );
}

#[test]
fn a_date_alone_is_the_first_working_minute_of_a_start_and_the_end_of_the_last_of_a_finish() {
    let mut live = session();
    live.propose_progress(0, "actual_start", "2026-01-05");
    live.accept_proposal();
    live.propose_progress(0, "actual_finish", "2026-01-05");
    live.accept_proposal();
    let a = &live.schedule().tasks()[0];
    assert_eq!(
        (a.actual_start, a.actual_finish),
        (Some(8 * 60), Some(17 * 60))
    );
}
