use super::*;
use engine::{CalendarId, Dependency, LinkType, Task, TaskId};

/// A task's actuals, as the engine holds them: none, a start only, or a start and a finish.
#[derive(Clone, Copy)]
pub(super) enum Actuals {
    Idle,
    Started(i64),
    Finished(i64, i64),
}

pub(super) fn task(actuals: Actuals) -> Task {
    let (actual_start, actual_finish) = match actuals {
        Actuals::Idle => (None, None),
        Actuals::Started(at) => (Some(at), None),
        Actuals::Finished(from, to) => (Some(from), Some(to)),
    };
    Task {
        duration: 8 * 60,
        calendar: CalendarId(0),
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start,
        actual_finish,
        remaining: None,
    }
}

/// Task `i` of `dates` runs `[i * DAY, i * DAY + 8h)`; a started one starts where its bar does.
fn started(i: i64) -> Actuals {
    Actuals::Started(i * DAY)
}

fn finished(i: i64) -> Actuals {
    Actuals::Finished(i * DAY, i * DAY + 8 * 60)
}

fn scene_with(actuals: &[Actuals], data_date: Option<i64>, dpr: f64) -> (Scene, Viewport) {
    let n = actuals.len();
    let d = dates(n);
    let tasks: Vec<Task> = actuals.iter().map(|&a| task(a)).collect();
    let order: Vec<u32> = (0..n as u32).collect();
    let mut v = Viewport::new(1200.0, 300.0, n);
    v.fit(0.0, 10.0 * DAY as f64);
    let backdrop = Backdrop {
        progress: Some(&tasks),
        data_date,
        ..Backdrop::default()
    };
    let s = build_marked(
        &v,
        dpr,
        &order,
        &d,
        &Summaries::default(),
        RowMarks::default(),
        None,
        backdrop,
        &THEME,
    );
    (s, v)
}

fn rect_of(s: &Scene, list: &[(u32, usize)], task: u32) -> Rect {
    s.rects[list.iter().find(|e| e.0 == task).unwrap().1]
}

#[test]
fn a_bar_has_air_and_rounded_corners() {
    let (s, v) = scene_with(&[Actuals::Idle; 2], None, 1.0);
    let share = bar_height(&v) / v.row_height;
    assert!((0.45..=0.55).contains(&share), "bar is {share} of the row");
    let body = rect_of(&s, &s.bar_rects, 1);
    assert_eq!(body.size[1], bar_height(&v) as f32);
    assert_eq!(
        body.radius, 2.0,
        "the default radius, one pixel per CSS pixel"
    );
    let (s2, _) = scene_with(&[Actuals::Idle; 2], None, 2.0);
    assert_eq!(
        rect_of(&s2, &s2.bar_rects, 1).radius,
        4.0,
        "scales with dpr"
    );
}

#[test]
fn the_progress_quad_ends_at_the_data_date_for_a_started_bar() {
    // Task 0 runs [0, 8h) and started at its left end; the data date is a quarter, then three
    // quarters of the way along it. The quad is from the start to the data date, not a percent.
    for (data_date, share) in [(2 * 60, 0.25), (6 * 60, 0.75)] {
        let (s, v) = scene_with(&[started(0)], Some(data_date), 1.0);
        assert_eq!(s.bar_progress.len(), 1, "one quad for the started bar");
        let (quad, body) = (
            rect_of(&s, &s.bar_progress, 0),
            rect_of(&s, &s.bar_rects, 0),
        );
        assert_eq!(quad.pos, body.pos, "starts where the bar does");
        let end = quad.pos[0] + quad.size[0];
        let at = v.x_of(data_date as f64) as f32;
        assert!(
            (end - at).abs() <= 1.0,
            "ends at the data date: {end} vs {at}"
        );
        let wide = (quad.size[0] / body.size[0]) as f64;
        assert!(
            (wide - share).abs() < 0.05,
            "{wide} of the bar, want {share}"
        );
        assert_eq!(quad.color, THEME.crit_fill, "task 0 is critical");
    }
}

#[test]
fn a_started_bar_whose_data_date_is_past_its_end_is_filled_whole() {
    let (s, _) = scene_with(&[started(0)], Some(3 * DAY), 1.0);
    let (quad, body) = (
        rect_of(&s, &s.bar_progress, 0),
        rect_of(&s, &s.bar_rects, 0),
    );
    assert_eq!(quad.size[0], body.size[0]);
}

#[test]
fn a_started_bar_with_no_data_date_or_a_data_date_before_its_start_has_no_quad() {
    // Task 1 starts on day 1.
    let (s, _) = scene_with(&[Actuals::Idle, started(1)], None, 1.0);
    assert!(s.bar_progress.is_empty(), "no data date");
    let (s, _) = scene_with(&[Actuals::Idle, started(1)], Some(DAY), 1.0);
    assert!(s.bar_progress.is_empty(), "the data date is at its start");
    let (s, _) = scene_with(&[Actuals::Idle, started(1)], Some(DAY / 2), 1.0);
    assert!(
        s.bar_progress.is_empty(),
        "the data date is before its start"
    );
}

#[test]
fn a_finished_bar_is_filled_whole_and_an_unstarted_one_not_at_all() {
    let all = [Actuals::Idle, finished(1), started(2), Actuals::Idle];
    let (s, _) = scene_with(&all, Some(2 * DAY + 4 * 60), 1.0);
    let tasks: Vec<u32> = s.bar_progress.iter().map(|e| e.0).collect();
    assert_eq!(tasks, [1, 2], "none for a task with no actual start");
    let (done, body) = (
        rect_of(&s, &s.bar_progress, 1),
        rect_of(&s, &s.bar_rects, 1),
    );
    assert_eq!((done.pos, done.size), (body.pos, body.size), "whole");
    assert_eq!(done.color, THEME.bar);
    assert_eq!(done.radius, body.radius);
    let half = rect_of(&s, &s.bar_progress, 2);
    let body2 = rect_of(&s, &s.bar_rects, 2);
    assert!((half.size[0] - body2.size[0] / 2.0).abs() <= 1.0, "half");
    assert_eq!(half.size[1], body2.size[1], "same height as the body");
}

#[test]
fn no_progress_given_draws_no_progress_quad() {
    let d = dates(2);
    let mut v = Viewport::new(800.0, 300.0, 2);
    v.fit(0.0, 4.0 * DAY as f64);
    let s = build(&v, 1.0, &[0, 1], &d, &Summaries::default(), None, None);
    assert_eq!(s.bar_progress, []);
}

#[test]
fn progress_quads_follow_the_visible_rows_not_the_task_count() {
    let n = 50_000;
    let all: Vec<Actuals> = (0..n as i64).map(finished).collect();
    let (s, _) = scene_with(&all, None, 1.0);
    assert!(s.bars < 40, "{} bars", s.bars);
    assert_eq!(s.bar_progress.len(), s.bars);
}

#[test]
fn a_highlighted_bar_keeps_its_accent_and_shows_progress_on_it() {
    let (mut s, _) = scene_with(&[finished(0), finished(1)], None, 1.0);
    apply_highlight(&mut s, &Highlight::new(2, [1], []));
    assert_eq!(rect_of(&s, &s.bar_rects, 1).color, THEME.sel_edge);
    let on = rect_of(&s, &s.bar_progress, 1).color;
    assert_eq!(on, THEME.bar_sel_progress);
    let off = rect_of(&s, &s.bar_progress, 0).color;
    assert_eq!(off[3], THEME.crit[3] * DIM, "the others dim");
}

#[test]
fn a_highlighted_bars_progress_colour_is_the_token_whole_alpha_included() {
    let (mut s, _) = scene_with(&[finished(0), finished(1)], None, 1.0);
    let theme = THEME
        .with_json(r##"{"bar-sel-progress":"#12345600"}"##)
        .unwrap();
    crate::scene::apply_highlight(&mut s, &Highlight::new(2, [1], []), &theme);
    let on = rect_of(&s, &s.bar_progress, 1).color;
    assert_eq!(on, theme.bar_sel_progress);
    assert_eq!(on[3], 0.0, "a transparent token leaves no progress overlay");
}

#[test]
fn arrows_shift_the_progress_quads_with_the_bars() {
    let (mut s, v) = scene_with(&[finished(0), finished(1)], None, 1.0);
    let before: Vec<Rect> = s.bar_progress.iter().map(|e| s.rects[e.1]).collect();
    let d = dates(2);
    let dep = Dependency {
        pred: TaskId(0),
        succ: TaskId(1),
        kind: LinkType::FS,
        lag: 0,
    };
    add_arrows(
        &mut s,
        &v,
        1.0,
        &d,
        &LinkIndex::new(&[dep], |_| true, &[0, 1]),
    );
    assert!(s.arrows > 0);
    let after: Vec<Rect> = s.bar_progress.iter().map(|e| s.rects[e.1]).collect();
    assert_eq!(before, after);
}

#[test]
fn radius_zero_is_square_everywhere() {
    let n = 2;
    let tasks = [task(finished(0)), task(finished(1))];
    let d = dates(n);
    let order: Vec<u32> = (0..n as u32).collect();
    let mut v = Viewport::new(1200.0, 300.0, n);
    v.fit(0.0, 10.0 * DAY as f64);
    v.bar_radius = 0.0;
    let backdrop = Backdrop {
        progress: Some(&tasks),
        ..Backdrop::default()
    };
    let s = build_marked(
        &v,
        1.0,
        &order,
        &d,
        &Summaries::default(),
        RowMarks::default(),
        Some((0, false)),
        backdrop,
        &THEME,
    );
    // The critical edge, the drag outline, the bodies and the progress quads.
    assert!(s.rects.iter().any(|r| r.color == THEME.crit_edge));
    assert!(s.rects.iter().any(|r| r.color == THEME.text));
    for r in s.rects.iter().filter(|r| {
        [THEME.crit_edge, THEME.text, THEME.bar_soft, THEME.crit_soft].contains(&r.color)
            || [THEME.bar, THEME.crit_fill].contains(&r.color)
    }) {
        assert_eq!(r.radius, 0.0, "{:?} is square", r.color);
    }
}

#[test]
fn a_milestone_is_a_diamond_not_a_square() {
    for theme in [Theme::light(), Theme::dark()] {
        let mut d = dates(2);
        // Task 0 is a milestone with float (plain); task 1 a milestone on the critical path.
        d[0].ef = d[0].es;
        d[1].ef = d[1].es;
        d[0].float = 480;
        d[1].float = 0;
        let task = TaskDates {
            es: 2 * DAY,
            ef: 2 * DAY + 8 * 60,
            ls: 2 * DAY,
            lf: 2 * DAY + 8 * 60,
            float: 480,
            finished: false,
        };
        d.push(task);
        let order: Vec<u32> = (0..3).collect();
        let mut v = Viewport::new(1200.0, 300.0, 3);
        v.fit(0.0, 10.0 * DAY as f64);
        let s = super::super::build(
            &v,
            1.0,
            &order,
            &d,
            &Summaries::default(),
            None,
            None,
            &theme,
        );
        let (plain, crit, bar) = (
            rect_of(&s, &s.bar_rects, 0),
            rect_of(&s, &s.bar_rects, 1),
            rect_of(&s, &s.bar_rects, 2),
        );
        assert!(plain.radius < 0.0, "a plain milestone carries the marker");
        assert!(crit.radius < 0.0, "a critical milestone carries the marker");
        assert_eq!(plain.color, theme.bar, "a milestone is solid");
        assert_eq!(crit.color, theme.crit_fill, "a critical milestone is solid");
        assert_eq!(plain.size[0], plain.size[1], "as wide as it is tall");
        assert!(bar.radius >= 0.0, "a task is not a diamond");
    }
}

#[test]
fn a_milestone_at_the_canvas_edge_is_still_a_whole_diamond() {
    let (mut d, mut v) = (dates(1), Viewport::new(1200.0, 300.0, 1));
    v.fit(0.0, 10.0 * DAY as f64);
    // Its date is the right canvas edge: half of the diamond lies past it.
    (d[0].es, d[0].float) = (v.t_of(v.width) as i64, 480);
    d[0].ef = d[0].es;
    let s = super::super::build(&v, 1.0, &[0], &d, &Summaries::default(), None, None, &THEME);
    let diamond = rect_of(&s, &s.bar_rects, 0);
    assert_eq!(
        diamond.size[0], diamond.size[1],
        "a diamond sits in a square"
    );
}
