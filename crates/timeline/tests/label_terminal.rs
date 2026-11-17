//! A name beside a bar stays clear of that bar's link terminal (gs#440).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{DAY, TaskDates};
use timeline::labels::{self, Placement, Rows};
use timeline::scene;
use timeline::{Summaries, Theme, Viewport};

const SIZE: f64 = 8.0;
const GAP: f64 = 3.0;

fn one_task() -> [TaskDates; 1] {
    [TaskDates {
        es: 0,
        ef: 3 * DAY,
        ls: 0,
        lf: 3 * DAY,
        float: 0,
        finished: false,
    }]
}

/// The terminal and the name of the one task, `bar_ratio` of the row tall, with `reach` as the
/// labels pass takes it. Returns the terminal's right edge and the label's left edge, both
/// physical pixels, after checking the two share rows (so a clash is possible at all).
fn edges(bar_ratio: f64, dpr: f64, reach: f64) -> (f32, f32) {
    let theme = Theme::light();
    let dates = one_task();
    let order = [0];
    let summaries = Summaries::new(1);
    let mut viewport = Viewport::new(900.0, 240.0, 1);
    viewport.bar_ratio = bar_ratio;
    viewport.fit(0.0, 30.0 * DAY as f64);
    let mut s = scene::build(
        &viewport, dpr, &order, &dates, &summaries, None, None, &theme,
    );
    scene::add_link_terminal(&mut s, &viewport, dpr, 0, &dates[0], SIZE, GAP, &theme);
    let dot = *s.rects.last().expect("the scene has a rect");
    assert_eq!(
        dot.color, theme.link_terminal,
        "the last rect is the terminal"
    );
    labels::add_bar_labels(
        &mut s,
        &viewport,
        dpr,
        Rows {
            order: &order,
            dates: &dates,
            summaries: &summaries,
            highlight: None,
            theme: &theme,
            terminal_reach: reach,
            rings: &[],
        },
        |_| "name",
        |_, _, px| px * 4.0,
    );
    let label = &s.bar_labels[0];
    assert_eq!(label.placement, Placement::Right);
    let line = (label.px * 1.2).ceil();
    let (dot_mid, label_mid) = (dot.pos[1] + dot.size[1] / 2.0, label.y + line / 2.0);
    assert!(
        (dot_mid - label_mid).abs() < line / 2.0,
        "the name and the terminal share rows: dot {dot_mid}, name {label_mid}"
    );
    (dot.pos[0] + dot.size[0], label.x)
}

#[test]
fn the_name_beside_a_bar_starts_past_the_terminal_at_both_bar_heights() {
    for bar_ratio in [0.5, 0.62] {
        for dpr in [1.0, 2.0] {
            let (dot_right, name_left) = edges(bar_ratio, dpr, GAP + SIZE);
            assert!(
                name_left >= dot_right,
                "bar {bar_ratio}, dpr {dpr}: the name starts at {name_left}, the terminal ends at {dot_right}"
            );
        }
    }
}

#[test]
fn without_the_reach_the_name_would_sit_on_the_terminal() {
    // The check above must be able to fail: with no reach the name starts inside the dot.
    let (dot_right, name_left) = edges(0.62, 1.0, 0.0);
    assert!(name_left < dot_right);
}
