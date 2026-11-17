#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use super::*;
use engine::DAY;

fn dates() -> Vec<TaskDates> {
    [(5, 3), (0, 2), (5, 1)]
        .iter()
        .map(|&(es, days)| TaskDates {
            es: es * DAY,
            ef: (es + days) * DAY,
            ls: es * DAY,
            lf: (es + days) * DAY,
            float: 0,
            finished: false,
        })
        .collect()
}

#[test]
fn order_is_by_start_then_id() {
    assert_eq!(order_by_start(&dates()), vec![1, 0, 2]);
}

#[test]
fn new_with_an_order_shorter_than_the_task_count_keeps_every_visible_row() {
    let tl = Timeline::new(vec![0, 1, 5], 1000.0, 400.0, 1.0);

    assert_eq!(tl.row_of_task(0), Some(0));
    assert_eq!(tl.row_of_task(1), Some(1));
    assert_eq!(tl.row_of_task(5), Some(2));
    assert_eq!(tl.row_of_task(3), None);
    assert_eq!(tl.row_of_task(9), None);
    assert_eq!(tl.viewport.rows, 3);
}

#[test]
fn new_with_an_empty_order_has_no_task_rows() {
    let tl = Timeline::new(vec![], 1000.0, 400.0, 1.0);

    assert_eq!(tl.row_of_task(0), None);
}

#[test]
fn hit_test_by_row_then_time() {
    let d = dates();
    let mut tl = Timeline::new(order_by_start(&d), 1000.0, 400.0, 1.0);
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    let v = &tl.viewport;
    let row_y = |r: usize| v.row_top(r) + v.row_height / 2.0;
    let x_day = |d: f64| v.x_of(d * DAY as f64);
    assert_eq!(tl.hit_test(&d, x_day(1.0), row_y(0)), Some(1));
    assert_eq!(tl.hit_test(&d, x_day(3.0), row_y(0)), None);
    assert_eq!(tl.hit_test(&d, x_day(7.0), row_y(1)), Some(0));
    assert_eq!(tl.hit_test(&d, x_day(7.0), row_y(2)), None);
    assert_eq!(tl.hit_test(&d, x_day(5.5), row_y(2)), Some(2));
    assert_eq!(tl.hit_test(&d, x_day(5.5), 5.0), None); // axis band
}

#[test]
fn hover_and_hit_return_the_summary_over_its_span() {
    let d = dates();
    let mut tl = Timeline::new(vec![0, 1, 2], 1000.0, 400.0, 1.0);
    *tl.summaries_mut() = Summaries::new(d.len());
    let span = (DAY, 3 * DAY);
    tl.summaries_mut().set(0, Some(span));
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    let y = tl.viewport.row_top(0) + tl.viewport.row_height / 2.0;
    let middle = tl.viewport.x_of(2.0 * DAY as f64);

    assert_eq!(tl.hit_test(&d, middle, y), Some(0), "[summary-hit]");
    assert_eq!(
        tl.hit_test(&d, tl.viewport.x_of(span.0 as f64) - 0.5, y),
        None,
        "[summary-hit]"
    );
    assert_eq!(
        tl.hit_test(&d, tl.viewport.x_of(span.1 as f64) + 0.5, y),
        None,
        "[summary-hit]"
    );
    tl.pointer_move(middle, y);
    assert_eq!(tl.hovered(&d), Some(0), "[summary-hit]");
}

#[test]
fn the_scene_fills_the_outside_hover_row_with_the_hover_token() {
    let d = dates();
    let theme = Theme::default();
    let mut tl = Timeline::new(vec![0, 1, 2], 1000.0, 400.0, 1.0);
    let top = tl.viewport.row_top(2) as f32;
    let hover_fills = |tl: &Timeline| -> Vec<f32> {
        tl.scene(&d, false, None, None, scene::Backdrop::default(), &theme)
            .rects
            .iter()
            .filter(|r| r.color == theme.hov)
            .map(|r| r.pos[1])
            .collect()
    };
    assert!(hover_fills(&tl).is_empty(), "no row is hovered yet");
    tl.set_outside_hover(Some(2));
    assert_eq!(hover_fills(&tl), vec![top], "the table's row gets --hov");
    tl.set_outside_hover(None);
    assert!(hover_fills(&tl).is_empty(), "clearing removes the fill");
}

#[test]
fn the_hover_row_is_the_canvas_pointer_then_the_outside_row() {
    let mut tl = Timeline::new(vec![0, 1, 2], 1000.0, 400.0, 1.0);
    assert_eq!(tl.hover_row(), None);
    tl.set_outside_hover(Some(2));
    assert_eq!(tl.hover_row(), Some(2), "the table's row shows");
    let y = tl.viewport.row_top(1) + tl.viewport.row_height / 2.0;
    tl.pointer_move(100.0, y);
    assert_eq!(tl.hover_row(), Some(1), "the canvas pointer wins");
    tl.pointer_leave();
    assert_eq!(tl.hover_row(), Some(2), "the outside row returns");
    tl.set_outside_hover(Some(7));
    assert_eq!(tl.hover_row(), None, "a row past the order is ignored");
    tl.set_outside_hover(None);
    assert_eq!(tl.hover_row(), None);
}

#[test]
fn pressing_a_summary_pans_instead_of_dragging() {
    let d = dates();
    let mut tl = Timeline::new(vec![0, 1, 2], 1000.0, 400.0, 1.0);
    *tl.summaries_mut() = Summaries::new(d.len());
    tl.summaries_mut().set(0, Some((DAY, 3 * DAY)));
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    let summary_y = tl.viewport.row_top(0) + tl.viewport.row_height / 2.0;
    let summary_x = tl.viewport.x_of(2.0 * DAY as f64);

    tl.pointer_down(&d, summary_x, summary_y);
    assert_eq!(tl.bar_drag(), None, "[summary-pan]");
    assert!(tl.dragging(), "[summary-pan]");
    let fixed = 4.0 * DAY as f64;
    let before = tl.viewport.x_of(fixed);
    tl.pointer_move(summary_x + 25.0, summary_y);
    assert!(
        (tl.viewport.x_of(fixed) - (before + 25.0)).abs() < 1e-9,
        "[summary-pan]"
    );

    tl.pointer_up();
    let leaf_y = tl.viewport.row_top(1) + tl.viewport.row_height / 2.0;
    let leaf_x = tl.viewport.x_of(DAY as f64);
    tl.pointer_down(&d, leaf_x, leaf_y);
    assert_eq!(
        tl.bar_drag().map(|drag| drag.task),
        Some(1),
        "[summary-pan]"
    );
    assert!(!tl.dragging(), "[summary-pan]");
}

#[test]
fn a_hidden_task_is_not_hit_even_with_a_summary_span() {
    let d = dates();
    let mut tl = Timeline::new(vec![0, 1, 2], 1000.0, 400.0, 1.0);
    *tl.summaries_mut() = Summaries::new(d.len());
    tl.summaries_mut().set(2, Some((DAY, 3 * DAY)));
    tl.set_order(vec![0, 1]);
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    let x = tl.viewport.x_of(2.0 * DAY as f64);

    // Rows past the new order included: the old row 2 holds no task now.
    for row in 0..d.len() {
        let y = tl.viewport.row_top(row) + tl.viewport.row_height / 2.0;
        assert_ne!(tl.hit_test(&d, x, y), Some(2), "[summary-hidden]");
    }
}

#[test]
fn c_jumps_through_critical_tasks_and_wraps() {
    let d = days_with_critical(&[4, 17, 25]);
    let mut tl = Timeline::new(order_by_start(&d), 1000.0, 200.0, 1.0);
    tl.viewport.fit(0.0, 30.0 * DAY as f64);
    let jumps: Vec<_> = (0..4).map(|_| tl.jump_to_next_critical(&d)).collect();
    assert_eq!(jumps, vec![Some(4), Some(17), Some(25), Some(4)]);
    let none: Vec<TaskDates> = d.iter().map(|x| TaskDates { float: 1, ..*x }).collect();
    assert_eq!(tl.jump_to_next_critical(&none), None);
}

#[test]
fn c_skips_summary_rows() {
    let d = dates();
    let mut tl = Timeline::new(vec![0, 1, 2], 1000.0, 400.0, 1.0);
    *tl.summaries_mut() = Summaries::new(d.len());
    tl.summaries_mut().set(1, Some((d[1].es, d[1].ef)));
    tl.viewport.fit(0.0, 10.0 * DAY as f64);

    let jumps: Vec<_> = (0..5).map(|_| tl.jump_to_next_critical(&d)).collect();
    assert_eq!(
        jumps,
        vec![Some(0), Some(2), Some(0), Some(2), Some(0)],
        "[summary-c]"
    );
    assert!(jumps.iter().all(|&row| row != Some(1)), "[summary-c]");
}

#[test]
fn set_order_rebuilds_rows_and_clamps_the_view() {
    let d = days_with_critical(&[25]);
    let mut tl = Timeline::new(order_by_start(&d), 1000.0, 200.0, 1.0);
    tl.viewport.fit(0.0, 30.0 * DAY as f64);
    tl.viewport.scroll_to_row(25);
    assert_eq!(tl.jump_to_next_critical(&d), Some(25));
    assert!(tl.viewport.scroll_y > 0.0);

    tl.set_order(vec![4, 17]);

    assert_eq!(tl.row_of().len(), 30);
    assert_eq!(tl.row_of()[4], 0);
    assert_eq!(tl.row_of()[17], 1);
    assert_eq!(tl.row_of()[25], HIDDEN_ROW);
    assert_eq!(tl.row_of_task(4), Some(0));
    assert_eq!(tl.row_of_task(25), None);
    assert_eq!(tl.viewport.rows, 2);
    assert_eq!(tl.viewport.scroll_y, 0.0);
    assert_eq!(tl.last_jump, None);

    let row0 = tl.viewport.row_top(0) + tl.viewport.row_height / 2.0;
    let task0 = tl.viewport.x_of(0.5 * DAY as f64);
    assert_eq!(tl.hit_test(&d, task0, row0), None);
}

#[test]
fn filtered_critical_jump_skips_refused_tasks() {
    let d = days_with_critical(&[4, 17]);
    let mut tl = Timeline::new(order_by_start(&d), 1000.0, 200.0, 1.0);
    tl.viewport.fit(0.0, 30.0 * DAY as f64);
    tl.set_order(vec![4, 17]);

    assert_eq!(
        tl.jump_to_next_critical_where(&d, |task| task != 4),
        Some(1)
    );
}

/// 30 tasks, one per day; the tasks in `critical` have no float.
fn days_with_critical(critical: &[i64]) -> Vec<TaskDates> {
    (0..30)
        .map(|i| TaskDates {
            es: i * DAY,
            ef: (i + 1) * DAY,
            ls: i * DAY,
            lf: (i + 1) * DAY,
            float: if critical.contains(&i) { 0 } else { 60 },
            finished: false,
        })
        .collect()
}

#[test]
fn c_visits_the_top_row_first_whatever_the_top_row() {
    let d = days_with_critical(&[0, 17, 25]);
    let timeline_at = |row: usize| {
        let mut tl = Timeline::new(order_by_start(&d), 1000.0, 200.0, 1.0);
        tl.viewport.fit(0.0, 30.0 * DAY as f64);
        tl.viewport.scroll_to_row(row);
        assert_eq!(
            tl.viewport.visible_rows().start,
            row,
            "scrolled to row {row}"
        );
        tl
    };
    // Top row 0 is critical: it is the first stop, and the last one is
    // the tail of the cycle (not the other way round).
    let mut tl = timeline_at(0);
    let jumps: Vec<_> = (0..4).map(|_| tl.jump_to_next_critical(&d)).collect();
    assert_eq!(jumps, vec![Some(0), Some(17), Some(25), Some(0)]);
    // The same with a critical top row anywhere else.
    let mut tl = timeline_at(17);
    let jumps: Vec<_> = (0..4).map(|_| tl.jump_to_next_critical(&d)).collect();
    assert_eq!(jumps, vec![Some(17), Some(25), Some(0), Some(17)]);
    // And with a top row that is not critical: the next one down.
    let mut tl = timeline_at(1);
    assert_eq!(tl.jump_to_next_critical(&d), Some(17));
}

#[test]
fn the_wheel_does_nothing_during_a_bar_drag() {
    let d = dates();
    let mut tl = Timeline::new(order_by_start(&d), 1000.0, 400.0, 1.0);
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    let before = tl.viewport.clone();
    let row0 = before.row_top(0) + before.row_height / 2.0;
    tl.pointer_down(&d, before.x_of(0.5 * DAY as f64), row0);
    assert!(tl.bar_drag().is_some());
    // Pan, scroll and zoom: the view stays under the still pointer.
    tl.wheel(500.0, row0, 120.0, 90.0, false);
    tl.wheel(500.0, row0, 0.0, -120.0, true);
    assert_eq!(tl.viewport.t_of(500.0), before.t_of(500.0));
    assert_eq!(tl.viewport.px_per_minute, before.px_per_minute);
    assert_eq!(tl.viewport.scroll_y, before.scroll_y);
    // After the release the wheel works again.
    tl.pointer_up();
    tl.wheel(500.0, row0, 0.0, -120.0, true);
    assert_ne!(tl.viewport.px_per_minute, before.px_per_minute);
}

#[test]
fn pressing_a_bar_drags_it_without_panning() {
    let d = dates();
    let mut tl = Timeline::new(order_by_start(&d), 1000.0, 400.0, 1.0);
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    let v = tl.viewport.clone();
    let row0 = v.row_top(0) + v.row_height / 2.0;
    // Task 1 (row 0) runs days 0-2; grab it half a day in.
    tl.pointer_down(&d, v.x_of(0.5 * DAY as f64), row0);
    let b = tl.bar_drag().expect("a bar drag");
    assert_eq!(b.task, 1);
    assert!(!tl.dragging());
    // Move the pointer three days right: the bar asks to start on day 3.
    tl.pointer_move(v.x_of(3.5 * DAY as f64), row0);
    assert_eq!(tl.bar_drag().unwrap().wanted_start(&tl.viewport), 3 * DAY);
    assert_eq!(tl.viewport.x_of(0.0), v.x_of(0.0), "a bar drag never pans");
    tl.pointer_leave(); // pointer capture: the drag goes on
    assert!(tl.bar_drag().is_some());
    tl.pointer_up();
    assert!(tl.bar_drag().is_none());
}

#[test]
fn drag_pans_and_ctrl_wheel_zooms_at_the_cursor() {
    let d = dates();
    let mut tl = Timeline::new(order_by_start(&d), 1000.0, 400.0, 2.0);
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    let t = tl.viewport.t_of(300.0);
    // y = 5 is the axis band: no bar there, so this is a pan.
    tl.pointer_down(&d, 300.0, 5.0);
    assert!(tl.dragging() && tl.bar_drag().is_none());
    tl.pointer_move(350.0, 5.0);
    tl.pointer_move(420.0, 5.0);
    tl.pointer_up();
    assert!((tl.viewport.x_of(t) - 420.0).abs() < 1e-9);
    tl.pointer_move(600.0, 5.0); // not dragging: no pan
    assert!((tl.viewport.x_of(t) - 420.0).abs() < 1e-9);
    let t = tl.viewport.t_of(733.0);
    tl.wheel(733.0, 50.0, 0.0, -120.0, true);
    assert!((tl.viewport.x_of(t) - 733.0).abs() < 1e-6);
    let before = tl.viewport.px_per_minute;
    tl.wheel(733.0, 50.0, 0.0, 100.0, false);
    assert_eq!(tl.viewport.px_per_minute, before);
}

#[test]
fn the_scene_draws_the_progress_its_backdrop_carries_and_no_other() {
    let d = dates();
    let theme = Theme::default();
    let mut tl = Timeline::new(vec![0, 1, 2], 1000.0, 400.0, 1.0);
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    tl.set_progress(vec![50, 50, 50]); // the stored percent is never drawn
    let quads = |backdrop| {
        tl.scene(&d, false, None, None, backdrop, &theme)
            .bar_progress
            .len()
    };
    assert_eq!(
        quads(scene::Backdrop::default()),
        0,
        "the backdrop owns the value"
    );
    let task = |finished: bool| engine::Task {
        duration: 480,
        calendar: engine::CalendarId(0),
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: finished.then_some(0),
        actual_finish: finished.then_some(480),
        remaining: None,
    };
    let tasks = [task(true), task(false), task(true)];
    let carried = scene::Backdrop {
        progress: Some(&tasks),
        ..scene::Backdrop::default()
    };
    assert_eq!(quads(carried), 2);
}

#[test]
fn a_press_on_a_leaf_bars_finish_edge_is_told_from_its_body() {
    let at = |es: i64, ef: i64| TaskDates {
        es,
        ef,
        ls: es,
        lf: ef,
        float: 0,
        finished: false,
    };
    // 0: a leaf over days 1 to 5; 1: a milestone; 2: a summary; 3: a one-minute bar.
    let d = vec![
        at(DAY, 5 * DAY),
        at(DAY, DAY),
        at(DAY, 5 * DAY),
        at(DAY, DAY + 1),
    ];
    let mut tl = Timeline::new(vec![0, 1, 2, 3], 1000.0, 400.0, 1.0);
    *tl.summaries_mut() = Summaries::new(d.len());
    tl.summaries_mut().set(2, Some((DAY, 5 * DAY)));
    tl.viewport.fit(0.0, 10.0 * DAY as f64);
    let v = tl.viewport.clone();
    let row_y = |r: usize| v.row_top(r) + v.row_height / 2.0;
    let finish = v.x_of(5.0 * DAY as f64);
    let body = v.x_of(3.0 * DAY as f64);

    assert!(tl.hit_edge(&d, finish - 1.0, row_y(0)), "[bar-edge]");
    assert!(
        !tl.hit_edge(&d, finish - EDGE_PX - 1.0, row_y(0)),
        "[bar-edge]"
    );
    assert!(!tl.hit_edge(&d, body, row_y(0)), "[bar-edge]");
    assert!(!tl.hit_edge(&d, finish + 1.0, row_y(0)), "[bar-edge]");
    assert!(
        !tl.hit_edge(&d, v.x_of(DAY as f64) + 4.0, row_y(1)),
        "[bar-edge]"
    );
    assert!(!tl.hit_edge(&d, finish - 1.0, row_y(2)), "[bar-edge]");
    let tiny = v.x_of(DAY as f64) + 1.9;
    assert!(tl.hit_test(&d, tiny, row_y(3)).is_some(), "[bar-edge]");
    assert!(!tl.hit_edge(&d, tiny, row_y(3)), "[bar-edge]");

    tl.pointer_down(&d, finish - 1.0, row_y(0));
    assert!(tl.bar_drag().expect("a bar drag").edge, "[bar-edge]");
    tl.pointer_up();
    tl.pointer_down(&d, body, row_y(0));
    assert!(!tl.bar_drag().expect("a bar drag").edge, "[bar-edge]");
}
