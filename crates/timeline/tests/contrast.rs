//! WCAG AA (4.5:1) for a task's name drawn inside its bar, from the theme tokens (gs#329).

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
use timeline::scene::{self, Highlight};
use timeline::{Summaries, Theme, Viewport};

fn linear(c: f32) -> f32 {
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: [f32; 4]) -> f32 {
    0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
}

fn ratio(a: [f32; 4], b: [f32; 4]) -> f32 {
    let (hi, lo) = (
        luminance(a).max(luminance(b)),
        luminance(a).min(luminance(b)),
    );
    (hi + 0.05) / (lo + 0.05)
}

/// `top` laid over the opaque `under`, by `top`'s own alpha.
fn over(top: [f32; 4], under: [f32; 4]) -> [f32; 4] {
    let mix = |i: usize| top[i] * top[3] + under[i] * (1.0 - top[3]);
    [mix(0), mix(1), mix(2), 1.0]
}

#[test]
fn a_name_inside_a_bar_reaches_aa_in_both_themes() {
    for (name, t) in [("light", Theme::light()), ("dark", Theme::dark())] {
        // Unselected: `bar_ink` over the pastel body and the progress fill, normal and critical.
        for (what, under) in [
            ("bar", t.bar),
            ("crit_fill", t.crit_fill),
            ("bar_soft", t.bar_soft),
            ("crit_soft", t.crit_soft),
        ] {
            let r = ratio(t.bar_ink, under);
            assert!(r >= 4.5, "{name}: bar_ink on {what} is {r:.2}:1");
        }
        // Highlighted: `bar_text` over the accent and over its progress overlay.
        let overlay = over(t.bar_sel_progress, t.sel_edge);
        for (what, under) in [("sel_edge", t.sel_edge), ("bar_sel_progress", overlay)] {
            let r = ratio(t.bar_text, under);
            assert!(r >= 4.5, "{name}: bar_text on {what} is {r:.2}:1");
        }
    }
}

#[test]
fn a_highlighted_bars_name_is_drawn_in_bar_text() {
    let theme = Theme::light();
    let dates = vec![TaskDates {
        es: 0,
        ef: 3 * DAY,
        ls: 0,
        lf: 3 * DAY,
        float: 0,
        finished: false,
    }];
    let order = [0];
    let mut viewport = Viewport::new(900.0, 240.0, 1);
    viewport.fit(0.0, 3.0 * DAY as f64);
    let summaries = Summaries::new(1);
    let highlight = Highlight::new(1, [0], []);
    for (highlight, want) in [(None, theme.bar_ink), (Some(&highlight), theme.bar_text)] {
        let mut s = scene::build(
            &viewport, 1.0, &order, &dates, &summaries, None, None, &theme,
        );
        labels::add_bar_labels(
            &mut s,
            &viewport,
            1.0,
            Rows {
                order: &order,
                dates: &dates,
                summaries: &summaries,
                highlight,
                theme: &theme,
                terminal_reach: 11.0,
                rings: &[],
            },
            |_| "name",
            |_, _, _| 20.0,
        );
        let label = &s.bar_labels[0];
        assert_eq!(label.placement, Placement::Inside);
        assert_eq!(label.color, want);
    }
}

#[test]
fn a_comparison_bar_reaches_three_to_one_on_every_row_fill_in_both_themes() {
    for (name, t) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (what, under) in [
            ("bg", t.bg),
            ("stripe", t.stripe),
            ("hov", t.hov),
            ("sel", t.sel),
        ] {
            let r = ratio(t.baseline, under);
            assert!(r >= 3.0, "{name}: baseline on {what} is {r:.2}:1");
        }
    }
}
