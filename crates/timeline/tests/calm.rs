//! The palette's quiet (gs#409, Direction C of gs#305; revised by tl#1193, Dave 2026-11-02, who chose
//! diazo-blue bars over grey ones): ordinary bars are the one linework blue, the soft bars, the
//! summary, the links and the grid stay quiet, and none of them is red, so the critical path is the
//! only red on the chart.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use timeline::Theme;

/// The 8-bit channels of `c`.
fn channels(c: [f32; 4]) -> [i32; 3] {
    [c[0], c[1], c[2]].map(|v| (v * 255.0).round() as i32)
}

/// The largest difference between two channels: how far from a grey.
fn chroma(c: [f32; 4]) -> u32 {
    let ch = channels(c);
    (ch.iter().max().expect("three channels") - ch.iter().min().expect("three channels")) as u32
}

/// The hue in degrees, or `None` for a grey.
fn hue(color: [f32; 4]) -> Option<f64> {
    let [r, g, b] = channels(color).map(f64::from);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let spread = max - min;
    if spread == 0.0 {
        return None;
    }
    let degrees = if max == r {
        60.0 * (((g - b) / spread) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / spread + 2.0)
    } else {
        60.0 * ((r - g) / spread + 4.0)
    };
    Some((degrees + 360.0) % 360.0)
}

#[test]
fn bars_are_the_linework_blue_and_the_rest_stays_quiet() {
    for (name, t) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (what, c) in [("bar", t.bar), ("bar_edge", t.bar_edge)] {
            let h = hue(c).unwrap_or(0.0);
            assert!(
                (215.0..=235.0).contains(&h) && chroma(c) >= 0x40,
                "{name}: {what} is not the linework blue (hue {h:.0}, chroma {:#x})",
                chroma(c)
            );
        }
        for (what, c) in [
            ("bar_soft", t.bar_soft),
            ("sum", t.sum),
            ("link", t.link),
            ("grid", t.grid),
            ("grid_major", t.grid_major),
        ] {
            let red = hue(c).is_some_and(|h| !(15.0..=345.0).contains(&h));
            assert!(
                chroma(c) <= 0x40 && !red,
                "{name}: {what} is not quiet (chroma {:#x}, red {red})",
                chroma(c)
            );
        }
    }
}
