//! The critical tasks and the longest path, listed for a program (`Session::critical_path`).
//!
//! The fixture is the engine's `longest_path_differs_from_zero_float_with_snet`: X (Monday-to-Friday
//! 08:00-17:00) starts Friday 08:00, Y (24/7, ten hours) cannot start before Saturday 10:00, and W
//! (24/7, one hour) follows Y. The weekend between X and Y holds no working time of X's calendar, so
//! X has zero float, yet Y's start-no-earlier-than date, not X, sets Y's start: X is critical and is
//! not on the longest path.

use crate::common;

use app::Session;
use engine::{Calendar, DAY, Dependency, LinkType, Schedule, Task, TaskId};
use serde_json::{Value, json};
use timeline::Timeline;

fn task(schedule: &mut Schedule, calendar: engine::CalendarId, minutes: i64, snet: Option<i64>) {
    let task = Task {
        duration: minutes,
        calendar,
        snet,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    schedule
        .add_task(task)
        .expect("calendar belongs to the schedule");
}

fn session(mut schedule: Schedule) -> Session {
    let solution = schedule.compute().expect("the fixture has no loop");
    let count = schedule.tasks().len();
    let meta = (0..count).map(|i| app::meta::TaskMeta {
        name: format!("task {i}"),
        ..app::meta::TaskMeta::default()
    });
    let order = (0..u32::try_from(count).expect("a small fixture")).collect();
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta.collect(), common::clock)
}

fn snet_session() -> Session {
    let mut schedule = Schedule::new();
    let weekday = vec![(480, 1020)];
    let standard = Calendar::new(
        [
            weekday.clone(),
            weekday.clone(),
            weekday.clone(),
            weekday.clone(),
            weekday,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("valid standard calendar");
    let continuous = Calendar::new(std::array::from_fn(|_| vec![(0, 1440)]), &[])
        .expect("valid continuous calendar");
    let (standard, continuous) = (
        schedule.add_calendar(standard),
        schedule.add_calendar(continuous),
    );
    task(&mut schedule, standard, 540, Some(4 * DAY + 480));
    task(&mut schedule, continuous, 600, Some(5 * DAY + 600));
    task(&mut schedule, continuous, 60, None);
    for (pred, succ) in [(0, 1), (1, 2)] {
        let (pred, succ) = (TaskId(pred), TaskId(succ));
        let link = Dependency {
            pred,
            succ,
            kind: LinkType::FS,
            lag: 0,
        };
        schedule.add_dependency(link).expect("tasks exist");
    }
    session(schedule)
}

fn ask(session: &Session) -> Value {
    serde_json::from_str(&session.critical_path()).expect("critical_path answers JSON")
}

fn held(answer: &Value, key: &str) -> Vec<u64> {
    let tasks = answer[key]["tasks"].as_array().expect("a list");
    let numbers = tasks.iter().map(|t| t["task"].as_u64().expect("a number"));
    numbers.collect()
}

/// The longest path is a subset of the critical set: here X has zero float and drives nothing.
#[test]
fn the_longest_path_differs_from_the_zero_float_tasks() {
    let session = snet_session();
    let answer = ask(&session);
    assert_eq!(held(&answer, "critical"), [0, 1, 2]);
    assert_eq!(answer["critical"]["count"], 3);
    assert_eq!(held(&answer, "longest"), [1, 2]);
    assert_eq!(answer["longest"]["count"], 2);
    let engine = session.schedule().longest_path(session.solution());
    assert_eq!(engine, [TaskId(1), TaskId(2)], "the engine's answer");
    assert_eq!(
        answer["longest"]["tasks"][0],
        json!({"task": 1, "id": "", "name": "task 1"})
    );
}

#[test]
fn sixty_critical_tasks_list_fifty_and_count_sixty() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(common::collapse::standard_calendar());
    for _ in 0..60 {
        task(&mut schedule, calendar, 480, None);
    }
    let answer = ask(&session(schedule));
    for key in ["critical", "longest"] {
        assert_eq!(answer[key]["count"], 60, "{key}");
        assert_eq!(held(&answer, key), (0..50).collect::<Vec<u64>>(), "{key}");
    }
}

/// Asking reads only: the solution, the undo history and the stamp are as they were.
#[test]
fn asking_changes_nothing() {
    let session = snet_session();
    let (solution, history, stamp) = (
        session.solution().clone(),
        session.history_len(),
        session.stamp(),
    );
    assert_eq!(session.critical_path(), session.critical_path());
    assert_eq!(session.solution(), &solution);
    assert_eq!((session.history_len(), session.stamp()), (history, stamp));
}

#[test]
fn asking_during_a_press_is_refused() {
    let (mut session, _, _, (x, y)) = common::setup();
    session.pointer_down(x, y, false);
    assert_eq!(
        ask(&session),
        json!({"ok": false, "reason": "busy", "message": "Finish the drag first."})
    );
}
