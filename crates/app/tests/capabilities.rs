//! The host capability profile (gantt#2090, decision 0019, rule 5): a gesture or a group of fields the
//! host turned off is refused with `EditError::NotAllowed` on every path, leaves the chart as it was,
//! and the rest of the chart still works.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::link_gesture::{link, on, session, session_on};
use crate::read_only::{longer, state};
use app::Session;
use app::dates::Origin;
use app::edit::{Edit, EditError, start::StartError};
use engine::{DepId, Limit, Pin, TaskId};
use std::{cell::Cell, rc::Rc};

const NO_ROWS: &str = r#"{"editable":{"rows":false}}"#;
const NO_OUTLINE: &str = r#"{"editable":{"outline":false}}"#;
const ROWS: EditError = EditError::NotAllowed("adding and deleting rows");
const OUTLINE: EditError = EditError::NotAllowed("changing the outline");

fn limited(json: &str) -> Session {
    let mut s = session();
    s.set_capabilities(json).unwrap();
    s
}

/// A press on task 2's bar, or on its finish edge, dragged far to the right: whether it changed the
/// chart, and the note it left while the pointer was still down.
fn dragged(json: &str, edge: bool) -> (bool, Option<String>) {
    let mut s = limited(json);
    let (before, (x, y)) = (state(&s), on(&s, 2));
    let x = if edge { x + 174.0 } else { x };
    s.pointer_down(x, y, false);
    s.pointer_move(x + 1500.0, y);
    s.apply_drag();
    let note = s.drag_note().map(str::to_string);
    s.pointer_up();
    (state(&s) != before, note)
}

const HOURS: &str = r#"{"durationStep":"hours","lagStep":"hours"}"#;

fn stores(what: &str) -> EditError {
    EditError::NotStored(what.into())
}

/// A chart whose working day is `day` minutes from 08:00, under the profile `json`.
fn host(day: u16, json: &str) -> Session {
    let mut s = session_on(&[(480, 480 + day)]);
    s.set_capabilities(json).unwrap();
    s
}

/// A press on task 2's bar, or its finish edge, dragged `by` pixels: whether it changed the chart,
/// and the note it left.
fn dragged_by(s: &mut Session, edge: bool, by: f64) -> (bool, Option<String>) {
    let (before, (x, y)) = (state(s), on(s, 2));
    let x = if edge { x + 174.0 } else { x };
    s.pointer_down(x, y, false);
    s.pointer_move(x + by, y);
    s.apply_drag();
    let note = s.drag_note().map(str::to_string);
    s.pointer_up();
    (state(s) != before, note)
}

#[test]
fn kinds_refuse_a_limit_of_another_kind_and_let_a_start_through() {
    let mut s = host(450, r#"{"kinds":["snet"]}"#);
    let before = state(&s);
    let typed = s.set_task_fields(TaskId(0), &[("fnlt", "2026-01-09")]);
    assert_eq!(typed, Err("Saving allows no fnlt limit.".to_string()));
    let pin = Limit {
        pin: Some(Pin::Start(0)),
        ..Limit::default()
    };
    let (task, limit) = (TaskId(0), pin);
    assert_eq!(
        s.edit(Edit::SetLimit { task, limit }),
        Err(stores("no mso limit"))
    );
    assert_eq!(state(&s), before);
    s.set_start_text(task, "2026-01-06").unwrap();
}

#[test]
fn per_task_refuses_a_second_limit_and_a_bar_move_beside_one() {
    let mut s = host(450, r#"{"perTask":1}"#);
    assert_eq!(
        dragged_by(&mut s, false, 1500.0),
        (true, None),
        "a first limit"
    );
    let mut s = host(450, r#"{"perTask":1}"#);
    s.set_task_fields(TaskId(2), &[("fnlt", "2026-01-09")])
        .unwrap();
    let before = state(&s);
    let said = "Saving allows date limits: at most 1 for a task.";
    let second = s.set_task_fields(TaskId(2), &[("snlt", "2026-01-08")]);
    assert_eq!(second, Err(said.to_string()));
    assert_eq!(
        dragged_by(&mut s, false, 1500.0),
        (false, Some(said.into()))
    );
    assert_eq!(state(&s), before);
}

#[test]
fn hours_refuse_a_part_hour_duration_lag_and_resize_and_days_a_part_day() {
    let mut s = host(450, HOURS);
    let before = state(&s);
    assert_eq!(s.edit(longer(90)), Err(stores("durations in whole hours")));
    let lag = s.set_link_text(DepId(0), "FS+30m");
    assert_eq!(lag, Err(stores("lags in whole hours")));
    let said = "Saving allows durations in whole hours.";
    assert_eq!(dragged_by(&mut s, true, 100.0), (false, Some(said.into())));
    assert_eq!(state(&s), before);
    s.edit(longer(120)).unwrap();
    s.set_link_text(DepId(0), "FS+2h").unwrap();
    let mut s = host(450, r#"{"durationStep":"days"}"#);
    assert_eq!(s.edit(longer(240)), Err(stores("durations in whole days")));
    s.edit(longer(450)).unwrap();
}

#[test]
fn a_paste_is_a_day_of_its_calendar_and_hours_refuse_a_day_that_is_not_whole() {
    let refused = host(450, HOURS).insert_many(&["Footing"]).map(drop);
    assert_eq!(refused, Err(stores("durations in whole hours")));
    host(480, HOURS).insert_many(&["Footing"]).unwrap();
}

#[test]
fn dates_by_the_day_allow_a_calendars_opening_and_closing_and_refuse_any_other_time() {
    let day = r#"{"dates":"day"}"#;
    let fnlt = |at| Edit::SetLimit {
        task: TaskId(2),
        limit: Limit {
            fnlt: Some(at),
            ..Limit::default()
        },
    };
    let no = || Err(stores("dates without a time of day"));
    let mut s = host(450, day);
    let typed = s.set_start_text(TaskId(0), "2026-01-06 08:30").map(drop);
    assert_eq!(
        typed,
        Err(StartError::Edit(stores("dates without a time of day")))
    );
    s.set_start_text(TaskId(0), "2026-01-06").unwrap();
    s.edit(fnlt(2 * 1440 + 930)).unwrap();
    assert_eq!(s.edit(fnlt(2 * 1440 + 900)), no());
    assert_eq!(dragged_by(&mut s, false, 1500.0), (true, None));
    let mut s = limited(day);
    s.edit(fnlt(3 * 1440)).unwrap();
    assert_eq!(s.edit(fnlt(2 * 1440 + 930)), no());
}

#[test]
fn a_dialog_is_checked_for_the_limits_it_leaves_not_one_field_at_a_time() {
    let mut s = host(450, r#"{"perTask":1}"#);
    let two = [("snlt", "2026-01-08"), ("fnlt", "2026-01-09")];
    assert!(s.set_task_fields(TaskId(2), &two).is_err());
    s.set_task_fields(TaskId(2), &two[1..]).unwrap();
    s.set_task_fields(TaskId(2), &[("fnlt", ""), ("snlt", "2026-01-08")])
        .unwrap();
}

#[test]
fn a_floor_and_a_ceiling_on_one_instant_are_the_one_kind_so_or_fo_unless_the_host_leaves_it_out() {
    let start = [("start", "2026-01-07 08:00"), ("snlt", "2026-01-07 08:00")];
    let finish = [("fnet", "2026-01-07"), ("fnlt", "2026-01-07")];
    let mut s = host(450, r#"{"kinds":["so","fo"],"perTask":1}"#);
    s.set_task_fields(TaskId(3), &finish).unwrap();
    let apart = [("start", "2026-01-07 08:00"), ("snlt", "2026-01-08 08:00")];
    let said = Err("Saving allows no snet limit.".to_string());
    assert_eq!(s.set_task_fields(TaskId(2), &apart), said);
    s.set_task_fields(TaskId(2), &start).unwrap();
    let mut s = host(
        450,
        r#"{"kinds":["snet","snlt","fnet","fnlt"],"perTask":1}"#,
    );
    let said = Err("Saving allows date limits: at most 1 for a task.".to_string());
    assert_eq!(s.set_task_fields(TaskId(2), &start), said);
}

#[test]
fn every_value_member_at_its_default_allows_everything() {
    let kinds = r#""kinds":["snet","snlt","fnet","fnlt","mso","mfo","alap"]"#;
    let steps = r#""durationStep":"minutes","lagStep":"minutes","dates":"minute""#;
    let mut s = host(450, &format!("{{{kinds},{steps}}}"));
    s.edit(longer(90)).unwrap();
    s.set_link_text(DepId(0), "FS+30m").unwrap();
    s.set_start_text(TaskId(0), "2026-01-06 08:30").unwrap();
    let limit = Limit {
        pin: Some(Pin::Finish(900)),
        ..Limit::default()
    };
    s.edit(Edit::SetLimit {
        task: TaskId(1),
        limit,
    })
    .unwrap();
    s.insert_many(&["Footing"]).unwrap();
}

#[test]
fn rows_off_refuses_insert_delete_and_paste_and_lets_an_outdent_through() {
    let mut s = limited(NO_ROWS);
    s.select(1);
    assert_eq!(s.insert_below_selection().map(drop), Err(ROWS));
    assert_eq!(s.delete_selected().map(drop), Err(ROWS));
    assert_eq!(s.insert_many(&["Footing"]).map(drop), Err(ROWS));
    s.select(5);
    s.outdent_selected().unwrap();
}

#[test]
fn outline_off_refuses_outdent_move_and_redo_and_lets_rows_and_their_undo_through() {
    let mut s = session();
    s.select(5);
    s.outdent_selected().unwrap();
    s.undo().unwrap();
    s.set_capabilities(NO_OUTLINE).unwrap();
    assert_eq!(s.redo(), Err(OUTLINE));
    s.select(1);
    s.insert_below_selection().unwrap();
    s.delete_selected().unwrap();
    assert_eq!((s.undo(), s.undo()), (Ok(true), Ok(true)));
    let before = state(&s);
    s.select(3);
    assert_eq!(s.move_selected_up().map(drop), Err(OUTLINE));
    s.select(5);
    assert_eq!(s.outdent_selected().map(drop), Err(OUTLINE));
    assert_eq!(state(&s), before, "a refusal changes nothing");
}

#[test]
fn paste_off_refuses_a_paste_and_lets_an_insert_through() {
    let mut s = limited(r#"{"editable":{"paste":false}}"#);
    assert_eq!(
        s.insert_many(&["Footing"]).map(drop),
        Err(EditError::NotAllowed("pasting rows"))
    );
    s.insert_below_selection().unwrap();
}

#[test]
fn every_member_at_its_default_allows_everything() {
    let mut s = limited(r#"{"editable":{"rows":true,"outline":true,"paste":true}}"#);
    s.insert_many(&["Footing"]).unwrap();
    s.select(5);
    s.outdent_selected().unwrap();
}

#[test]
fn bar_move_off_refuses_a_move_and_still_resizes() {
    let json = r#"{"gestures":{"barMove":false}}"#;
    let said = EditError::NotAllowed("moving a bar").to_string();
    assert_eq!(dragged(json, false), (false, Some(said)));
    assert_eq!(dragged(json, true), (true, None));
}

#[test]
fn each_refusal_of_what_the_host_turned_off_has_a_sentence_and_the_host_can_word_it() {
    let said = [
        ("moving a bar", "Moving a bar is not allowed."),
        ("resizing a bar", "Resizing a bar is not allowed."),
        ("drawing a link", "Drawing a link is not allowed."),
        (
            "adding and deleting rows",
            "Adding and deleting rows is not allowed.",
        ),
        (
            "changing the outline",
            "Changing the outline is not allowed.",
        ),
        ("pasting rows", "Pasting rows is not allowed."),
        ("changing names", "Changing names is not allowed."),
        (
            "changing dates and durations",
            "Changing dates and durations is not allowed.",
        ),
        ("changing links", "Changing links is not allowed."),
        (
            "changing progress and the data date",
            "Changing progress and the data date is not allowed.",
        ),
        (
            "changing the baseline",
            "Changing the baseline is not allowed.",
        ),
    ];
    for (what, sentence) in said {
        assert_eq!(EditError::NotAllowed(what).to_string(), sentence);
    }
    let names = r#""months":["a","b","c","d","e","f","g","h","i","j","k","l"],"weekdays":["a","b","c","d","e","f","g"],"weekStart":0"#;
    let mut s = limited(r#"{"gestures":{"barMove":false}}"#);
    s.set_locale(&format!(
        r#"{{{names},"texts":{{"notAllowedBarMove":"Fixed bar."}}}}"#
    ))
    .unwrap();
    assert_eq!(s.say(&EditError::NotAllowed("moving a bar")), "Fixed bar.");
    assert_eq!(
        s.say(&EditError::NotAllowed("pasting rows")),
        "Pasting rows is not allowed."
    );
}

#[test]
fn resize_off_refuses_an_edge_drag_and_still_moves() {
    let json = r#"{"gestures":{"resize":false}}"#;
    let said = EditError::NotAllowed("resizing a bar").to_string();
    assert_eq!(dragged(json, true), (false, Some(said)));
    assert_eq!(dragged(json, false), (true, None));
}

#[test]
fn link_off_adds_no_link_and_the_press_still_selects() {
    let mut s = limited(r#"{"gestures":{"link":false}}"#);
    let (before, to) = (state(&s), on(&s, 2));
    link(&mut s, 0, to);
    assert_eq!(state(&s), before);
    assert_eq!(s.selection().map(|selected| selected.task), Some(0));
    let said = EditError::NotAllowed("drawing a link").to_string();
    assert_eq!(s.drag_note(), Some(said.as_str()));
}

#[test]
fn links_off_refuses_a_link_drawn_by_drag_with_the_gesture_on() {
    let mut s = limited(r#"{"editable":{"links":false}}"#);
    let (before, to) = (state(&s), on(&s, 2));
    link(&mut s, 0, to);
    assert_eq!(state(&s), before);
    let said = EditError::NotAllowed("changing links").to_string();
    assert_eq!(s.drag_note(), Some(said.as_str()));
}

#[test]
fn dates_off_refuses_a_dialog_whole_and_names_the_duration() {
    let mut s = limited(r#"{"editable":{"dates":false}}"#);
    let before = state(&s);
    let fields = [("name", "Renamed"), ("duration", "5h")];
    let said = s.set_task_fields(TaskId(0), &fields).unwrap_err();
    assert_eq!(said, "Changing dates and durations is not allowed.");
    assert_eq!((state(&s), s.meta()[0].name.as_str()), (before, "A"));
    s.set_task_fields(TaskId(0), &fields[..1]).unwrap();
    assert_eq!(s.meta()[0].name, "Renamed", "the name alone passes");
}

#[test]
fn undo_of_a_delete_is_refused_with_rows_off() {
    let mut s = session();
    s.select(1);
    s.delete_selected().unwrap();
    s.set_capabilities(NO_ROWS).unwrap();
    let before = state(&s);
    assert_eq!(s.undo(), Err(ROWS));
    assert_eq!(state(&s), before);
    s.set_capabilities("{}").unwrap();
    assert_eq!(s.undo(), Ok(true), "off again, the delete undoes");
}

#[test]
fn undo_of_a_data_date_is_refused_with_progress_off() {
    let mut s = session();
    s.set_data_date_text("2026-01-05").unwrap();
    s.set_capabilities(r#"{"editable":{"progress":false}}"#)
        .unwrap();
    let before = state(&s);
    let refused = EditError::NotAllowed("changing progress and the data date");
    assert_eq!(s.undo(), Err(refused));
    assert_eq!(state(&s), before, "the date stays");
}

#[test]
fn read_only_refuses_first_and_the_host_function_is_not_asked_after_a_refusal() {
    let asked = Rc::new(Cell::new(0));
    let count = Rc::clone(&asked);
    let mut s = limited(NO_ROWS);
    s.set_before_edit(Some(Rc::new(move |_| {
        count.set(count.get() + 1);
        Ok(())
    })));
    assert_eq!(s.insert_below_selection().map(drop), Err(ROWS));
    assert_eq!(asked.get(), 0, "a profile refusal never reaches it");
    s.edit(longer(300)).unwrap();
    assert_eq!(asked.get(), 1, "an allowed edit does");
    s.set_read_only(true);
    assert_eq!(
        s.insert_below_selection().map(drop),
        Err(EditError::ReadOnly)
    );
}

#[test]
fn a_profile_that_is_not_one_is_refused_by_name_and_changes_nothing() {
    let mut s = limited(NO_ROWS);
    for (json, said) in [
        (
            r#"{"gestures":{"drag":false}}"#,
            "gestures.drag: unknown key",
        ),
        (r#"{"edit":{}}"#, "edit: unknown key"),
        (
            r#"{"editable":{"structure":false}}"#,
            "editable.structure: unknown key",
        ),
        (
            r#"{"editable":{"dates":0}}"#,
            "editable.dates: expected true or false",
        ),
        ("[]", "expected an object, found an array"),
        (
            r#"{"durationStep":"hour"}"#,
            "durationStep: expected one of minutes, hours, days",
        ),
        (
            r#"{"perTask":-1}"#,
            "perTask: expected a whole number, 0 or more",
        ),
    ] {
        assert_eq!(s.set_capabilities(json), Err(said.to_string()), "{json}");
    }
    assert_eq!(s.insert_below_selection().map(drop), Err(ROWS));
}

#[test]
fn no_profile_allows_everything_and_an_empty_one_takes_a_profile_away() {
    let mut s = session();
    s.insert_below_selection().unwrap();
    s.set_capabilities(NO_ROWS).unwrap();
    assert_eq!(s.insert_below_selection().map(drop), Err(ROWS));
    s.set_capabilities("{}").unwrap();
    s.insert_below_selection().unwrap();
}

#[test]
fn reopen_keeps_the_profile() {
    let s = limited(NO_ROWS);
    let meta = s.meta().to_vec();
    let order = (0..meta.len() as u32).collect();
    let (schedule, solution) = (s.schedule().clone(), s.solution().clone());
    let fresh = s.reopen(schedule, solution, order, meta, None, None, Origin::DEMO);
    assert_eq!(fresh.unwrap().insert_below_selection().map(drop), Err(ROWS));
}
