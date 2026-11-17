//! The Check row filter (gs#703): the table keeps the unfinished leaves an open start, an open end
//! or missing logic counts, and the summaries above them as context.
//!
//! ```text
//! 0 summary { 1, 2, 3 }   1 -> 2 FS, 2 -> 4 FS; 5 is finished and linkless
//! ```
//!
//! Open starts are 1 and 3, open ends are 3 and 4, so missing logic is 1, 3 and 4.

use crate::common;

use app::{
    RowFilter, Session, check_filter::CheckKind, checks::Checks, edit::Edit, meta::TaskMeta,
    outline::Outline, status_filter::StatusKind,
};
use common::{clock, collapse::standard_calendar};
use engine::{DAY, Dependency, Limit, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

const HOUR: i64 = 60;

fn session() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for index in 0..6 {
        schedule
            .add_task(Task {
                duration: 8 * HOUR,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: (index == 5).then_some(0),
                actual_finish: (index == 5).then_some(8 * HOUR),
                remaining: None,
            })
            .expect("the calendar is in the schedule");
    }
    for (pred, succ) in [(1, 2), (2, 4)] {
        let (pred, succ) = (TaskId(pred), TaskId(succ));
        let kind = LinkType::FS;
        let dep = Dependency {
            pred,
            succ,
            kind,
            lag: 0,
        };
        schedule.add_dependency(dep).expect("tasks exist");
    }
    let mut meta = vec![TaskMeta::default(); 6];
    for leaf in &mut meta[1..=3] {
        leaf.parent = Some(TaskId(0));
    }
    let wbs: Vec<TaskId> = (0..6).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a one-summary outline");
    let solution = schedule.compute().expect("computes");
    let timeline = Timeline::new((0..6).collect(), 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline fits")
}

fn listed(s: &Session) -> Vec<u32> {
    s.timeline().order().to_vec()
}

fn five_check_session() -> Session {
    let mut schedule = common::collapse::schedule_with_tasks(3);
    for (task, days) in [(0, 1), (1, 46), (2, 1)] {
        let duration = days * 8 * HOUR;
        schedule
            .set_duration(TaskId(task), duration)
            .expect("a task");
    }
    let late = Some(2 * DAY);
    schedule.set_actual_start(TaskId(2), late).expect("a task");
    for (task, at) in [(0, 100 * DAY), (1, DAY)] {
        let limit = Limit {
            fnlt: Some(at),
            ..Limit::default()
        };
        schedule.set_limit(TaskId(task), limit).expect("a task");
    }
    let solution = schedule.compute().expect("computes");
    let meta = vec![TaskMeta::default(); 3];
    let wbs = [TaskId(0), TaskId(1), TaskId(2)];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a flat outline");
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    let mut s = Session::new(schedule, solution, timeline, meta, clock)
        .with_resources(vec![vec!["Crew".into()], vec!["Crew".into()], vec![]])
        .with_outline(outline)
        .expect("the outline fits");
    common::baseline::set(&mut s);
    s.edit(Edit::SetDataDate { date: Some(DAY) })
        .expect("a data date");
    s
}

/// High float `[0]`, high duration `[1]`, invalid dates `[2]`, missing resources `[2]`, missed `[0]`, hard constraints `[0, 1]`, negative float `[1]`.
#[test]
fn seven_health_checks_keep_the_tasks_their_reports_count() {
    let mut s = five_check_session();
    let schedule = app::checks::schedule::ScheduleChecks::of(&s);
    let missed = app::checks::baseline::BaselineChecks::of(&s).missed_tasks;
    for (kind, expected, count) in [
        (CheckKind::HighFloat, vec![0], schedule.high_float),
        (CheckKind::HighDuration, vec![1], schedule.high_duration),
        (CheckKind::InvalidDates, vec![2], schedule.invalid_dates),
        (
            CheckKind::MissingResources,
            vec![2],
            schedule.missing_resources.expect("resources"),
        ),
        (CheckKind::Missed, vec![0], missed),
        (
            CheckKind::HardConstraints,
            vec![0, 1],
            schedule.hard_constraints,
        ),
        (CheckKind::NegativeFloat, vec![1], schedule.negative_float),
    ] {
        assert!(s.set_check_filter(Some(kind)));
        let leaves = listed(&s);
        assert_eq!(leaves.len(), count, "{kind:?} count");
        assert_eq!(leaves, expected, "{kind:?} slots");
        let key = kind.name().replace('-', "_");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&s.health()).expect("JSON")["counts"][key]["count"],
            count
        );
    }
}

#[test]
fn each_check_keeps_its_leaves_and_their_summaries() {
    let mut s = session();
    let counts = Checks::of(&s);
    for (kind, rows, count) in [
        (CheckKind::OpenStart, vec![0, 1, 3], counts.open_starts),
        (CheckKind::OpenEnd, vec![0, 3, 4], counts.open_ends),
        (
            CheckKind::MissingLogic,
            vec![0, 1, 3, 4],
            counts.missing_logic,
        ),
    ] {
        assert!(s.set_check_filter(Some(kind)));
        assert_eq!(s.check_filter(), Some(kind));
        assert_eq!(s.row_filter(), RowFilter::Check(kind));
        assert_eq!(
            listed(&s),
            rows,
            "{kind:?} keeps its leaves and the summary"
        );
        let leaves = listed(&s).iter().filter(|&&row| row != 0).count();
        assert_eq!(leaves, count, "{kind:?} keeps what the checks count");
    }
    assert!(s.set_check_filter(None));
    assert_eq!(listed(&s), vec![0, 1, 2, 3, 4, 5], "clearing keeps all");
}

#[test]
fn fixing_an_open_end_drops_its_row() {
    let mut s = session();
    assert!(s.set_check_filter(Some(CheckKind::OpenEnd)));
    s.edit(Edit::AddLink {
        pred: TaskId(3),
        succ: TaskId(4),
        kind: LinkType::FS,
        lag: 0,
    })
    .expect("applies");
    assert_eq!(
        listed(&s),
        vec![4],
        "3 and its summary are gone, 4 is still open"
    );
    assert!(s.undo().expect("undoes"));
    assert_eq!(
        listed(&s),
        vec![0, 3, 4],
        "undoing the link brings them back"
    );
}

#[test]
fn a_check_replaces_another_filter_and_clearing_it_leaves_a_different_one() {
    let mut s = session();
    assert!(s.set_critical_only(true));
    assert!(s.set_check_filter(Some(CheckKind::OpenEnd)));
    assert!(!s.critical_only());
    assert_eq!(
        s.row_filter().print_label(&app::Texts::default()),
        "check open-end"
    );
    assert!(s.set_status_filter(Some(StatusKind::Complete)));
    assert_eq!(s.check_filter(), None, "a Status filter replaces the Check");
    assert!(s.set_check_filter(None));
    assert_eq!(s.status_filter().map(StatusKind::name), Some("complete"));
}
