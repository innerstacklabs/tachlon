//! A summary's Dur cell shows its working duration, from its earliest start to its latest finish
//! (gs#820), not the 0 of the summary's own task.

use crate::common;

use app::{Session, meta::TaskMeta, outline::Outline};
use engine::{Calendar, Dependency, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

/// "Design" (a summary) over two linked leaves of 4 and 3 working days, on a Monday-to-Friday
/// calendar of 8-hour days that starts on a working day. The leaves run back to back, so the
/// summary spans 7 working days, which is 9 calendar days: the weekend between them does not count.
fn design() -> Session {
    let mut schedule = Schedule::new();
    let work = vec![(9 * 60, 17 * 60)];
    let week = std::array::from_fn(|day| if day < 5 { work.clone() } else { Vec::new() });
    let calendar = schedule.add_calendar(Calendar::new(week, &[]).expect("a calendar"));
    for days in [0, 4, 3] {
        let task = Task {
            duration: days * 8 * 60,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("a task");
    }
    schedule
        .add_dependency(Dependency {
            pred: TaskId(1),
            succ: TaskId(2),
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("a link");
    let solution = schedule.compute().expect("acyclic");
    let row = |name: &str, parent: Option<u32>, level: u8| TaskMeta {
        name: name.to_string(),
        parent: parent.map(TaskId),
        outline_level: level,
        code: None,
        ..TaskMeta::default()
    };
    let meta = vec![
        row("Design", None, 1),
        row("Draw", Some(0), 2),
        row("Check", Some(0), 2),
    ];
    let wbs: Vec<TaskId> = (0..3).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, common::clock)
        .with_outline(outline)
        .expect("the outline fits")
}

#[test]
fn a_summary_shows_its_working_span_not_zero() {
    let session = design();
    assert_eq!(session.task_duration(1), "4d");
    assert_eq!(session.task_duration(2), "3d");
    let (start, finish) = session.summary_span(0).expect("Design is a summary");
    assert!(finish - start > 7 * 24 * 60, "the span crosses a weekend");
    assert_eq!(session.task_duration(0), "7d");
}
