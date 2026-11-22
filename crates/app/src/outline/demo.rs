//! The deterministic demo layout.

use crate::meta::TaskMeta;
use engine::{Schedule, Solution, TaskId};

/// Everything [`crate::demo`] returns, plus the outline order.
pub struct DemoLayout {
    /// The generated leaf schedule with appended summary milestones.
    pub schedule: Schedule,
    /// The computed dates for every task.
    pub solution: Solution,
    /// Row order: `timeline::order_by_start` over every task.
    pub order: Vec<u32>,
    /// Display data indexed by task id, like [`Schedule::tasks`].
    pub meta: Vec<TaskMeta>,
    /// Task ids in outline (position) order: `wbs[p]` is the task at outline position `p`.
    pub wbs: Vec<TaskId>,
}

/// Builds the deterministic demo schedule, display metadata, and outline order.
///
/// # Panics
/// Panics if the engine refuses the generated schedule. Unreachable: the generator makes valid,
/// acyclic schedules over calendar 0.
#[must_use]
pub fn demo_layout(n: usize) -> DemoLayout {
    let names = crate::names::generate(crate::SEED, n);
    let mut is_summary = vec![false; n];
    for task in &names {
        if let Some(parent) = task.parent {
            is_summary[parent.0 as usize] = true;
        }
    }
    let leaf_count = is_summary.iter().filter(|&&summary| !summary).count();

    let mut schedule = engine::generate_demo(crate::SEED, leaf_count);
    for _ in leaf_count..n {
        schedule
            .add_task(engine::Task {
                duration: 0,
                calendar: engine::CalendarId(0),
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("the generated schedule has calendar 0");
    }
    let solution = schedule
        .compute()
        .expect("generated schedules are acyclic and valid");

    let leaf_rank = timeline::order_by_start(&solution.dates[..leaf_count]);
    let mut leaf = 0;
    let mut summary = 0;
    let wbs: Vec<TaskId> = is_summary
        .iter()
        .map(|&is_summary| {
            if is_summary {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
                )]
                let task = TaskId((leaf_count + summary) as u32);
                summary += 1;
                task
            } else {
                let task = TaskId(leaf_rank[leaf]);
                leaf += 1;
                task
            }
        })
        .collect();

    let mut meta = vec![None; n];
    for (position, task) in names.into_iter().enumerate() {
        let task_id = wbs[position];
        meta[task_id.0 as usize] = Some(TaskMeta {
            name: task.name,
            outline_level: task.outline_level,
            code: None,
            activity_codes: Vec::new(),
            wbs_code: None,
            uid: None,
            floor: None,
            color: None,
            label: None,
            key: None,
            physical: None,
            parent: task.parent.map(|parent| wbs[parent.0 as usize]),
        });
    }
    let meta = meta
        .into_iter()
        .map(|task| task.expect("wbs contains every task id"))
        .collect();
    let order = timeline::order_by_start(&solution.dates);

    DemoLayout {
        schedule,
        solution,
        order,
        meta,
        wbs,
    }
}

/// The stress fixture of `n` tasks under one root summary, each task coded `Phase` with the value `i % groups` (`groups` at
/// least 1), so the rows can be grouped by `Phase`. Returns the layout and the head task's id.
///
/// # Panics
/// Panics if the engine refuses the generated schedule. Unreachable, as for [`demo_layout`].
#[must_use]
pub fn stress_grouped_layout(n: usize, groups: usize) -> (DemoLayout, TaskId) {
    let (mut schedule, head) = engine::stress(crate::SEED, n.max(2));
    let leaves = schedule.tasks().len();
    let root = schedule
        .add_task(engine::Task {
            duration: 0,
            calendar: engine::CalendarId(0),
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("the stress fixture has calendar 0");
    let solution = schedule
        .compute()
        .expect("generated schedules are acyclic and valid");
    let mut meta = crate::names::generate(crate::SEED, leaves);
    crate::names::paint(&mut meta, crate::SEED);
    for (i, task) in meta.iter_mut().enumerate() {
        task.parent = Some(root);
        task.outline_level = 2;
        task.activity_codes = vec![import::ActivityCode {
            code_type: "Phase".to_owned(),
            value: (i % groups.max(1)).to_string(),
        }];
    }
    meta.push(TaskMeta {
        name: "Project".to_owned(),
        parent: None,
        outline_level: 1,
        activity_codes: Vec::new(),
        color: None,
        ..meta[0].clone()
    });
    let wbs = std::iter::once(root)
        .chain((0..leaves).map(|i| TaskId(u32::try_from(i).expect("a task slot index"))))
        .collect();
    let order = timeline::order_by_start(&solution.dates);
    let layout = DemoLayout {
        schedule,
        solution,
        order,
        meta,
        wbs,
    };
    (layout, head)
}
