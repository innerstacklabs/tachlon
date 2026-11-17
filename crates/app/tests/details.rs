use app::baseline::{Baseline, BaselineTask};
use app::{Session, dates::Origin, edit::Edit, meta::TaskMeta};
use engine::{Calendar, DAY, Dependency, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

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

fn task(schedule: &mut Schedule, calendar: engine::CalendarId, duration: i64) -> TaskId {
    schedule
        .add_task(Task {
            duration,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("valid task")
}

fn link(schedule: &mut Schedule, pred: TaskId, succ: TaskId, kind: LinkType) {
    schedule
        .add_dependency(Dependency {
            pred,
            succ,
            kind,
            lag: 0,
        })
        .expect("valid dependency");
}

fn meta(name: &str, parent: Option<TaskId>, outline_level: u8) -> TaskMeta {
    TaskMeta {
        name: name.to_string(),
        parent,
        outline_level,
        code: None,
        ..TaskMeta::default()
    }
}

/// Two summary tasks after `first`: `Block` (top level) and `Frame` (under it). Each of the
/// `leaves` is named `leaf` and sits under `Frame`, so its outline path is `Block / Frame / leaf`.
fn block_and_frame(first: u32, leaves: &[&str]) -> Vec<TaskMeta> {
    let (block, frame) = (TaskId(first), TaskId(first + 1));
    let mut metadata: Vec<TaskMeta> = leaves
        .iter()
        .map(|leaf| meta(leaf, Some(frame), 3))
        .collect();
    metadata.push(meta("Block", None, 1));
    metadata.push(meta("Frame", Some(block), 2));
    metadata
}

#[test]
fn details_show_dates_float_criticality_and_distinct_neighbours() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let a = task(&mut schedule, calendar, 8 * 60);
    let b = task(&mut schedule, calendar, 8 * 60);
    let c = task(&mut schedule, calendar, 16 * 60);
    let d = task(&mut schedule, calendar, 8 * 60);

    link(&mut schedule, a, b, LinkType::FS);
    link(&mut schedule, b, d, LinkType::FS);
    link(&mut schedule, a, c, LinkType::FS);
    link(&mut schedule, c, d, LinkType::FS);
    // The two summaries that give A to D their outline path; nothing links to them.
    task(&mut schedule, calendar, 0);
    task(&mut schedule, calendar, 0);

    let solution = schedule.compute().expect("acyclic schedule");
    let order = vec![a.0, b.0, c.0, d.0, 4, 5];
    let metadata = block_and_frame(4, &["A", "B", "C", "D"]);
    let session = Session::new(
        schedule,
        solution,
        Timeline::new(order, 800.0, 400.0, 1.0),
        metadata,
        || 0.0,
    );

    // Instant zero is Monday 2026-01-05 00:00 (the axis origin). A occupies
    // Monday's two periods (8 h); B follows it on Tuesday 08:00-17:00 (8 h).
    // C (16 h) fills Tuesday and Wednesday, so D starts Thursday 08:00 and B
    // can slip a working day, to Wednesday 08:00-17:00: 8 h = 480 minutes.
    assert_eq!(Origin::DEMO.stamp(0), "2026-01-05 00:00");
    let details = session.task_details(b.0).expect("B exists");
    assert_eq!(details.name, "B");
    assert_eq!(details.outline_path, "Block / Frame / B");
    assert_eq!(details.start, "2026-01-06 08:00");
    assert_eq!(details.finish, "2026-01-06 17:00");
    assert_eq!(details.late_start, "2026-01-07 08:00");
    assert_eq!(details.late_finish, "2026-01-07 17:00");
    assert_eq!(details.float_minutes, 480);
    assert_eq!(details.float_days, 1);
    assert_eq!(details.float_hours, 0);
    assert_eq!(details.float_rem_minutes, 0);
    assert_eq!(details.float_text, "1 d 0 h");
    assert!(!details.critical);
    assert_eq!(details.predecessor_count, 1);
    assert_eq!(details.predecessors, ["Block / Frame / A (FS)"]);
    assert_eq!(details.successor_count, 1);
    assert_eq!(details.successors, ["Block / Frame / D (FS)"]);

    let details = session.task_details(d.0).expect("D exists");
    assert_eq!(details.predecessor_count, 2);
    assert_eq!(
        details.predecessors,
        ["Block / Frame / B (FS)", "Block / Frame / C (FS)"]
    );
    assert_eq!(details.successor_count, 0);
    assert_eq!(details.successors, [] as [String; 0]);
    assert!(details.critical);

    let details = session.task_details(a.0).expect("A exists");
    assert_eq!(details.successor_count, 2);
}

#[test]
fn neighbour_lists_are_distinct_ordered_and_limited_to_three() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let source = task(&mut schedule, calendar, 60);
    let successors: Vec<_> = (0..5).map(|_| task(&mut schedule, calendar, 60)).collect();
    for &successor in &successors {
        link(&mut schedule, source, successor, LinkType::FS);
    }
    link(&mut schedule, source, successors[0], LinkType::SS);
    task(&mut schedule, calendar, 0);
    task(&mut schedule, calendar, 0);

    let solution = schedule.compute().expect("acyclic schedule");
    let order = (0..8).collect();
    let metadata = block_and_frame(6, &["Source", "One", "Two", "Three", "Four", "Five"]);
    let session = Session::new(
        schedule,
        solution,
        Timeline::new(order, 800.0, 400.0, 1.0),
        metadata,
        || 0.0,
    );

    let details = session.task_details(source.0).expect("source exists");
    assert_eq!(details.successor_count, 5);
    assert_eq!(
        details.successors,
        [
            "Block / Frame / One (FS)",
            "Block / Frame / Two (FS)",
            "Block / Frame / Three (FS)"
        ]
    );
    assert!(session.task_details(8).is_none());
}

#[test]
fn a_name_that_holds_the_separator_is_the_card_name_and_the_path_extends_it() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let leaf = task(&mut schedule, calendar, 60);
    let predecessor = task(&mut schedule, calendar, 60);
    link(&mut schedule, predecessor, leaf, LinkType::FS);
    task(&mut schedule, calendar, 0);
    task(&mut schedule, calendar, 0);

    let solution = schedule.compute().expect("acyclic schedule");
    let metadata = block_and_frame(2, &["Pour / cure", "Strip / clean"]);
    let session = Session::new(
        schedule,
        solution,
        Timeline::new((0..4).collect(), 800.0, 400.0, 1.0),
        metadata,
        || 0.0,
    );

    let details = session.task_details(leaf.0).expect("the task exists");
    assert_eq!(details.name, "Pour / cure");
    assert_eq!(details.outline_path, "Block / Frame / Pour / cure");
    assert_eq!(details.predecessors, ["Block / Frame / Strip / clean (FS)"]);
    assert_eq!(session.task_label(leaf.0), "Pour / cure");
}

#[test]
fn related_lines_carry_the_link_type_and_lag() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let pred = task(&mut schedule, calendar, 60);
    let succ = task(&mut schedule, calendar, 60);
    schedule
        .add_dependency(Dependency {
            pred,
            succ,
            kind: LinkType::SS,
            lag: 2 * 8 * 60,
        })
        .expect("valid dependency");
    task(&mut schedule, calendar, 0);
    task(&mut schedule, calendar, 0);

    let solution = schedule.compute().expect("acyclic schedule");
    let metadata = block_and_frame(2, &["Pour slab", "Strip"]);
    let session = Session::new(
        schedule,
        solution,
        Timeline::new((0..4).collect(), 800.0, 400.0, 1.0),
        metadata,
        || 0.0,
    );

    let details = session.task_details(pred.0).expect("pred exists");
    assert_eq!(details.successors, ["Block / Frame / Strip (SS+2d)"]);
    let details = session.task_details(succ.0).expect("succ exists");
    assert_eq!(details.predecessors, ["Block / Frame / Pour slab (SS+2d)"]);
}

/// A one-task session whose task has the given actuals and activity ID, plus its two summaries.
fn progress_session(
    code: Option<&str>,
    actual_start: Option<i64>,
    actual_finish: Option<i64>,
    remaining: Option<i64>,
) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    schedule
        .add_task(Task {
            duration: 8 * 60,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start,
            actual_finish,
            remaining,
        })
        .expect("valid task");
    task(&mut schedule, calendar, 0);
    task(&mut schedule, calendar, 0);
    let solution = schedule.compute().expect("acyclic schedule");
    let mut metadata = block_and_frame(1, &["Pour slab"]);
    metadata[0].code = code.map(str::to_owned);
    Session::new(
        schedule,
        solution,
        Timeline::new((0..3).collect(), 800.0, 400.0, 1.0),
        metadata,
        || 0.0,
    )
}

#[test]
fn details_carry_the_activity_id_status_and_actuals() {
    let started = progress_session(Some("A1-204"), Some(0), None, Some(4 * 60));
    let details = started.task_details(0).expect("the task exists");
    assert_eq!(details.activity_id, "A1-204");
    assert_eq!(details.status, "In progress");
    assert_eq!(details.actual_start, "2026-01-05 00:00");
    assert_eq!(details.actual_finish, "");
    assert_eq!(details.percent_complete, "50%");

    let done = progress_session(Some("A1-204"), Some(0), Some(480), None);
    let details = done.task_details(0).expect("the task exists");
    assert_eq!(details.status, "Complete");
    assert_eq!(details.actual_finish, "2026-01-05 08:00");
    assert_eq!(details.percent_complete, "100%");

    let waiting = progress_session(None, None, None, Some(4 * 60));
    let details = waiting.task_details(0).expect("the task exists");
    assert_eq!(details.activity_id, "");
    assert_eq!(details.status, "Not started");
    assert_eq!(details.actual_start, "");
    assert_eq!(details.actual_finish, "");
    assert_eq!(details.percent_complete, "");
}

/// A (8 h) then B (8 h) then a lone C, each on the standard calendar, with the baseline set when
/// `baselined` and A then lengthened to 16 h, which moves B a working day later.
fn baseline_session(baselined: bool) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let a = task(&mut schedule, calendar, 8 * 60);
    let b = task(&mut schedule, calendar, 8 * 60);
    task(&mut schedule, calendar, 8 * 60);
    link(&mut schedule, a, b, LinkType::FS);
    let solution = schedule.compute().expect("acyclic schedule");
    let mut session = Session::new(
        schedule,
        solution,
        Timeline::new(vec![0, 1, 2], 800.0, 400.0, 1.0),
        vec![meta("A", None, 1), meta("B", None, 1), meta("C", None, 1)],
        || 0.0,
    );
    if baselined {
        session
            .edit(Edit::SetBaseline { baseline: None })
            .expect("the baseline sets");
    }
    session
        .edit(Edit::SetDuration {
            task: a,
            minutes: 16 * 60,
        })
        .expect("the duration applies");
    session
}

#[test]
fn a_task_with_a_baseline_entry_reports_its_baseline_dates_and_variance() {
    let session = baseline_session(true);

    let moved = session.task_details(1).expect("B exists");
    assert_eq!(moved.start, "2026-01-07 08:00");
    assert_eq!(moved.baseline_start.as_deref(), Some("2026-01-06 08:00"));
    assert_eq!(moved.baseline_finish.as_deref(), Some("2026-01-06 17:00"));
    assert_eq!(
        moved.baseline_variance,
        Some([Some("+1d".to_string()), Some("+1d".to_string())])
    );

    let still = session.task_details(2).expect("C exists");
    assert_eq!(still.baseline_start.as_deref(), Some("2026-01-05 08:00"));
    assert_eq!(
        still.baseline_variance,
        Some([Some("0".to_string()), Some("0".to_string())])
    );
}

#[test]
fn a_task_the_baseline_lacks_shows_nothing_extra() {
    let entry = BaselineTask {
        es: 0,
        ef: 480,
        float: None,
    };
    let session = baseline_session(false)
        .with_baseline(Baseline::from_parts(vec![Some(entry), None, None], 480));
    let details = session.task_details(1).expect("B exists");
    assert_eq!(details.baseline_start, None);
    assert_eq!(details.baseline_finish, None);
    assert_eq!(details.baseline_variance, None);
}

#[test]
fn no_baseline_leaves_the_details_unchanged() {
    let session = baseline_session(false);
    let details = session.task_details(1).expect("B exists");
    assert_eq!(details.baseline_start, None);
    assert_eq!(details.baseline_finish, None);
    assert_eq!(details.baseline_variance, None);
}

/// A one-task session on `calendar`, plus its two summaries.
fn calendar_session(calendar: Calendar) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar);
    task(&mut schedule, calendar, 8 * 60);
    task(&mut schedule, calendar, 0);
    task(&mut schedule, calendar, 0);
    let solution = schedule.compute().expect("acyclic schedule");
    Session::new(
        schedule,
        solution,
        Timeline::new((0..3).collect(), 800.0, 400.0, 1.0),
        block_and_frame(1, &["Pour slab"]),
        || 0.0,
    )
}

#[test]
fn the_calendar_text_names_the_days_a_week_and_hours_a_day() {
    let office = calendar_session(standard_calendar());
    assert_eq!(office.task_calendar_text(0), "5 days a week, 8 h a day");

    let round_the_clock = Calendar::new(std::array::from_fn(|_| vec![(0, 1440)]), &[])
        .expect("valid 24-hour calendar");
    let round_the_clock = calendar_session(round_the_clock);
    assert_eq!(
        round_the_clock.task_calendar_text(0),
        "7 days a week, 24 h a day"
    );

    let half_days = Calendar::new(
        std::array::from_fn(|day| if day == 0 { vec![(480, 930)] } else { vec![] }),
        &[],
    )
    .expect("valid one-day calendar");
    let half_days = calendar_session(half_days);
    assert_eq!(half_days.task_calendar_text(0), "1 day a week, 7.5 h a day");
    assert_eq!(
        half_days.task_calendar_text(99),
        "",
        "an absent task reads empty"
    );
}

#[test]
fn a_start_constraint_reads_as_a_date_and_none_reads_empty() {
    let mut session = calendar_session(standard_calendar());
    assert_eq!(
        session.task_constraint_text(0),
        "",
        "no constraint reads empty"
    );
    session
        .set_start_text(TaskId(0), "2026-02-16 08:00")
        .expect("a typed Start makes a constraint");
    assert_eq!(session.task_constraint_text(0), "2026-02-16 08:00");
    assert_eq!(session.task_constraint_text(1), "", "another task has none");
    assert_eq!(
        session.task_constraint_text(99),
        "",
        "an absent task reads empty"
    );
    session
        .set_start_text(TaskId(0), "")
        .expect("empty text clears it");
    assert_eq!(session.task_constraint_text(0), "");
}

#[test]
fn driven_by_names_the_predecessors_that_set_the_early_start() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let a = task(&mut schedule, calendar, 8 * 60);
    let b = task(&mut schedule, calendar, 8 * 60);
    let c = task(&mut schedule, calendar, 16 * 60);
    let d = task(&mut schedule, calendar, 8 * 60);
    link(&mut schedule, a, b, LinkType::FS);
    link(&mut schedule, b, d, LinkType::FS);
    link(&mut schedule, a, c, LinkType::FS);
    link(&mut schedule, c, d, LinkType::FS);
    // Four equal tasks all finish together, so each drives the one that follows them.
    let fan: Vec<_> = (0..4)
        .map(|_| task(&mut schedule, calendar, 8 * 60))
        .collect();
    let last = task(&mut schedule, calendar, 8 * 60);
    for &pred in &fan {
        link(&mut schedule, pred, last, LinkType::FS);
    }
    task(&mut schedule, calendar, 0);
    task(&mut schedule, calendar, 0);

    let solution = schedule.compute().expect("acyclic schedule");
    let metadata = block_and_frame(9, &["A", "B", "C", "D", "P1", "P2", "P3", "P4", "Last"]);
    let session = Session::new(
        schedule,
        solution,
        Timeline::new((0..11).collect(), 800.0, 400.0, 1.0),
        metadata,
        || 0.0,
    );

    // C (16 h) outlasts B (8 h), so only C sets D's start.
    assert_eq!(session.task_driven_by(d.0), "Block / Frame / C");
    assert_eq!(session.task_driven_by(b.0), "Block / Frame / A");
    assert_eq!(
        session.task_driven_by(a.0),
        "",
        "a task with no predecessor has no driver"
    );
    assert_eq!(
        session.task_driven_by(last.0),
        "Block / Frame / P1\nBlock / Frame / P2\nBlock / Frame / P3\n+ 1 more"
    );
    assert_eq!(session.task_driven_by(99), "", "an absent task reads empty");
}

#[test]
fn a_missed_deadline_reads_with_the_days_it_is_missed_by() {
    let ef = |task: usize| calendar_session(standard_calendar()).solution().dates[task].ef;
    let with = |deadlines| calendar_session(standard_calendar()).with_deadlines(deadlines);
    let (ef0, ef2) = (ef(0), ef(2));
    let session = with(vec![Some(ef0 - 2 * DAY), None, Some(ef2)]);
    let late = session.date_text(ef0 - 2 * DAY);
    assert_eq!(
        session.task_deadline_text(0),
        format!("{late} (missed by 2d)")
    );
    assert_eq!(session.task_deadline_text(1), "", "none, no text");
    assert_eq!(
        session.task_deadline_text(2),
        session.date_text(ef2),
        "in time"
    );
    assert_eq!(session.task_deadline_text(99), "", "an absent task");
    let hours = with(vec![Some(ef0 - 60)]);
    assert_eq!(
        hours.task_deadline_text(0),
        format!("{} (missed by <1d)", hours.date_text(ef0 - 60))
    );
}

#[test]
fn the_activity_codes_of_a_task_read_as_type_value_pairs() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    task(&mut schedule, calendar, 8 * 60);
    task(&mut schedule, calendar, 8 * 60);
    let solution = schedule.compute().expect("acyclic schedule");
    let code = |code_type: &str, value: &str| import::ActivityCode {
        code_type: code_type.to_owned(),
        value: value.to_owned(),
    };
    let mut metadata = vec![meta("Coded", None, 1), meta("Plain", None, 1)];
    metadata[0].activity_codes = vec![code("Phase", "Civil"), code("Area \"N\"", "North")];
    let session = Session::new(
        schedule,
        solution,
        Timeline::new((0..2).collect(), 800.0, 400.0, 1.0),
        metadata,
        || 0.0,
    );
    assert_eq!(
        session.task_activity_codes(0),
        r#"[["Phase","Civil"],["Area \"N\"","North"]]"#
    );
    assert_eq!(session.task_activity_codes(1), "[]", "no code, no row");
    assert_eq!(session.task_activity_codes(99), "[]", "an absent task");
}
