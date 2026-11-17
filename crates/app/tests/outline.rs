use app::{
    SEED,
    meta::TaskMeta,
    outline::{Outline, OutlineError, demo_layout},
};
use engine::{Calendar, DepId, Dependency, LinkType, Schedule, Task, TaskId};

#[test]
fn a_valid_forest_is_accepted_in_wbs_order() {
    let schedule = schedule_with_tasks(5);
    let meta = vec![
        task_meta(Some(TaskId(3)), 2),
        task_meta(None, 1),
        task_meta(Some(TaskId(1)), 2),
        task_meta(None, 1),
        task_meta(Some(TaskId(3)), 2),
    ];
    let wbs = [TaskId(3), TaskId(0), TaskId(4), TaskId(1), TaskId(2)];

    let outline = Outline::new(&meta, &wbs, &schedule).expect("valid forest");

    assert_eq!(outline.len(), 5);
    assert_eq!(outline.roots(), [TaskId(3), TaskId(1)]);
    assert_eq!(outline.children(TaskId(0)), []);
    assert_eq!(outline.children(TaskId(1)), [TaskId(2)]);
    assert_eq!(outline.children(TaskId(2)), []);
    assert_eq!(outline.children(TaskId(3)), [TaskId(0), TaskId(4)]);
    assert_eq!(outline.children(TaskId(4)), []);
    assert_eq!(outline.preorder(), wbs);
    assert_eq!(outline.parent(TaskId(0)), Some(TaskId(3)));
    assert_eq!(outline.parent(TaskId(1)), None);
    assert_eq!(outline.parent(TaskId(2)), Some(TaskId(1)));
    assert_eq!(outline.parent(TaskId(3)), None);
    assert_eq!(outline.parent(TaskId(4)), Some(TaskId(3)));
    assert!(!outline.is_summary(TaskId(0)));
    assert!(outline.is_summary(TaskId(1)));
    assert!(!outline.is_summary(TaskId(2)));
    assert!(outline.is_summary(TaskId(3)));
    assert!(!outline.is_summary(TaskId(4)));
    assert_eq!(
        outline.subtree(TaskId(3)),
        [TaskId(3), TaskId(0), TaskId(4)]
    );
    assert_eq!(outline.subtree(TaskId(4)), [TaskId(4)]);
}

#[test]
fn a_cycle_is_refused() {
    let schedule = schedule_with_tasks(4);
    let meta = vec![
        task_meta(Some(TaskId(1)), 2),
        task_meta(Some(TaskId(2)), 2),
        task_meta(Some(TaskId(0)), 2),
        task_meta(None, 1),
    ];
    let wbs = [TaskId(0), TaskId(1), TaskId(2), TaskId(3)];
    assert_eq!(
        Outline::new(&meta, &wbs, &schedule).err(),
        Some(OutlineError::Cycle)
    );

    let schedule = schedule_with_tasks(2);
    let meta = vec![task_meta(Some(TaskId(0)), 2), task_meta(None, 1)];
    let wbs = [TaskId(0), TaskId(1)];
    assert_eq!(
        Outline::new(&meta, &wbs, &schedule).err(),
        Some(OutlineError::Cycle)
    );
}

#[test]
fn a_wrong_length_is_refused() {
    let schedule = schedule_with_tasks(3);
    let meta = vec![task_meta(None, 1), task_meta(None, 1), task_meta(None, 1)];
    let wbs = [TaskId(0), TaskId(1), TaskId(2)];

    assert_eq!(
        Outline::new(&meta[..2], &wbs, &schedule).err(),
        Some(OutlineError::WrongLength)
    );
    assert_eq!(
        Outline::new(&meta, &wbs[..2], &schedule).err(),
        Some(OutlineError::WrongLength)
    );
    assert_eq!(
        Outline::new(&meta, &[TaskId(0), TaskId(1), TaskId(1)], &schedule,).err(),
        Some(OutlineError::WrongLength)
    );

    let mut bad_parent = meta;
    bad_parent[0].parent = Some(TaskId(3));
    assert_eq!(
        Outline::new(&bad_parent, &wbs, &schedule).err(),
        Some(OutlineError::WrongLength)
    );
}

#[test]
fn a_dependency_on_a_summary_is_refused() {
    let meta = vec![
        task_meta(None, 1),
        task_meta(Some(TaskId(0)), 2),
        task_meta(None, 1),
    ];
    let wbs = [TaskId(0), TaskId(1), TaskId(2)];
    let mut schedule = schedule_with_tasks(3);
    add_link(&mut schedule, TaskId(0), TaskId(2));
    assert_eq!(
        Outline::new(&meta, &wbs, &schedule).err(),
        Some(OutlineError::SummaryHasDependency(DepId(0)))
    );

    let meta = vec![
        task_meta(None, 1),
        task_meta(Some(TaskId(0)), 2),
        task_meta(None, 1),
        task_meta(None, 1),
    ];
    let wbs = [TaskId(0), TaskId(1), TaskId(2), TaskId(3)];
    let mut schedule = schedule_with_tasks(4);
    add_link(&mut schedule, TaskId(1), TaskId(2));
    add_link(&mut schedule, TaskId(3), TaskId(0));
    assert_eq!(
        Outline::new(&meta, &wbs, &schedule).err(),
        Some(OutlineError::SummaryHasDependency(DepId(1)))
    );

    let mut schedule = schedule_with_tasks(3);
    add_link(&mut schedule, TaskId(1), TaskId(2));
    Outline::new(&meta[..3], &wbs[..3], &schedule).expect("leaf dependency is valid");
}

#[test]
fn the_demo_has_the_expected_shape_at_50000_tasks() {
    let layout = demo_layout(50_000);
    assert_eq!(layout.schedule.tasks().len(), 50_000);
    let outline =
        Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("valid demo outline");

    let summaries = (0..outline.len())
        .filter(|&task| outline.is_summary(TaskId(task as u32)))
        .count();
    assert_eq!(summaries, 6_636);
    assert_eq!(outline.len() - summaries, 43_364);
    assert_eq!(outline.preorder(), layout.wbs.as_slice());
}

#[test]
fn no_summary_has_a_dependency() {
    let layout = demo_layout(50_000);
    let mut is_summary = vec![false; layout.meta.len()];
    for task in &layout.meta {
        if let Some(parent) = task.parent {
            is_summary[parent.0 as usize] = true;
        }
    }

    for dependency in layout.schedule.dependencies() {
        assert!(!is_summary[dependency.pred.0 as usize]);
        assert!(!is_summary[dependency.succ.0 as usize]);
    }
}

#[test]
fn summaries_do_not_move_leaves() {
    let layout = demo_layout(5_000);
    let outline =
        Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("valid demo outline");
    let leaf_count = (0..outline.len())
        .filter(|&task| !outline.is_summary(TaskId(task as u32)))
        .count();
    let mut reference_schedule = engine::generate_demo(SEED, leaf_count);
    let reference = reference_schedule
        .compute()
        .expect("valid generated schedule");

    assert_eq!(layout.solution.dates[..leaf_count], reference.dates[..]);
    assert_eq!(layout.solution.project_finish, reference.project_finish);
}

#[test]
fn level_one_summaries_are_narrow() {
    let layout = demo_layout(50_000);
    let outline =
        Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("valid demo outline");
    let mut widths = Vec::new();

    for task in 0..outline.len() {
        let task = TaskId(task as u32);
        if outline.is_summary(task) && layout.meta[task.0 as usize].outline_level == 1 {
            let (min_es, max_ef) = date_bounds(
                outline
                    .subtree(task)
                    .iter()
                    .copied()
                    .filter(|&descendant| !outline.is_summary(descendant)),
                &layout.solution.dates,
            );
            widths.push(max_ef - min_es);
        }
    }

    let (project_start, project_finish) = date_bounds(
        (0..outline.len())
            .map(|task| TaskId(task as u32))
            .filter(|&task| !outline.is_summary(task)),
        &layout.solution.dates,
    );
    let project_span = project_finish - project_start;
    widths.sort_unstable();
    let median = widths[widths.len() / 2];
    let ratio = median as f64 / project_span as f64;
    eprintln!(
        "level-1 summaries: {}, median width: {median}, project span: {project_span}, ratio: {ratio}",
        widths.len()
    );
    assert!(ratio < 0.05, "median/span ratio was {ratio}");
}

#[test]
fn small_demo_sizes_work() {
    for n in 0..=40 {
        let layout = demo_layout(n);
        assert_eq!(layout.schedule.tasks().len(), n);
        Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
            .unwrap_or_else(|error| panic!("demo_layout({n}) had invalid outline: {error}"));
    }
}

fn standard_calendar() -> Calendar {
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

fn schedule_with_tasks(task_count: usize) -> Schedule {
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
            .expect("valid task");
    }
    schedule
}

fn task_meta(parent: Option<TaskId>, outline_level: u8) -> TaskMeta {
    TaskMeta {
        name: "task".to_string(),
        parent,
        outline_level,
        code: None,
        ..TaskMeta::default()
    }
}

fn add_link(schedule: &mut Schedule, pred: TaskId, succ: TaskId) {
    schedule
        .add_dependency(Dependency {
            pred,
            succ,
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("valid dependency");
}

fn date_bounds(tasks: impl Iterator<Item = TaskId>, dates: &[engine::TaskDates]) -> (i64, i64) {
    let mut bounds: Option<(i64, i64)> = None;
    for task in tasks {
        let dates = dates[task.0 as usize];
        bounds = Some(match bounds {
            None => (dates.es, dates.ef),
            Some((min_es, max_ef)) => (min_es.min(dates.es), max_ef.max(dates.ef)),
        });
    }
    bounds.expect("a summary and the project both contain leaves")
}
