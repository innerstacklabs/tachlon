use super::*;

/// The tasks a stand-in for the app says are held by a start constraint, then by a finish one.
#[derive(Debug)]
struct Held(&'static [u32], &'static [u32]);

impl StartConstraints for Held {
    fn holds_start(&self, task: u32) -> bool {
        self.0.contains(&task)
    }
    fn holds_finish(&self, task: u32) -> bool {
        self.1.contains(&task)
    }
}

/// The scene of five tasks one day apart over `v`, with `held` as the constrained tasks.
fn caps(v: &Viewport, held: Option<&Held>) -> (Scene, Vec<Rect>) {
    let (d, order) = (dates(5), [0, 1, 2, 3, 4]);
    let backdrop = Backdrop {
        start_constraints: held.map(|h| h as &dyn StartConstraints),
        ..Backdrop::default()
    };
    let (marks, summaries) = (RowMarks::default(), Summaries::default());
    let s = build_marked(
        v, 1.0, &order, &d, &summaries, marks, None, backdrop, &THEME,
    );
    let caps = s.rects.iter().filter(|r| r.color == THEME.text);
    let caps = caps.copied().collect();
    (s, caps)
}

#[test]
fn a_held_task_has_a_cap_on_its_left_edge_and_others_have_none() {
    let mut v = Viewport::new(1200.0, 800.0, 5);
    v.fit(0.0, 6.0 * DAY as f64);
    assert_eq!(caps(&v, None).1, [], "no constraints, no cap");
    let (s, found) = caps(&v, Some(&Held(&[1, 3], &[])));
    let [one, three] = found[..] else {
        panic!("one cap for each held task");
    };
    let at = |task: usize| f64::from(s.rects[s.bar_rects[task].1].pos[0]);
    assert_eq!(
        (f64::from(one.pos[0]), f64::from(three.pos[0])),
        (at(1), at(3))
    );
    assert_eq!(one.size[0], 3.0, "3 px wide");
    assert_eq!(
        one.size[1], s.rects[s.bar_rects[1].1].size[1],
        "as tall as its bar"
    );
    let owners = |cap: Rect| s.bar_extras.iter().find(|&&(_, i)| s.rects[i] == cap);
    assert_eq!(owners(one).map(|e| e.0), Some(1), "it follows its task");
    assert_eq!(s.bars, 5, "a cap is not a bar");
}

#[test]
fn a_held_bar_cut_off_on_the_left_or_too_narrow_has_no_cap() {
    let held = Held(&[1, 2], &[]);
    let mut v = Viewport::new(1200.0, 800.0, 5);
    // Task 1 begins before the left edge and ends after it.
    v.fit(DAY as f64 + 240.0, 6.0 * DAY as f64);
    assert!(v.x_of((DAY + 60) as f64) < -1.0 && v.x_of((DAY + 420) as f64) > 0.0);
    assert_eq!(caps(&v, Some(&held)).1.len(), 1, "only task 2 is whole");
    // A year across 1200 px leaves a bar under a pixel.
    v.fit(0.0, 365.0 * DAY as f64);
    assert_eq!(
        caps(&v, Some(&held)).1,
        [],
        "no room for the body beside it"
    );
}

#[test]
fn a_held_finish_has_a_cap_on_its_right_edge_unless_cut_off_or_too_narrow() {
    let mut v = Viewport::new(1200.0, 800.0, 5);
    v.fit(0.0, 6.0 * DAY as f64);
    let (s, found) = caps(&v, Some(&Held(&[], &[1, 3])));
    let [one, three] = found[..] else {
        panic!("one cap for each held finish");
    };
    let right = |task: usize| {
        let bar = s.rects[s.bar_rects[task].1];
        bar.pos[0] + bar.size[0]
    };
    let end = |cap: Rect| cap.pos[0] + cap.size[0];
    assert!((end(one) - right(1)).abs() < 0.01 && (end(three) - right(3)).abs() < 0.01);
    assert_eq!(one.size[0], 3.0, "3 px wide");
    let owners = |cap: Rect| s.bar_extras.iter().find(|&&(_, i)| s.rects[i] == cap);
    assert_eq!(owners(one).map(|e| e.0), Some(1), "it follows its task");
    let n = |v: &Viewport, held: Held| caps(v, Some(&held)).1.len();
    assert_eq!(n(&v, Held(&[2], &[2])), 2, "one on each edge");
    // Task 1 begins inside the right edge and ends after it.
    v.fit(0.0, DAY as f64 + 240.0);
    assert_eq!(n(&v, Held(&[], &[0, 1])), 1, "only task 0 is whole");
    v.fit(0.0, 365.0 * DAY as f64);
    assert_eq!(n(&v, Held(&[], &[1, 2])), 0, "no room for the body");
}
