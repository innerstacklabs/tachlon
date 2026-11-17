//! Shared setup for the collapse tests (`collapse*.rs`): the demo session,
//! the link views and the small hand-built schedules. Each test binary uses
//! part of it, so unused items are expected.
#![allow(dead_code, reason = "each test binary uses part of the shared setup")]

use super::outline_session;
use app::{Session, meta::TaskMeta};
use engine::{Calendar, Schedule, Task, TaskId};
use std::collections::BTreeSet;
use timeline::Viewport;

pub const N: usize = 2_000;

pub fn session(n: usize) -> Session {
    outline_session(n).0
}

pub fn link_view(session: &Session) -> Viewport {
    let rows = session.timeline().order().len();
    let mut view = Viewport::new(
        10_000.0,
        40.0 + rows as f64 * Viewport::new(1.0, 1.0, 1).row_height + 1.0,
        rows,
    );
    let span = (session.solution().project_finish as f64).max(120.0) * 1.04;
    view.px_per_minute = view.width / span;
    view.t0 = -0.02 * span;
    view
}

pub fn link_pairs(session: &Session) -> BTreeSet<(u32, u32)> {
    session
        .links()
        .visible(&link_view(session), &session.solution().dates)
        .iter()
        .map(|placed| (placed.link.pred, placed.link.succ))
        .collect()
}

pub fn summaries(session: &Session) -> Vec<TaskId> {
    let outline = session.outline().expect("test session has an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| outline.is_summary(task))
        .collect()
}

pub fn standard_calendar() -> Calendar {
    let day = vec![(480, 720), (780, 1020)];
    Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("valid standard calendar")
}

pub fn schedule_with_tasks(task_count: usize) -> Schedule {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for _ in 0..task_count {
        schedule
            .add_task(Task {
                duration: 0,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    schedule
}

pub fn task_meta(parent: Option<TaskId>, level: u8) -> TaskMeta {
    TaskMeta {
        name: "task".to_string(),
        parent,
        outline_level: level,
        code: None,
        ..TaskMeta::default()
    }
}
