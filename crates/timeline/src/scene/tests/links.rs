use super::*;
use engine::{DAY, Dependency, LinkType, TaskId};

#[test]
fn highlighted_arrows_use_the_accent_and_the_rest_dim() {
    let d = dates(3);
    let order: Vec<u32> = (0..3).collect();
    let dependencies = [
        Dependency {
            pred: TaskId(0),
            succ: TaskId(1),
            kind: LinkType::FS,
            lag: 0,
        },
        Dependency {
            pred: TaskId(1),
            succ: TaskId(2),
            kind: LinkType::FS,
            lag: 0,
        },
    ];
    let index = LinkIndex::new(&dependencies, |_| true, &order);
    let mut v = Viewport::new(800.0, 300.0, 3);
    v.fit(0.0, 3.0 * DAY as f64);
    let mut s = build(&v, 1.0, &order, &d, &Summaries::default(), None, None);
    let highlight = Highlight::new(d.len(), [], [(0, 1)]);
    add_arrows_highlighted(&mut s, &v, 1.0, &d, &index, Some(&highlight));

    let count = |color| s.rects.iter().filter(|r| r.color == color).count();
    let dim_arrow = [
        THEME.link[0],
        THEME.link[1],
        THEME.link[2],
        THEME.link[3] * DIM,
    ];
    assert!(count(THEME.sel_edge) > 0);
    assert!(count(dim_arrow) > 0);
    assert_eq!(count(THEME.link), 0);
}

#[test]
fn arrows_shift_the_bar_extras_with_the_bars() {
    let d = dates(4);
    let order: Vec<u32> = (0..4).collect();
    let mut summaries = Summaries::new(d.len());
    summaries.set(3, Some((0, 2 * DAY)));
    let mut v = Viewport::new(800.0, 300.0, 4);
    v.fit(0.0, 4.0 * DAY as f64);
    let mut s = build(&v, 1.0, &order, &d, &summaries, None, None);
    let colours: Vec<_> = s
        .bar_extras
        .iter()
        .map(|&(_, index)| s.rects[index].color)
        .collect();
    assert_ne!(colours, Vec::<[f32; 4]>::new());
    let dep = Dependency {
        pred: TaskId(0),
        succ: TaskId(1),
        kind: LinkType::FS,
        lag: 0,
    };
    let links = LinkIndex::new(&[dep], |_| true, &order);
    add_arrows(&mut s, &v, 1.0, &d, &links);
    assert!(s.arrows > 0);
    let after: Vec<_> = s
        .bar_extras
        .iter()
        .map(|&(_, index)| s.rects[index].color)
        .collect();
    assert_eq!(after, colours, "[bar-extras] still the same rectangles");
}

fn one_link_scene(scroll: f64) -> Scene {
    let d = dates(3);
    let order: Vec<u32> = (0..3).collect();
    let dep = Dependency {
        pred: TaskId(0),
        succ: TaskId(1),
        kind: LinkType::FS,
        lag: 0,
    };
    let links = LinkIndex::new(&[dep], |_| true, &order);
    let mut v = Viewport::new(800.0, 300.0, 3);
    v.fit(0.0, 3.0 * DAY as f64);
    v.t0 = d[1].es as f64 - scroll / v.px_per_minute;
    let mut s = build(&v, 1.0, &order, &d, &Summaries::default(), None, None);
    add_arrows(&mut s, &v, 1.0, &d, &links);
    s
}

#[test]
fn an_arrowhead_is_one_triangle() {
    let s = one_link_scene(200.0);
    let heads: Vec<_> = s.rects.iter().filter(|r| r.radius <= -2.0).collect();
    assert_eq!(heads.len(), 1, "one rect carries the triangle marker");
    assert_eq!(heads[0].radius, -2.0, "an FS link points right");
    assert_eq!(heads[0].size, [HEAD as f32, 2.0 * HEAD as f32 - 1.0]);
}

#[test]
fn a_clipped_arrowhead_keeps_its_columns() {
    // The tip sits on the canvas edge, so the quad would be cut and draw a squeezed triangle.
    let s = one_link_scene(0.0);
    assert!(s.rects.iter().all(|r| r.radius > -2.0), "no triangle");
    let arrow: Vec<_> = s.rects.iter().filter(|r| r.color == THEME.link).collect();
    let last = &arrow[arrow.len() - 2..];
    assert_eq!((last[0].size, last[1].size), ([1.0, 1.0], [1.0, 3.0]));
}
