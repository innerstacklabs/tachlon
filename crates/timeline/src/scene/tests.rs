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
use crate::summary::Summaries;
use engine::DAY;

const THEME: Theme = Theme::light();

fn build(
    v: &Viewport,
    dpr: f64,
    order: &[u32],
    dates: &[TaskDates],
    summaries: &Summaries,
    hover: Option<usize>,
    dragged: Option<(u32, bool)>,
) -> Scene {
    super::build(v, dpr, order, dates, summaries, hover, dragged, &THEME)
}

fn apply_highlight(scene: &mut Scene, highlight: &Highlight) {
    super::apply_highlight(scene, highlight, &THEME);
}

fn add_arrows(scene: &mut Scene, v: &Viewport, dpr: f64, dates: &[TaskDates], links: &LinkIndex) {
    super::add_arrows(scene, v, dpr, dates, links, &THEME);
}

fn add_arrows_highlighted(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    dates: &[TaskDates],
    index: &LinkIndex,
    highlight: Option<&Highlight>,
) {
    super::add_arrows_highlighted(scene, v, dpr, dates, index, highlight, None, &THEME);
}

fn dates(n: usize) -> Vec<TaskDates> {
    (0..n as i64)
        .map(|i| TaskDates {
            es: i * DAY,
            ef: i * DAY + 8 * 60,
            ls: i * DAY + (i % 2) * DAY,
            lf: i * DAY + 8 * 60 + (i % 2) * DAY,
            float: (i % 2) * 480,
            finished: false,
        })
        .collect()
}

mod backdrop;
mod bar_color;
mod bars;
mod constraint;
mod header;
mod links;
mod long_links;
mod look;
