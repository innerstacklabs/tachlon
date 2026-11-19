//! Every undo step is a commit (decision 0019), read back from the outbox. The chain of `start_edit`:
//! three tasks of eight working hours, day 0 is Monday 2026-01-05.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::{clock, zoom_to};
use crate::link_gesture;
use crate::start_edit::{chain, session};
use app::{Session, edit::Edit, host::OUTBOX_CAP, meta::TaskMeta};
use engine::{CalendarId, DepId, LagCalendar, Limit, LinkType, Pin, TaskId};
use serde_json::{Value, json};
use timeline::Timeline;

const FIRST: TaskId = TaskId(0);

/// The commits read, as `seq source` and then `task field: old -> new` for each change.
fn drained(s: &mut Session) -> Vec<String> {
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    assert_eq!(out["snapshot"], false);
    let mut lines = Vec::new();
    for c in out["commits"].as_array().unwrap() {
        lines.push(format!("{} {}", c["seq"], c["source"].as_str().unwrap()));
        for c in c["changes"].as_array().unwrap() {
            let field = c["field"].as_str().unwrap();
            lines.push(format!(
                "{} {field}: {} -> {}",
                c["task"], c["old"], c["new"]
            ));
        }
    }
    lines
}

fn rename(s: &mut Session, name: &str) {
    let (task, name) = (FIRST, name.into());
    s.edit(Edit::Rename { task, name }).unwrap();
}

fn duration(s: &mut Session, minutes: i64) {
    let task = FIRST;
    s.edit(Edit::SetDuration { task, minutes }).unwrap();
}

#[test]
fn each_typed_field_is_a_change_in_the_host_forms() {
    let mut s = session();
    s.set_percent_text(TaskId(2), "100").unwrap();
    let want = [
        r#"2 actualStart: null -> "2026-01-07T08:00""#,
        r#"2 actualFinish: null -> "2026-01-07T17:00""#,
        "2 remaining: null -> 0.0",
    ];
    assert_eq!(drained(&mut s)[1..], want);
    s.set_finish_text(FIRST, "2026-01-07").unwrap();
    assert_eq!(drained(&mut s)[1..], ["0 duration: 1.0 -> 3.0"]);
    s.set_start_text(FIRST, "2026-01-12").unwrap();
    let want = [r#"0 snet: null -> "2026-01-12T00:00""#];
    assert_eq!(drained(&mut s)[1..], want);
    duration(&mut s, 720);
    assert_eq!(drained(&mut s)[1..], ["0 duration: 3.0 -> 1.5"]);
    rename(&mut s, "Pour");
    assert_eq!(drained(&mut s)[1..], [r#"0 name: "" -> "Pour""#]);
    s.set_progress_text(TaskId(1), "2026-01-05", "2026-01-06", "")
        .unwrap();
    let want = [
        r#"1 actualStart: null -> "2026-01-05T00:00""#,
        r#"1 actualFinish: null -> "2026-01-06T00:00""#,
    ];
    assert_eq!(drained(&mut s)[1..], want);
    s.set_progress_text(TaskId(1), "", "", "").unwrap();
    let want = [
        r#"1 actualStart: "2026-01-05T00:00" -> null"#,
        r#"1 actualFinish: "2026-01-06T00:00" -> null"#,
    ];
    assert_eq!(drained(&mut s)[1..], want);
}

#[test]
fn a_typed_duration_its_undo_and_its_redo_are_three_commits() {
    let mut s = session();
    duration(&mut s, 720);
    assert!(s.undo().unwrap() && s.redo().unwrap());
    let (up, down) = ("0 duration: 1.0 -> 1.5", "0 duration: 1.5 -> 1.0");
    let want = ["1 typed", up, "2 undo", down, "3 redo", up];
    assert_eq!(drained(&mut s), want);
    assert!(s.undo().unwrap());
    assert_eq!(drained(&mut s)[0], "4 undo", "seq goes on after a read");
}

/// Press task 0's bar `px` pixels left of its finish, move to `to` and release: the one `drag` commit's changes.
fn drag(s: &mut Session, px: f64, to: f64) -> Vec<String> {
    let v = s.timeline().viewport.clone();
    let (x, y) = (v.x_of(1020.0) - px, v.row_top(0) + v.row_height / 2.0);
    s.pointer_down(x, y, false);
    s.pointer_move(v.x_of(to), y);
    s.apply_drag();
    assert_eq!(drained(s), [] as [String; 0], "no commit per frame");
    s.pointer_up();
    let done = drained(s);
    assert!(done[0] == "1 drag" && done.len() == 2, "{done:?}");
    done[1..].to_vec()
}

#[test]
fn a_bar_drag_and_an_edge_drag_are_one_drag_commit_each_at_release() {
    let on_screen = || {
        let mut s = session();
        s.fit(0.0, 10_000.0);
        s
    };
    let (mut bar, mut edge) = (on_screen(), on_screen());
    let moved = drag(&mut bar, 20.0, 4_000.0);
    assert!(moved[0].starts_with("0 snet: null -> "), "{moved:?}");
    let longer = drag(&mut edge, 1.0, 4_000.0);
    assert!(longer[0].starts_with("0 duration: 1.0 -> "), "{longer:?}");
    assert!(bar.undo().unwrap());
    let to = moved[0].strip_prefix("0 snet: null -> ").unwrap();
    assert_eq!(
        drained(&mut bar),
        ["2 undo", &format!("0 snet: {to} -> null")]
    );
}

#[test]
fn the_dialog_s_three_fields_are_one_dialog_commit() {
    let mut s = session();
    let fields = [
        ("name", "Pour"),
        ("duration", "16h"),
        ("start", "2026-01-12"),
    ];
    s.set_task_fields(FIRST, &fields).unwrap();
    let want = [
        "1 dialog",
        r#"0 name: "" -> "Pour""#,
        "0 duration: 1.0 -> 2.0",
        r#"0 snet: null -> "2026-01-12T00:00""#,
    ];
    assert_eq!(drained(&mut s), want);
}

#[test]
fn an_accepted_proposal_is_one_commit() {
    let mut s = crate::what_moves::session();
    s.propose_duration(1, 3);
    s.accept_proposal();
    let want = ["1 proposal", "1 duration: 1.0 -> 3.0"];
    assert_eq!(drained(&mut s), want);
}

#[test]
fn a_refused_entry_and_a_rolled_back_group_commit_nothing_and_take_no_seq() {
    let mut s = session();
    s.set_start_text(FIRST, "soon").unwrap_err();
    let fields = [("name", "Pour"), ("duration", "soon")];
    s.set_task_fields(FIRST, &fields).unwrap_err();
    assert_eq!(drained(&mut s), [] as [String; 0]);
    rename(&mut s, "Pour");
    assert_eq!(drained(&mut s)[0], "1 typed");
}

#[test]
fn the_outbox_empties_past_its_cap_and_says_to_take_a_snapshot() {
    let mut s = session();
    for n in 0..=OUTBOX_CAP {
        rename(&mut s, &format!("n{n}"));
    }
    let seq = OUTBOX_CAP + 1;
    let want = format!(r#"{{"commits":[],"snapshot":true,"seq":{seq}}}"#);
    assert_eq!(s.take_commits(), want);
    rename(&mut s, "again");
    assert_eq!(drained(&mut s)[0], format!("{} typed", seq + 1));
}

#[test]
fn the_dialog_s_limit_fields_are_three_updates_in_one_commit() {
    let mut s = session();
    let fields = [
        ("snlt", "2026-02-02"),
        ("fnet", "2026-02-03"),
        ("fnlt", "2026-02-04"),
    ];
    s.set_task_fields(FIRST, &fields).unwrap();
    let want = [
        "1 dialog",
        r#"0 snlt: null -> "2026-02-02T00:00""#,
        r#"0 fnet: null -> "2026-02-04T00:00""#,
        r#"0 fnlt: null -> "2026-02-05T00:00""#,
    ];
    assert_eq!(drained(&mut s), want);
}

#[test]
fn a_limit_with_a_pin_alap_and_resume_is_one_commit_of_the_changed_fields() {
    let mut s = session();
    let limit = Limit {
        pin: Some(Pin::Start(0)),
        alap: true,
        resume: Some(0),
        ..Limit::default()
    };
    s.edit(Edit::SetLimit { task: FIRST, limit }).unwrap();
    let want = [
        "1 typed",
        r#"0 mso: null -> "2026-01-05T00:00""#,
        "0 alap: false -> true",
        r#"0 resume: null -> "2026-01-05T00:00""#,
    ];
    assert_eq!(drained(&mut s), want);
}

/// The commits read, as `source` then each change's JSON.
fn raw(s: &mut Session) -> Vec<Value> {
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    let commits = out["commits"].as_array().unwrap();
    commits.iter().map(|c| c["changes"].clone()).collect()
}

#[test]
fn a_link_is_a_create_an_update_per_field_a_delete_and_its_undo_a_create() {
    let mut s = session();
    let (pred, succ) = (TaskId(0), TaskId(2));
    let slot = s.schedule().dependencies().len();
    let dep = DepId(u32::try_from(slot).unwrap());
    let add = Edit::AddLink {
        pred,
        succ,
        kind: LinkType::FS,
        lag: 0,
    };
    s.edit(add).unwrap();
    s.set_link_text(dep, "SS+2d").unwrap();
    s.edit(Edit::RemoveLink { dep }).unwrap();
    assert!(s.undo().unwrap());
    let ends = json!({"from":{"task":0},"to":{"task":2}});
    let create = |kind: &str, lag: f64| {
        let fields = json!({"from":ends["from"],"to":ends["to"],"type":kind,"lag":lag});
        json!([{"record":"link","key":null,"link":slot,"op":"create","fields":fields}])
    };
    let update = |field: &str, old: Value, new: Value| json!({"record":"link","key":null,"link":slot,"op":"update","field":field,"old":old,"new":new});
    let delete = json!([{"record":"link","key":null,"link":slot,"op":"delete","fields":create("SS", 2.0)[0]["fields"]}]);
    let want = [
        create("FS", 0.0),
        json!([
            update("type", json!("FS"), json!("SS")),
            update("lag", json!(0.0), json!(2.0))
        ]),
        delete,
        create("SS", 2.0),
    ];
    assert_eq!(raw(&mut s), want);
}

#[test]
fn a_data_date_set_and_cleared_is_two_schedule_updates() {
    let mut s = session();
    s.edit(Edit::SetDataDate { date: Some(0) }).unwrap();
    s.edit(Edit::SetDataDate { date: None }).unwrap();
    let change = |old: Value, new: Value| json!([{"record":"schedule","op":"update","field":"dataDate","old":old,"new":new}]);
    let day = json!("2026-01-05T00:00");
    let want = [change(Value::Null, day.clone()), change(day, Value::Null)];
    assert_eq!(raw(&mut s), want);
}

/// `start_edit`'s chain with task 0 and task 2 keyed, and its links' lags counted in `lag`.
fn keyed(lag: LagCalendar) -> Session {
    let mut schedule = chain();
    schedule.set_lag_calendar(lag).unwrap();
    let solution = schedule.compute().unwrap();
    let key = |k: &str| TaskMeta {
        key: Some(k.into()),
        ..TaskMeta::default()
    };
    let meta = vec![key("a1"), TaskMeta::default(), key("c3")];
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    let mut s = Session::new(schedule, solution, timeline, meta, clock);
    zoom_to(&mut s, 4.0, 0.0);
    s
}

#[test]
fn a_link_drawn_between_keyed_tasks_names_them_by_key() {
    let mut s = keyed(LagCalendar::Successor);
    let to = link_gesture::on(&s, 2);
    link_gesture::link(&mut s, 0, to);
    let fields = json!({"from":"a1","to":"c3","type":"FS","lag":0.0});
    let want = json!([{"record":"link","key":null,"link":2,"op":"create","fields":fields}]);
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    assert_eq!(out["commits"][0]["source"], "drag");
    assert_eq!(out["commits"][0]["changes"], want);
}

#[test]
fn a_lag_on_the_24_hour_lag_calendar_counts_1440_minutes_a_day() {
    let mut s = keyed(LagCalendar::TwentyFourHour);
    let kind = LinkType::FS;
    let dep = DepId(0);
    s.edit(Edit::SetLink {
        dep,
        kind,
        lag: 1440,
    })
    .unwrap();
    let want = json!([{"record":"link","key":null,"link":0,"op":"update",
        "field":"lag","old":0.0,"new":1.0}]);
    assert_eq!(raw(&mut s), [want]);
}

#[test]
fn the_working_day_of_each_task_and_link_is_read_by_slot() {
    let days = |lag| serde_json::from_str::<Value>(&keyed(lag).day_lengths()).unwrap();
    let own = days(LagCalendar::Successor);
    assert_eq!(own["links"][0], own["tasks"][1], "the successor's day");
    assert_eq!(days(LagCalendar::TwentyFourHour)["links"][0], 1440);
}

/// The create the undo of a link's removal writes, the link having its own lag calendar `own`.
fn restored(own: LagCalendar, lag: i64) -> Value {
    let mut schedule = chain();
    let own = Some(own);
    schedule.set_dependency_lag_calendar(DepId(0), own).unwrap();
    let solution = schedule.compute().unwrap();
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    let meta = vec![TaskMeta::default(); 3];
    let mut s = Session::new(schedule, solution, timeline, meta, clock);
    let kind = LinkType::FS;
    s.edit(Edit::SetLink {
        dep: DepId(0),
        kind,
        lag,
    })
    .unwrap();
    s.edit(Edit::RemoveLink { dep: DepId(0) }).unwrap();
    assert!(s.undo().unwrap());
    raw(&mut s)[2][0]["fields"].take()
}

#[test]
fn a_removed_link_with_its_own_lag_calendar_is_restored_with_that_calendar() {
    let fields =
        json!({"from":{"task":0},"to":{"task":1},"type":"FS","lag":1.0,"lagCalendar":"24h"});
    assert_eq!(restored(LagCalendar::TwentyFourHour, 1440), fields);
}

#[test]
fn a_fixed_lag_calendar_with_no_key_is_named_by_its_slot() {
    let fields = restored(LagCalendar::Fixed(CalendarId(0)), 480);
    assert_eq!(fields["lagCalendar"], json!({"calendar":0}));
}

#[test]
fn a_baseline_set_is_a_commit_with_no_change() {
    let mut s = session();
    s.edit(Edit::SetBaseline { baseline: None }).unwrap();
    assert_eq!(raw(&mut s), [json!([])]);
    rename(&mut s, "after");
    assert_eq!(
        drained(&mut s)[0],
        "2 typed",
        "seq rose for the baseline commit"
    );
}
