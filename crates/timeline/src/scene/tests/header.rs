//! The axis band (gs#315): two tiers, day letters under weeks, a pill for the data date.

use super::*;
use crate::scene::header::Sizes;

fn band(width: f64, days: f64, rows: usize, dpr: f64, axis_height: f64) -> (Scene, Viewport) {
    band_from(0.0, width, days, rows, dpr, axis_height)
}

/// [`band`] showing `days` days from day `from`.
fn band_from(
    from: f64,
    width: f64,
    days: f64,
    rows: usize,
    dpr: f64,
    axis_height: f64,
) -> (Scene, Viewport) {
    let mut v = Viewport::new(width, 400.0, rows);
    v.axis_height = axis_height;
    v.fit(from * DAY as f64, (from + days) * DAY as f64);
    let d = dates(rows);
    let order: Vec<u32> = (0..rows as u32).collect();
    let backdrop = Backdrop {
        data_date: Some(5 * DAY + 8 * 60),
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

/// Labels of the top and the bottom tier, the data-date pill's text left out.
fn tiers(s: &Scene, v: &Viewport) -> (Vec<Label>, Vec<Label>) {
    let half = (v.axis_height / 2.0) as f32;
    s.labels
        .iter()
        .filter(|l| l.text != TAG_TEXT)
        .cloned()
        .partition(|l| l.y < half)
}

#[test]
fn the_band_is_forty_eight_pixels_of_two_tiers() {
    let (s, v) = band(1200.0, 40.0, 3, 1.0, crate::viewport::AXIS_HEIGHT);
    assert_eq!(v.axis_height, 48.0, "[axis] two tiers of 24 px");
    let (top, bottom) = tiers(&s, &v);
    assert!(
        !top.is_empty() && !bottom.is_empty(),
        "[axis] both tiers have labels"
    );
    let sizes = Sizes::of(v.axis_height);
    assert!(
        top.iter().all(|l| l.px == sizes.major_px as f32),
        "major tier"
    );
    assert!(
        bottom.iter().all(|l| l.px == sizes.minor_px as f32),
        "minor tier"
    );
    assert_eq!((sizes.major_px, sizes.minor_px), (12.0, 11.0));
    for l in top.iter().chain(&bottom) {
        assert!(
            l.y >= 0.0 && l.y + l.px * 1.2 <= 48.0 + 0.5,
            "{l:?} fits the band"
        );
    }
    // The minor tier's text is muted, the major tier's is the chart's text.
    assert!(top.iter().all(|l| l.color.is_none()));
    // A period names itself centred, except the one cut by the left edge, which keeps its name at the edge.
    assert!(
        top.iter().filter(|l| !l.center).all(|l| l.x < 8.0),
        "{top:?}"
    );
    assert!(
        bottom
            .iter()
            .all(|l| l.color == Some(THEME.muted) && l.center)
    );
}

#[test]
fn weeks_zoom_is_week_names_over_day_letters_with_a_line_per_day() {
    // 60 days over 1200 px: 20 px a day, the mockup's weeks zoom.
    let (s, v) = band(1200.0, 60.0, 3, 1.0, 48.0);
    let (top, bottom) = tiers(&s, &v);
    assert!(
        top.iter().any(|l| l.text == "12 Jan 2026"),
        "[axis] a week is named by its Monday: {top:?}"
    );
    assert!(
        top.iter().filter(|l| !l.center).all(|l| l.x < 8.0),
        "{top:?}"
    );
    let letters: String = bottom.iter().map(|l| l.text.as_str()).collect();
    assert!(
        letters.starts_with("MTWTFSS") || letters.contains("MTWTFSSMTWTFSS"),
        "[axis] {letters}"
    );
    assert!(bottom.iter().all(|l| l.text.chars().count() == 1));
    // A faint line down the body at every day, a stronger one at every week.
    // The day lines start under the top tier, the week lines at the top of the band.
    let lines = |color: [f32; 4], top: f32| {
        s.rects
            .iter()
            .filter(|r| r.color == color && r.size[0] == 1.0 && r.pos[1] == top)
            .count()
    };
    let (days, weeks) = (lines(THEME.grid, 24.0), lines(THEME.grid_major, 0.0));
    let visible = v.width / (v.px_per_minute * DAY as f64);
    assert!(
        (days as f64 - visible).abs() <= 1.5,
        "[day lines] a line a day: {days} lines for {visible:.1} days"
    );
    assert!(
        (8..=10).contains(&weeks),
        "[day lines] a line a week, got {weeks}"
    );
}

#[test]
fn further_out_it_is_month_names_over_weeks() {
    // 120 days over 1200 px: 10 px a day.
    let (s, v) = band(1200.0, 120.0, 3, 1.0, 48.0);
    let (top, bottom) = tiers(&s, &v);
    assert!(top.iter().any(|l| l.text == "Feb 2026"), "[axis] {top:?}");
    assert!(
        bottom.iter().any(|l| l.text == "Jan 26"),
        "[axis] {bottom:?}"
    );
    assert!(bottom.iter().all(|l| l.text.chars().count() > 1));
}

#[test]
fn closer_in_a_day_gets_three_letters_then_weekday_and_date() {
    let (s, v) = band(1200.0, 30.0, 3, 1.0, 48.0);
    let (_, bottom) = tiers(&s, &v);
    assert!(
        bottom.iter().any(|l| l.text == "Tue"),
        "[axis] 40 px a day: {bottom:?}"
    );
    let (s, v) = band(1200.0, 14.0, 3, 1.0, 48.0);
    let (_, bottom) = tiers(&s, &v);
    assert!(
        bottom.iter().any(|l| l.text == "Tue 13"),
        "[axis] 86 px a day: {bottom:?}"
    );
}

#[test]
fn the_lines_and_labels_follow_the_canvas_width_never_the_task_count() {
    let count = |rows: usize, width: f64| {
        let (s, v) = band(width, 60.0, rows, 1.0, 48.0);
        let lines = s
            .rects
            .iter()
            .filter(|r| (r.color == THEME.grid || r.color == THEME.grid_major) && r.size[0] == 1.0)
            .count();
        (lines, s.labels.len(), v.visible_rows().len())
    };
    let (few, many) = (count(3, 1200.0), count(40_000, 1200.0));
    assert_eq!(
        (few.0, few.1),
        (many.0, many.1),
        "[day lines] same lines and labels for 3 and 40000 tasks"
    );
    assert!(many.2 > few.2, "the second schedule really has more rows");
    // Twice the width at the same scale: twice the days, so twice the lines (give or take the
    // one line a column edge adds or drops).
    let lines = |width: f64, days: f64| {
        let (s, _) = band(width, days, 3, 1.0, 48.0);
        s.rects
            .iter()
            .filter(|r| (r.color == THEME.grid || r.color == THEME.grid_major) && r.size[0] == 1.0)
            .count()
    };
    let (narrow, wide) = (lines(1200.0, 60.0), lines(2400.0, 120.0));
    assert!(narrow > 20, "[day lines] {narrow} lines on the base canvas");
    assert!(
        wide.abs_diff(2 * narrow) <= 2,
        "[day lines] {wide} lines on a canvas twice as wide, {narrow} on the base"
    );
}

#[test]
fn the_data_date_tag_is_a_pill_in_the_minor_tier() {
    let (s, v) = band(1200.0, 40.0, 3, 1.0, 48.0);
    let sizes = Sizes::of(v.axis_height);
    let pill = s
        .rects
        .iter()
        .find(|r| r.color == THEME.today && r.size[0] > 1.0)
        .expect("[pill] the data-date tag");
    assert_eq!(pill.size[1], sizes.tag_h as f32, "[pill] 18 px tall");
    assert_eq!(
        pill.radius,
        (sizes.tag_h / 2.0) as f32,
        "[pill] a full round end"
    );
    assert!(
        pill.pos[1] >= 24.0 && pill.pos[1] + pill.size[1] <= 48.0,
        "[pill] inside the minor tier"
    );
    let text = s
        .labels
        .iter()
        .find(|l| l.text == TAG_TEXT)
        .expect("[pill] its text");
    assert!(text.center && text.x >= pill.pos[0] && text.right <= pill.pos[0] + pill.size[0]);
    assert!(text.y >= pill.pos[1] && text.y + text.px * 1.2 <= pill.pos[1] + pill.size[1] + 0.5);
}

#[test]
fn every_size_follows_the_band_height_and_the_pixel_ratio() {
    // A host that sets another `--gantt-axis-h` gets the same proportions.
    for axis in [32.0, 48.0, 64.0] {
        let (s, v) = band(1200.0, 40.0, 3, 1.0, axis);
        let sizes = Sizes::of(axis);
        assert_eq!(sizes.tier, axis / 2.0);
        let (top, bottom) = tiers(&s, &v);
        assert!(top.iter().all(|l| l.px == sizes.major_px as f32), "{axis}");
        assert!(
            bottom.iter().all(|l| l.px == sizes.minor_px as f32),
            "{axis}"
        );
        for l in &s.labels {
            assert!(
                l.y >= 0.0 && l.y + l.px * 1.2 <= axis as f32 + 0.5,
                "{axis}: {l:?}"
            );
        }
    }
    let (s, _) = band(1200.0, 40.0, 3, 2.0, 48.0);
    let sizes = Sizes::of(48.0);
    assert!(s.labels.iter().all(|l| {
        l.px == 2.0 * sizes.major_px as f32
            || l.px == 2.0 * sizes.minor_px as f32
            || l.px == 2.0 * sizes.tag_px as f32
    }));
}

#[test]
fn the_label_padding_follows_the_band_height() {
    // A period only partly in view keeps its name `pad` clear of the canvas edge. Slide the view
    // across month ends until one name is cut down to a sliver: its left edge is the padding.
    for axis in [32.0, 48.0, 64.0] {
        let pad = Sizes::of(axis).pad;
        assert_eq!(pad, axis / 12.0, "{axis}: a sixth of a tier");
        let mut edges = Vec::new();
        for step in 0..240 {
            let mut v = Viewport::new(1200.0, 300.0, 1);
            v.axis_height = axis;
            let from = step as f64 * 6.0 * 60.0;
            v.fit(from, from + 40.0 * DAY as f64);
            let s = build_marked(
                &v,
                1.0,
                &[0],
                &dates(1),
                &Summaries::default(),
                RowMarks::default(),
                None,
                Backdrop::default(),
                &THEME,
            );
            edges.extend(tiers(&s, &v).0.iter().map(|l| l.x));
        }
        // Labels sit on whole physical pixels.
        assert!(edges.contains(&(pad.round() as f32)), "{axis}: pad {pad}");
    }
    assert_eq!(Sizes::of(48.0).pad, 4.0, "the default band keeps its 4 px");
}

/// gs#150: the axis labels follow the schedule's origin. A Monday 2024-01-01 origin reads
/// 2024 and January; the demo origin reads the same view as 2026.
#[test]
fn the_axis_labels_follow_the_schedule_origin() {
    let texts = |origin: axis::Origin| -> Vec<String> {
        let mut v = Viewport::new(1200.0, 300.0, 0);
        v.fit(0.0, 40.0 * DAY as f64);
        let backdrop = Backdrop {
            origin,
            ..Backdrop::default()
        };
        build_marked(
            &v,
            1.0,
            &[],
            &[],
            &Summaries::default(),
            RowMarks::default(),
            None,
            backdrop,
            &THEME,
        )
        .labels
        .into_iter()
        .map(|l| l.text)
        .collect()
    };
    let in_2024 = texts(axis::Origin::from_day(19_723));
    assert!(
        in_2024.iter().any(|t| t.ends_with("2024")),
        "[axis] a major label names 2024: {in_2024:?}"
    );
    assert!(
        in_2024.iter().all(|t| !t.contains("2026")),
        "[axis] no 2026 label: {in_2024:?}"
    );
    let demo = texts(axis::Origin::DEMO);
    assert!(demo.iter().any(|t| t.ends_with("2026")), "{demo:?}");
}

#[test]
fn labels_stay_inside_the_canvas() {
    let d = dates(1);
    let mut v = Viewport::new(800.0, 300.0, 1);
    for span in [
        120.0,
        3.0 * DAY as f64,
        90.0 * DAY as f64,
        4.0 * 365.0 * DAY as f64,
    ] {
        v.fit(1000.0, 1000.0 + span);
        let backdrop = Backdrop {
            data_date: Some(1000 + (span / 2.0) as i64),
            ..Backdrop::default()
        };
        let s = build_marked(
            &v,
            1.0,
            &[0],
            &d,
            &Summaries::default(),
            RowMarks::default(),
            None,
            backdrop,
            &THEME,
        );
        assert!(!s.labels.is_empty(), "span {span}");
        for l in &s.labels {
            // A tick label is centred on the part of its column in view, so it ends on the
            // canvas. A name set at the edge is clipped at the canvas.
            let end = if l.center {
                f32::midpoint(l.x, l.right)
            } else {
                l.right
            };
            assert!(l.x >= 0.0 && l.x < 800.0 && end <= 800.0, "{l:?}");
        }
    }
}

#[test]
fn a_partial_period_keeps_its_name_at_the_visible_left_edge() {
    // 120 days over 1200 px starting on day 10: January is cut off on the left, yet its name
    // fits in what shows.
    let (s, v) = band_from(10.0, 1200.0, 120.0, 3, 1.0, 48.0);
    let (top, _) = tiers(&s, &v);
    let jan = top
        .iter()
        .find(|l| l.text == "Jan 2026")
        .expect("[edge] Jan");
    assert!(
        !jan.center,
        "[edge] a cut-off period is not centred: {jan:?}"
    );
    assert!(
        jan.x >= 0.0 && jan.x < 10.0,
        "[edge] at the left edge: {jan:?}"
    );
    let feb = top
        .iter()
        .find(|l| l.text == "Feb 2026")
        .expect("[edge] Feb");
    assert!(feb.center, "[edge] a whole period stays centred: {feb:?}");
}

/// gs#320: a schedule a couple of years wide shows three year columns, the first and the last cut
/// by the canvas edges. Each is named on the part in view; none is dropped for being cut.
#[test]
fn a_column_cut_by_either_edge_is_named_on_the_part_in_view() {
    let mut v = Viewport::new(608.0, 300.0, 1);
    let from = 150.0 * DAY as f64;
    v.fit(from, from + 767.0 * DAY as f64);
    let s = build_marked(
        &v,
        1.0,
        &[0],
        &dates(1),
        &Summaries::default(),
        RowMarks::default(),
        None,
        Backdrop::default(),
        &THEME,
    );
    let (top, bottom) = tiers(&s, &v);
    assert!(top.is_empty(), "years have no coarser tier: {top:?}");
    let texts: Vec<&str> = bottom.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["2026", "2027", "2028"], "{bottom:?}");
    // The cut columns are centred on what shows of them, inside the canvas.
    assert!(
        bottom
            .iter()
            .all(|l| l.center && l.x >= 0.0 && l.right <= 608.0)
    );
    assert!(bottom[0].x < 40.0 && bottom[2].right > 570.0, "{bottom:?}");
}

#[test]
fn a_minor_label_under_the_data_date_pill_is_kept() {
    // 130 days over 640 px. Find the second column's label with no data date, then put the date
    // in the first quarter of that column, so the pill covers the column's centre.
    let d = dates(1);
    let mut v = Viewport::new(640.0, 300.0, 1);
    v.fit(0.0, 130.0 * DAY as f64);
    let run = |data_date: Option<i64>| {
        let backdrop = Backdrop {
            data_date,
            ..Backdrop::default()
        };
        let s = build_marked(
            &v,
            1.0,
            &[0],
            &d,
            &Summaries::default(),
            RowMarks::default(),
            None,
            backdrop,
            &THEME,
        );
        tiers(&s, &v).1
    };
    let plain = run(None);
    let second = &plain[1];
    let col = f32::midpoint(plain[2].x, plain[2].right) - f32::midpoint(second.x, second.right);
    let left = f32::midpoint(second.x, second.right) - col / 2.0;
    let pill_x = left + col / 4.0;
    let minutes = (pill_x as f64 - v.x_of(0.0)) / v.px_per_minute;
    let with = run(Some(minutes as i64));
    let kept = with
        .iter()
        .find(|l| l.text == second.text)
        .unwrap_or_else(|| panic!("[pill] {} is kept: {with:?}", second.text));
    let sizes = Sizes::of(v.axis_height);
    let pill = (pill_x, pill_x + sizes.tag_w as f32);
    let (a, b) = if kept.center {
        let w = kept.right - kept.x;
        let c = f32::midpoint(kept.x, kept.right);
        (c - w / 2.0, c + w / 2.0)
    } else {
        (kept.x, kept.right)
    };
    let pad = sizes.pad as f32;
    assert!(
        a >= pill.1 + pad - 0.5 || b <= pill.0 - pad + 0.5,
        "[pill] {kept:?} clears the pill {pill:?}"
    );
    assert_eq!(with.len(), plain.len(), "[pill] one label per tick");
}

#[test]
fn weekend_shades_the_minor_tier() {
    let day = vec![(480, 1020)];
    let calendar = engine::Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .unwrap();
    let mut v = Viewport::new(1200.0, 400.0, 0);
    v.fit(0.0, 28.0 * DAY as f64);
    let backdrop = Backdrop {
        calendar: Some(&calendar),
        ..Backdrop::default()
    };
    let s = build_marked(
        &v,
        1.0,
        &[],
        &[],
        &Summaries::default(),
        RowMarks::default(),
        None,
        backdrop,
        &THEME,
    );
    let tier = (v.axis_height / 2.0) as f32;
    let (x0, x1) = (
        v.x_of(5.0 * DAY as f64) as f32,
        v.x_of(6.0 * DAY as f64) as f32,
    );
    let in_tier: Vec<_> = s
        .rects
        .iter()
        .filter(|r| r.color == THEME.nonwork && r.pos[1] == tier)
        .collect();
    assert!(
        in_tier.iter().any(|r| r.pos[0] <= x0
            && r.pos[0] + r.size[0] >= x1
            && r.pos[1] + r.size[1] == v.axis_height as f32),
        "[axis] a Saturday is shaded from the minor tier's top to the band's bottom: {in_tier:?}"
    );
    assert_eq!(
        in_tier.len(),
        4,
        "[axis] one per weekend, none in the major tier"
    );
    // The tier rects come after the band, or the band would paint over them.
    let band = s.rects.iter().position(|r| r.color == THEME.axis).unwrap();
    let first = s
        .rects
        .iter()
        .position(|r| r.pos[1] == tier && r.color == THEME.nonwork)
        .unwrap();
    assert!(band < first);
}

#[test]
fn no_minor_tier_shade_under_the_day_cutoff() {
    let day = vec![(480, 1020)];
    let calendar = engine::Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .unwrap();
    let tier_rects = |days: f64| {
        let mut v = Viewport::new(1200.0, 400.0, 0);
        v.fit(0.0, days * DAY as f64);
        let backdrop = Backdrop {
            calendar: Some(&calendar),
            ..Backdrop::default()
        };
        let s = build_marked(
            &v,
            1.0,
            &[],
            &[],
            &Summaries::default(),
            RowMarks::default(),
            None,
            backdrop,
            &THEME,
        );
        let tier = (v.axis_height / 2.0) as f32;
        let n = s
            .rects
            .iter()
            .filter(|r| r.color == THEME.nonwork && r.pos[1] == tier)
            .count();
        (v.px_per_minute * DAY as f64, n, s.shaded)
    };
    // Just over the cutoff: one tier rect per body column.
    let (px, tier, body) = tier_rects(380.0);
    assert!(px > MIN_SHADE_DAY_PX && body > 0);
    assert_eq!(tier, body, "[axis] a tier rect for every shaded run");
    // At the cutoff and under it: nothing in the body and nothing in the tier.
    for days in [400.0, 1000.0] {
        let (px, tier, body) = tier_rects(days);
        assert!(px <= MIN_SHADE_DAY_PX);
        assert_eq!((tier, body), (0, 0), "[axis] {days} days: no shading");
    }
}
