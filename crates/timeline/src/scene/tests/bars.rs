use super::*;
use engine::{DAY, Dependency, LinkType, TaskId};

#[test]
fn only_visible_rows_draw_bars_and_critical_is_coloured() {
    let n = 50_000;
    let d = dates(n);
    let order: Vec<u32> = (0..n as u32).collect();
    let mut v = Viewport::new(1200.0, 800.0, n);
    v.fit(0.0, 60.0 * DAY as f64);
    let s = build(&v, 1.0, &order, &d, &Summaries::default(), None, None);
    let visible = v.visible_rows();
    assert_eq!(visible.start, 0);
    assert_eq!(s.bars, visible.len());
    assert_eq!(s.critical_bars, visible.len().div_ceil(2)); // even task ids have zero float
    let red = s
        .rects
        .iter()
        .filter(|r| r.color == THEME.crit_soft)
        .count();
    let blue = s.rects.iter().filter(|r| r.color == THEME.bar_soft).count();
    assert_eq!((red, blue), (visible.len().div_ceil(2), visible.len() / 2));
}

#[test]
fn a_row_fill_is_selected_then_hovered_then_summary_then_stripe() {
    let n = 8;
    let d = dates(n);
    let order: Vec<u32> = (0..n as u32).collect();
    let mut summaries = Summaries::new(n);
    summaries.set(2, Some((0, DAY)));
    summaries.set(3, Some((0, DAY)));
    summaries.set(4, Some((0, DAY)));
    let mut v = Viewport::new(1200.0, 800.0, n);
    v.fit(0.0, 10.0 * DAY as f64);
    let marks = RowMarks {
        hover: Some(3),
        selected: Some(4),
        range: None,
    };
    let s = build_marked(
        &v,
        1.0,
        &order,
        &d,
        &summaries,
        marks,
        None,
        Backdrop::default(),
        &THEME,
    );
    // The colour of the full-width fill that starts at row `r`'s top.
    let fill = |r: usize| {
        let top = v.row_top(r) as f32;
        s.rects
            .iter()
            .find(|rect| rect.pos == [0.0, top] && rect.size[0] == 1200.0)
            .map(|rect| rect.color)
    };
    assert_eq!(fill(0), None, "an even plain row has no fill");
    assert_eq!(fill(1), Some(THEME.stripe), "an odd plain row is striped");
    assert_eq!(fill(2), Some(THEME.band), "a summary row is a band");
    assert_eq!(fill(3), Some(THEME.hov), "hover beats the band");
    assert_eq!(fill(4), Some(THEME.sel), "selection beats the band");
    let both = RowMarks {
        hover: Some(1),
        selected: Some(1),
        range: None,
    };
    let s = build_marked(
        &v,
        1.0,
        &order,
        &d,
        &summaries,
        both,
        None,
        Backdrop::default(),
        &THEME,
    );
    let top = v.row_top(1) as f32;
    let colors: Vec<_> = s
        .rects
        .iter()
        .filter(|rect| rect.pos == [0.0, top] && rect.size[0] == 1200.0)
        .map(|rect| rect.color)
        .collect();
    assert_eq!(colors, vec![THEME.sel], "selection beats hover on one row");
}

#[test]
fn the_dragged_bar_gets_an_outline_amber_when_held() {
    let d = dates(40);
    let order: Vec<u32> = (0..40).collect();
    let mut v = Viewport::new(1200.0, 800.0, 40);
    v.fit(0.0, 40.0 * DAY as f64);
    let count = |s: &Scene, c| s.rects.iter().filter(|r| r.color == c).count();
    let none = build(&v, 1.0, &order, &d, &Summaries::default(), None, None);
    assert_eq!(count(&none, THEME.text) + count(&none, THEME.held), 0);
    let free = build(
        &v,
        1.0,
        &order,
        &d,
        &Summaries::default(),
        None,
        Some((3, false)),
    );
    assert_eq!((count(&free, THEME.text), count(&free, THEME.held)), (1, 0));
    let held = build(
        &v,
        1.0,
        &order,
        &d,
        &Summaries::default(),
        None,
        Some((3, true)),
    );
    assert_eq!((count(&held, THEME.text), count(&held, THEME.held)), (0, 1));
    assert_eq!(held.bars, none.bars, "the outline is not a bar");
}

#[test]
fn highlight_colours_member_bars_and_dims_the_rest() {
    let d = dates(6);
    let order: Vec<u32> = (0..6).collect();
    let mut v = Viewport::new(800.0, 300.0, 6);
    v.fit(0.0, 6.0 * DAY as f64);
    let mut s = build(&v, 1.0, &order, &d, &Summaries::default(), None, None);
    let before_bars = s.bars;
    let before_rects = s.rects.len();
    let original: Vec<_> = s
        .bar_rects
        .iter()
        .map(|&(_, index)| s.rects[index].color)
        .collect();

    let highlight = Highlight::new(d.len(), [1, 4], []);
    apply_highlight(&mut s, &highlight);

    assert_eq!(s.bars, before_bars);
    assert_eq!(s.rects.len(), before_rects);
    for ((&(task, index), original), expected_member) in s
        .bar_rects
        .iter()
        .zip(original)
        .zip([false, true, false, false, true, false])
    {
        let color = s.rects[index].color;
        assert_eq!(highlight.has_task(task), expected_member);
        if expected_member {
            assert_eq!(color, THEME.sel_edge);
        } else {
            assert_eq!(color[..3], original[..3]);
            assert_eq!(color[3], original[3] * DIM);
        }
    }
}

#[test]
fn bar_rects_skip_the_drag_outline() {
    let d = dates(6);
    let order: Vec<u32> = (0..6).collect();
    let mut v = Viewport::new(800.0, 300.0, 6);
    v.fit(0.0, 6.0 * DAY as f64);
    let s = build(
        &v,
        1.0,
        &order,
        &d,
        &Summaries::default(),
        None,
        Some((3, false)),
    );
    let index = s
        .bar_rects
        .iter()
        .find_map(|&(task, index)| (task == 3).then_some(index))
        .unwrap();

    assert!(s.rects[index].color == THEME.crit_soft || s.rects[index].color == THEME.bar_soft);
    assert_eq!(s.rects[index - 1].color, THEME.text);
}

#[test]
fn bars_left_or_right_of_the_view_are_culled() {
    let d = dates(40);
    let order: Vec<u32> = (0..40).collect();
    let mut v = Viewport::new(1200.0, 800.0, 40);
    v.fit(10.0 * DAY as f64, 12.0 * DAY as f64);
    let s = build(&v, 2.0, &order, &d, &Summaries::default(), None, None);
    // Days 10, 11 and 12 are in the window (with the fit margin).
    assert_eq!(s.bars, 3);
    for r in s
        .rects
        .iter()
        .filter(|r| r.color == THEME.crit_soft || r.color == THEME.bar_soft)
    {
        assert!(r.pos[0] >= -4.0 && r.pos[0] + r.size[0] <= 2404.0 + 1.0);
    }
}

#[test]
fn a_summary_row_draws_a_thin_bracket_with_end_ticks() {
    let d = dates(4);
    let order: Vec<u32> = (0..4).collect();
    let span = (d[2].es, d[3].ef);
    let mut summaries = Summaries::new(d.len());
    summaries.set(1, Some(span));
    let mut v = Viewport::new(800.0, 300.0, order.len());
    v.fit(0.0, 4.0 * DAY as f64);

    let s = build(&v, 1.0, &order, &d, &summaries, None, None);
    assert_eq!(s.summary_bars, 1, "[summary-drawn]");
    assert_eq!(s.bars, 3, "[summary-drawn]");
    // One bar along the top and a tick at each end.
    let summary: Vec<_> = s.rects.iter().filter(|r| r.color == THEME.sum).collect();
    assert_eq!(summary.len(), 3, "[summary-drawn]");
    let (x0, x1) = span_extent(&v, span.0, span.1);
    let bar = s.rects[s.bar_rects.iter().find(|e| e.0 == 1).unwrap().1];
    assert_eq!(bar.pos[0], x0.round() as f32, "[summary-drawn]");
    assert_eq!(
        bar.pos[0] + bar.size[0],
        x1.round() as f32,
        "[summary-drawn]"
    );
    assert_eq!(bar.size[1], BRACKET_BAR as f32, "[bracket]");
    let ticks: Vec<_> = s
        .bar_extras
        .iter()
        .filter(|extra| extra.0 == 1)
        .map(|&(_, index)| s.rects[index])
        .collect();
    assert_eq!(ticks.len(), 2, "[bracket] two end ticks");
    let (left, right) = (ticks[0], ticks[1]);
    assert_eq!(left.pos[0], bar.pos[0], "[bracket] left tick at the start");
    assert_eq!(
        right.pos[0] + right.size[0],
        bar.pos[0] + bar.size[0],
        "[bracket] right tick at the end"
    );
    for tick in [left, right] {
        assert_eq!(tick.size[0], BRACKET_TICK as f32, "[bracket]");
        assert_eq!(
            tick.pos[1],
            bar.pos[1] + bar.size[1],
            "[bracket] hangs below"
        );
        assert_eq!(
            tick.pos[1] + tick.size[1],
            bar.pos[1] + BRACKET_H as f32,
            "[bracket] full height"
        );
    }
    let task = s
        .rects
        .iter()
        .find(|r| r.color == THEME.crit_soft || r.color == THEME.bar_soft)
        .unwrap();
    assert!(bar.size[1] < task.size[1], "[summary-drawn]");
}

#[test]
fn a_bracket_cut_by_the_canvas_has_no_tick_at_the_cut_end() {
    let d = dates(3);
    let order: Vec<u32> = (0..3).collect();
    let mut summaries = Summaries::new(d.len());
    summaries.set(1, Some((0, 10 * DAY)));
    let mut v = Viewport::new(800.0, 300.0, order.len());
    v.fit(0.0, 4.0 * DAY as f64);
    let s = build(&v, 1.0, &order, &d, &summaries, None, None);
    // The span runs off the right edge: only the start tick is drawn.
    let ticks: Vec<_> = s.bar_extras.iter().filter(|e| e.0 == 1).collect();
    assert_eq!(ticks.len(), 1, "[bracket]");
    assert_eq!(
        s.rects[ticks[0].1].pos[0],
        v.x_of(0.0).round() as f32,
        "[bracket]"
    );
}

#[test]
fn a_critical_bar_is_taller_by_its_darker_edge() {
    let n = 2;
    let d = dates(n); // task 0 is critical, task 1 has float
    let order: Vec<u32> = (0..n as u32).collect();
    let mut v = Viewport::new(1200.0, 300.0, n);
    v.fit(0.0, 10.0 * DAY as f64);
    let s = build(&v, 1.0, &order, &d, &Summaries::default(), None, None);
    let bh = bar_height(&v) as f32;
    let fill = s.rects[s.bar_rects.iter().find(|e| e.0 == 0).unwrap().1];
    let plain = s.rects[s.bar_rects.iter().find(|e| e.0 == 1).unwrap().1];
    assert_eq!(plain.color, THEME.bar_soft);
    assert_eq!(plain.size[1], bh, "a normal bar is the standard height");
    assert_eq!(fill.color, THEME.crit_soft);
    assert_eq!(
        fill.size[1], bh,
        "the fill inside the edge is the standard height"
    );
    let (task, index) = s.bar_extras[0];
    assert_eq!(task, 0, "only the critical bar has an edge");
    assert_eq!(s.bar_extras.len(), 1);
    let edge = s.rects[index];
    assert_eq!(edge.color, THEME.crit_edge);
    let edge_w = v.crit_edge as f32;
    assert_eq!(
        edge.size[1],
        bh + 2.0 * edge_w,
        "taller by the edge above and below"
    );
    assert_eq!(edge.pos[1], fill.pos[1] - edge_w, "the edge above");
    assert_eq!(edge.pos[0], fill.pos[0] - edge_w, "the edge left");
    assert_eq!(edge.size[0], fill.size[0] + 2.0 * edge_w, "the edge right");
    assert_ne!(THEME.crit_edge, THEME.crit_soft);
    assert!(
        index < s.bar_rects.iter().find(|e| e.0 == 0).unwrap().1,
        "the edge draws first"
    );
}

#[test]
fn a_critical_sliver_has_no_edge() {
    let d = vec![TaskDates {
        es: 0,
        ef: 60,
        ls: 0,
        lf: 60,
        float: 0,
        finished: false,
    }];
    let mut v = Viewport::new(1200.0, 300.0, 1);
    v.fit(0.0, 700.0 * DAY as f64);
    let s = build(&v, 1.0, &[0], &d, &Summaries::default(), None, None);
    assert_eq!(s.critical_bars, 1);
    assert!(
        s.bar_extras.is_empty(),
        "a bar under 3px draws its fill alone"
    );
    assert_eq!(s.rects[s.bar_rects[0].1].color, THEME.crit_soft);
}

#[test]
fn highlighting_takes_the_edge_and_the_ticks_with_the_bar() {
    let d = dates(4);
    let order: Vec<u32> = (0..4).collect();
    let mut summaries = Summaries::new(d.len());
    summaries.set(3, Some((0, 2 * DAY)));
    let mut v = Viewport::new(800.0, 300.0, 4);
    v.fit(0.0, 4.0 * DAY as f64);
    let mut s = build(&v, 1.0, &order, &d, &summaries, None, None);
    let before = s.rects.clone();
    apply_highlight(&mut s, &Highlight::new(4, [0, 3], []));
    // Task 0 is critical (an edge) and task 3 is a summary (two ticks).
    let of = |task: u32| s.bar_extras.iter().filter(move |e| e.0 == task).count();
    assert_eq!((of(0), of(3)), (1, 2));
    for &(task, index) in &s.bar_extras {
        let want = if task == 0 || task == 3 {
            THEME.sel_edge
        } else {
            let mut dim = before[index].color;
            dim[3] *= DIM;
            dim
        };
        assert_eq!(s.rects[index].color, want, "task {task}");
    }
    let mut s = build(&v, 1.0, &order, &d, &summaries, None, None);
    apply_highlight(&mut s, &Highlight::new(4, [1], []));
    for &(_, index) in &s.bar_extras {
        assert_eq!(s.rects[index].color[3], before[index].color[3] * DIM);
    }
}

#[test]
fn a_summary_bar_is_in_bar_rects_and_dims_with_the_others() {
    let d = dates(3);
    let order: Vec<u32> = (0..3).collect();
    let mut summaries = Summaries::new(d.len());
    summaries.set(1, Some((d[0].es, d[2].ef)));
    let mut v = Viewport::new(800.0, 300.0, order.len());
    v.fit(0.0, 3.0 * DAY as f64);
    let mut s = build(&v, 1.0, &order, &d, &summaries, None, None);
    let summary_entry = s
        .bar_rects
        .iter()
        .find(|&&(task, _)| task == 1)
        .copied()
        .expect("[summary-index] summary entry");
    assert_eq!(s.rects[summary_entry.1].color, THEME.sum, "[summary-index]");

    apply_highlight(&mut s, &Highlight::new(d.len(), [0], []));
    assert_eq!(
        s.rects[summary_entry.1].color[3],
        THEME.sum[3] * DIM,
        "[summary-index]"
    );

    let dependencies = [Dependency {
        pred: TaskId(0),
        succ: TaskId(2),
        kind: LinkType::FS,
        lag: 0,
    }];
    let index = LinkIndex::new(&dependencies, |_| true, &order);
    add_arrows(&mut s, &v, 1.0, &d, &index);
    assert!(s.arrows > 0, "[summary-index]");
    let shifted = s
        .bar_rects
        .iter()
        .find(|&&(task, _)| task == 1)
        .map(|&(_, index)| index)
        .expect("[summary-index] shifted summary entry");
    assert_eq!(
        s.rects[shifted].color[..3],
        THEME.sum[..3],
        "[summary-index]"
    );
    assert_eq!(
        s.rects[shifted].color[3],
        THEME.sum[3] * DIM,
        "[summary-index]"
    );
}

#[test]
fn a_summary_and_its_highlighted_descendants_use_the_accent() {
    let d = dates(4);
    let order: Vec<u32> = (0..4).collect();
    let mut summaries = Summaries::new(d.len());
    summaries.set(0, Some((d[1].es, d[2].ef)));
    let mut v = Viewport::new(800.0, 300.0, order.len());
    v.fit(0.0, 4.0 * DAY as f64);
    let mut s = build(&v, 1.0, &order, &d, &summaries, None, None);
    let unrelated = s
        .bar_rects
        .iter()
        .find(|&&(task, _)| task == 3)
        .map(|&(_, index)| (index, s.rects[index].color))
        .expect("[summary-highlight] unrelated task entry");

    apply_highlight(&mut s, &Highlight::new(d.len(), [0, 1, 2], []));

    for task in [0, 1, 2] {
        let index = s
            .bar_rects
            .iter()
            .find(|&&(candidate, _)| candidate == task)
            .map(|&(_, index)| index)
            .expect("[summary-highlight] highlighted task entry");
        assert_eq!(
            s.rects[index].color, THEME.sel_edge,
            "[summary-highlight] task {task} must use the accent"
        );
    }
    assert_eq!(
        s.rects[unrelated.0].color[..3],
        unrelated.1[..3],
        "[summary-highlight] the unrelated task keeps its colour"
    );
    assert_eq!(
        s.rects[unrelated.0].color[3],
        unrelated.1[3] * DIM,
        "[summary-highlight] the unrelated task dims"
    );
}

#[test]
fn a_summary_bar_is_culled_off_screen() {
    let d = dates(1);
    let mut summaries = Summaries::new(d.len());
    summaries.set(0, Some((100 * DAY, 101 * DAY)));
    let mut v = Viewport::new(800.0, 300.0, 1);
    v.fit(0.0, DAY as f64);

    let s = build(&v, 1.0, &[0], &d, &summaries, None, None);
    assert_eq!(s.summary_bars, 0, "[summary-culled]");
    assert!(
        s.rects.iter().all(|r| r.color != THEME.sum),
        "[summary-culled]"
    );
}

#[test]
fn dpr_scales_to_physical_pixels() {
    let d = dates(1);
    let mut v = Viewport::new(600.0, 300.0, 1);
    v.fit(0.0, 8.0 * 60.0);
    let s1 = build(&v, 1.0, &[0], &d, &Summaries::default(), None, None);
    let s2 = build(&v, 2.0, &[0], &d, &Summaries::default(), None, None);
    let bar = |s: &Scene| *s.rects.iter().find(|r| r.color == THEME.crit_soft).unwrap();
    let (a, b) = (bar(&s1), bar(&s2));
    assert!((b.size[0] - 2.0 * a.size[0]).abs() <= 2.0);
    assert!((b.pos[1] - 2.0 * a.pos[1]).abs() <= 1.0);
}

#[test]
fn a_range_fills_its_rows_with_the_selection_band_and_only_those_in_view() {
    let n = 50_000;
    let d = dates(n);
    let order: Vec<u32> = (0..n as u32).collect();
    let mut v = Viewport::new(1200.0, 800.0, n);
    v.fit(0.0, 10.0 * DAY as f64);
    let visible = v.visible_rows();
    let build_range = |hover, selected, range| {
        let marks = RowMarks {
            hover,
            selected,
            range,
        };
        build_marked(
            &v,
            1.0,
            &order,
            &d,
            &Summaries::default(),
            marks,
            None,
            Backdrop::default(),
            &THEME,
        )
    };
    // The tops of the full-width fills in the selection colour, in draw order.
    let sel_tops = |s: &Scene| -> Vec<f32> {
        s.rects
            .iter()
            .filter(|r| r.color == THEME.sel && r.pos[0] == 0.0 && r.size[0] == 1200.0)
            .map(|r| r.pos[1])
            .collect()
    };
    let top = |r: usize| v.row_top(r) as f32;

    let none = build_range(None, Some(4), None);
    assert_eq!(sel_tops(&none), vec![top(4)], "the selected row alone");

    let three = build_range(None, Some(3), Some((3, 5)));
    assert_eq!(sel_tops(&three), vec![top(3), top(4), top(5)]);

    let hovered = build_range(Some(4), Some(3), Some((3, 5)));
    assert_eq!(
        sel_tops(&hovered),
        vec![top(3), top(4), top(5)],
        "a range row keeps the selection fill under the pointer"
    );

    // A range over every row of a 50,000-row schedule draws the rows in view and no more.
    let all = build_range(None, Some(0), Some((0, n - 1)));
    assert_eq!(sel_tops(&all).len(), visible.len());
    assert!(visible.len() < 100, "the window is a few dozen rows");
    assert_eq!(all.bars, none.bars, "the bars are unchanged");

    // A range entirely below the window draws nothing.
    let below = build_range(None, None, Some((n - 10, n - 1)));
    assert_eq!(sel_tops(&below), []);
}

#[test]
fn a_task_past_its_deadline_has_a_mark_and_others_have_none() {
    let d = dates(5);
    let order: Vec<u32> = (0..5).collect();
    let mut v = Viewport::new(1200.0, 800.0, 5);
    v.fit(0.0, 6.0 * DAY as f64);
    // Task 1 finishes a minute after its deadline; 2 finishes on it; 0 has a later one; 3 has
    // none; 4 is past the slice.
    let deadlines = [Some(d[0].ef + DAY), Some(d[1].ef - 1), Some(d[2].ef), None];
    let draw = |deadlines| {
        let backdrop = Backdrop {
            deadlines,
            ..Backdrop::default()
        };
        let (marks, summaries) = (RowMarks::default(), Summaries::default());
        build_marked(
            &v, 1.0, &order, &d, &summaries, marks, None, backdrop, &THEME,
        )
    };
    let ticks = |s: &Scene| -> Vec<Rect> {
        let crit = s.rects.iter().filter(|r| r.color == THEME.crit);
        crit.copied().collect()
    };
    assert_eq!(ticks(&draw(None)), [], "no deadlines, no mark");
    let s = draw(Some(&deadlines));
    let [mark] = ticks(&s)[..] else {
        panic!("one mark for the one missed deadline");
    };
    let x = v.x_of((d[1].ef - 1) as f64);
    assert_eq!(f64::from(mark.pos[0]), (x - 1.0).round(), "at the deadline");
    let bar = s.rects[s.bar_rects[1].1];
    assert!(mark.size[1] > bar.size[1], "taller than its bar");
    // The other extras are the critical edges of tasks 0, 2 and 4.
    let owner = s.bar_extras.iter().find(|&&(_, i)| s.rects[i] == mark);
    assert_eq!(owner.map(|e| e.0), Some(1), "it follows its task");
    assert_eq!(s.bars, 5, "the mark is not a bar");
}
