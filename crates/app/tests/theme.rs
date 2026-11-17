//! Session-owned timeline themes and their effect on native scenes.

use crate::common;

use app::{Session, meta::TaskMeta, outline::Outline};
use common::clock;
use engine::{Calendar, Dependency, LinkType, Schedule, Task};
use timeline::theme::TOKENS;
use timeline::{Theme, Timeline};

/// A summary containing a two-task critical chain and a shorter normal task.
fn setup() -> (Session, u32) {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7 calendar");
    let calendar = schedule.add_calendar(calendar);
    let mut add = |hours: i64| {
        schedule
            .add_task(Task {
                duration: hours * 60,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("valid task")
    };
    let summary = add(0);
    let first = add(3);
    let selected = add(4);
    let normal = add(2);
    schedule
        .add_dependency(Dependency {
            pred: first,
            succ: selected,
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("valid dependency");

    let solution = schedule.compute().expect("acyclic schedule");
    assert!(solution.dates[first.0 as usize].is_critical());
    assert!(solution.dates[selected.0 as usize].is_critical());
    assert!(!solution.dates[normal.0 as usize].is_critical());

    let meta = vec![
        TaskMeta {
            name: "Summary".into(),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        },
        TaskMeta {
            name: "Summary / First".into(),
            parent: Some(summary),
            outline_level: 2,
            code: None,
            ..TaskMeta::default()
        },
        TaskMeta {
            name: "Summary / Selected".into(),
            parent: Some(summary),
            outline_level: 2,
            code: None,
            ..TaskMeta::default()
        },
        TaskMeta {
            name: "Summary / Normal".into(),
            parent: Some(summary),
            outline_level: 2,
            code: None,
            ..TaskMeta::default()
        },
    ];
    let wbs = [summary, first, selected, normal];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("valid outline");
    let mut timeline = Timeline::new(wbs.iter().map(|task| task.0).collect(), 1_000.0, 400.0, 1.0);
    timeline.viewport.fit(0.0, solution.project_finish as f64);
    let session = Session::new(schedule, solution, timeline, Vec::new(), clock)
        .with_outline(outline)
        .expect("matching outline");
    (session, selected.0)
}

fn distinct_theme() -> Theme {
    let mut theme = Theme::default();
    for (index, key) in TOKENS.iter().enumerate() {
        let value = index as f32 + 1.0;
        assert!(theme.set(key, [value / 255.0, (value + 32.0) / 255.0, 0.5, 1.0]));
    }
    theme
}

fn has_color(scene: &timeline::Scene, color: [f32; 4]) -> bool {
    scene.rects.iter().any(|rect| rect.color == color)
}

#[test]
fn scenes_use_the_session_theme_for_bars_summaries_and_highlights() {
    let (mut session, selected) = setup();
    let defaults = Theme::default();
    let scene = session.scene();
    assert!(has_color(&scene, defaults.bar_soft), "default normal bar");
    assert!(
        has_color(&scene, defaults.crit_soft),
        "default critical bar"
    );

    let theme = distinct_theme();
    session.set_theme(theme);
    let scene = session.scene();
    assert!(has_color(&scene, theme.bar_soft), "custom normal bar");
    assert!(has_color(&scene, theme.crit_soft), "custom critical bar");
    assert!(has_color(&scene, theme.sum), "custom summary bar");

    session.select(selected);
    let highlighted = session.scene();
    assert!(
        has_color(&highlighted, theme.sel_edge),
        "custom highlighted path"
    );
}

#[test]
fn setting_a_theme_does_not_change_schedule_or_history_state() {
    let (mut session, selected) = setup();
    let row = session
        .timeline()
        .row_of_task(selected)
        .expect("selected task has a row");
    let dates = &session.solution().dates[selected as usize];
    let x = session
        .timeline()
        .viewport
        .x_of((dates.es + dates.ef) as f64 / 2.0);
    let y = session.timeline().viewport.row_top(row) + session.timeline().viewport.row_height / 2.0;
    session.pointer_down(x, y, false);
    session.pointer_move(x + 3000.0, y);
    session.pointer_up();
    assert_eq!(session.history_len(), (1, 0), "drag created undo state");

    let revision = session.revision();
    let stamp = session.stamp();
    let history = session.history_len();
    let can_undo = session.can_undo();
    let can_redo = session.can_redo();

    session.set_theme(distinct_theme());

    assert_eq!(session.revision(), revision);
    assert_eq!(session.stamp(), stamp);
    assert_eq!(session.history_len(), history);
    assert_eq!(session.can_undo(), can_undo);
    assert_eq!(session.can_redo(), can_redo);
}

#[test]
fn the_bar_look_is_set_and_a_bad_value_changes_nothing() {
    let (mut session, _) = setup();
    assert_eq!(session.set_bar_look(5.0, 0.5), Ok(()));
    let look = |s: &Session| {
        let v = &s.timeline().viewport;
        (v.bar_radius, v.bar_ratio)
    };
    assert_eq!(look(&session), (5.0, 0.5));
    for (radius, height) in [
        (f64::NAN, 0.5),
        (-1.0, 0.5),
        (3.0, 0.0),
        (3.0, 1.5),
        (3.0, f64::NAN),
    ] {
        assert!(session.set_bar_look(radius, height).is_err());
    }
    assert_eq!(
        look(&session),
        (5.0, 0.5),
        "a refused value changes nothing"
    );
}
