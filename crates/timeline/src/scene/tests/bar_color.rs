use super::look::{Actuals, task};
use super::*;
use crate::labels::{Placement, Rows, add_bar_labels};
use crate::text::bar_spans;

const GOLD: [f32; 4] = [1.0, 0.843, 0.0, 1.0];
const NAVY: [f32; 4] = [0.0, 0.2, 0.4, 1.0];

#[derive(Debug)]
struct Paint([f32; 4]);

impl BarColors for Paint {
    fn bar_color(&self, task: u32) -> Option<[f32; 4]> {
        (task != 3).then_some(self.0)
    }
}

/// WCAG 2 contrast, with its sRGB curve (a 2.2 gamma misses the 2:1 edge by a hair).
fn ratio(a: [f32; 4], b: [f32; 4]) -> f32 {
    let lin = |v: f32| match v {
        ..=0.04045 => v / 12.92,
        _ => ((v + 0.055) / 1.055).powf(2.4),
    };
    let l = |c: [f32; 4]| 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
    (l(a).max(l(b)) + 0.05) / (l(a).min(l(b)) + 0.05)
}

/// The scene and names of `d` on a canvas fitted to its end, under `Paint(c)`, every task started
/// at its start and the data date halfway through task 0.
fn build(theme: &Theme, c: [f32; 4], d: &[TaskDates], dpr: f64) -> Scene {
    let order: Vec<u32> = (0..d.len() as u32).collect();
    let (sums, mut v) = (Summaries::default(), Viewport::new(900.0, 240.0, d.len()));
    v.fit(0.0, d[d.len() - 1].ef as f64);
    let tasks: Vec<_> = d.iter().map(|t| task(Actuals::Started(t.es))).collect();
    let backdrop = Backdrop {
        bar_colors: Some(&Paint(c)),
        progress: Some(&tasks),
        data_date: Some(d[0].es.midpoint(d[0].ef)),
        ..Backdrop::default()
    };
    let mut s = build_marked(
        &v,
        dpr,
        &order,
        d,
        &sums,
        RowMarks::default(),
        None,
        backdrop,
        theme,
    );
    let rows = Rows {
        order: &order,
        dates: d,
        summaries: &sums,
        highlight: None,
        theme,
        terminal_reach: 11.0,
        rings: &[],
    };
    add_bar_labels(&mut s, &v, dpr, rows, |_| "name", |_, _, _| 600.0);
    s
}

fn body(s: &Scene, task: u32) -> [f32; 4] {
    s.bar_rects
        .iter()
        .find(|e| e.0 == task)
        .map(|e| s.rects[e.1].color)
        .expect("a bar")
}

/// Tasks 0 to 4 of `dates`: critical, plain, critical, plain (unpainted), a milestone.
fn five() -> Scene {
    let mut d = dates(5);
    d[4].ef = d[4].es;
    d[4].float = 480;
    build(&THEME, GOLD, &d, 1.0)
}

#[test]
fn a_critical_bar_stays_the_critical_color_under_a_host_color() {
    assert_eq!(body(&five(), 2), THEME.crit_soft);
}

#[test]
fn a_non_critical_bar_takes_the_host_color_and_an_unpainted_one_does_not() {
    let s = five();
    for (i, c) in body(&s, 1).iter().enumerate().take(3) {
        assert!((c - (0.7 * GOLD[i] + 0.3 * THEME.bg[i])).abs() < 1e-6);
    }
    assert_eq!(body(&s, 4), GOLD, "a milestone is solid");
    assert_eq!(body(&s, 3), THEME.bar_soft);
}

/// A tested color in a theme: (what, body, progress, name ink on the body, name ink on the progress).
type Look = (String, [f32; 4], [f32; 4], [f32; 4], [f32; 4]);

/// Each tested color's part-progress bar in each theme.
fn palette() -> Vec<Look> {
    let hex = |h: u32| [h >> 16, h >> 8 & 255, h & 255, 255].map(|v| v as f32 / 255.0);
    let colors = [
        ("gold", GOLD),
        ("navy", NAVY),
        ("teal", hex(0x2a_9d8f)),
        ("amber", hex(0xd9_7706)),
        ("violet", hex(0x8e_44ad)),
        ("grey", hex(0xa4_a4a4)),
    ];
    let one = [TaskDates {
        float: 480,
        ..dates(1)[0]
    }];
    let mut out = Vec::new();
    for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (what, c) in colors {
            let s = build(&theme, c, &one, 1.0);
            assert_eq!(s.bar_labels[0].placement, Placement::Inside);
            let (quad, l) = (s.rects[s.bar_progress[0].1], &s.bar_labels[0]);
            let edge = quad.pos[0] + quad.size[0];
            assert!(l.x < edge && edge < l.right, "the name crosses the edge");
            out.push((
                format!("{name} {what}"),
                body(&s, 0),
                quad.color,
                l.color,
                l.done.map_or(l.color, |d| d.1),
            ));
        }
    }
    out
}

#[test]
fn a_host_colored_bars_progress_is_darker_in_the_light_theme_and_brighter_in_the_dark_one_by_2_to_1()
 {
    let black = [0.0, 0.0, 0.0, 1.0];
    for (what, body, done, ..) in palette() {
        let darker = ratio(done, black) < ratio(body, black);
        assert_eq!(darker, what.starts_with("light"), "{what}");
        let r = ratio(done, body);
        assert!(r >= 2.0, "{what}: {r:.2}:1");
    }
}

#[test]
fn a_name_inside_a_host_colored_bar_reaches_4_5_on_its_body_and_progress_across_the_edge() {
    for (what, body, done, ink, ink_on_done) in palette() {
        let (b, d) = (ratio(ink, body), ratio(ink_on_done, done));
        assert!(b.min(d) >= 4.5, "{what}: body {b:.2}, progress {d:.2}");
    }
}

/// The one-bar scene of `palette` at `dpr`: its label and where the progress quad ends.
fn split(dpr: f64) -> (crate::labels::BarLabel, f32) {
    let mut one = dates(1);
    one[0].float = 480;
    let s = build(&THEME, [0.165, 0.616, 0.561, 1.0], &one, dpr);
    let quad = s.rects[s.bar_progress[0].1];
    (s.bar_labels[0].clone(), quad.pos[0] + quad.size[0])
}

#[test]
fn a_name_splits_at_the_progress_edge_in_physical_pixels_and_never_into_an_empty_part() {
    let (mut l, edge) = split(2.0);
    let ends: Vec<_> = bar_spans(&l).map(|p| (p.0, p.1)).collect();
    assert_eq!(ends, [(l.x, edge), (edge, l.right)]);
    for edge in [l.x, l.right] {
        l.done = Some((edge, [0.0; 4]));
        assert_eq!(bar_spans(&l).count(), 1, "the edge at {edge}");
    }
}
