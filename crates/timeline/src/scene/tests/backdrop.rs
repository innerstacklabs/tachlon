use super::*;
use engine::{Calendar, DAY, Instant};

/// Monday to Friday working, as the demo and the importers' fallback do.
fn mon_fri() -> Calendar {
    let day = vec![(480, 1020)];
    Calendar::new(
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
    .unwrap()
}

fn shaded(v: &Viewport, calendar: &Calendar) -> Scene {
    let backdrop = Backdrop {
        calendar: Some(calendar),
        data_date: None,
        ..Backdrop::default()
    };
    let marks = RowMarks::default();
    build_marked(
        v,
        1.0,
        &[],
        &[],
        &Summaries::default(),
        marks,
        None,
        backdrop,
        &THEME,
    )
}

#[test]
fn non_working_days_are_shaded_as_one_column_per_run() {
    let calendar = mon_fri();
    // Four weeks over 1200 px is 42.9 px a day: four Saturday-Sunday columns.
    let mut v = Viewport::new(1200.0, 300.0, 0);
    v.fit(0.0, 28.0 * DAY as f64);
    let s = shaded(&v, &calendar);
    assert_eq!(s.shaded, 4, "[shading]");
    let columns: Vec<_> = s
        .rects
        .iter()
        // Only the rects under the axis band: each run also has a rect through the band's minor tier.
        .filter(|r| r.color == THEME.nonwork && r.pos[1] == v.axis_height as f32)
        .collect();
    assert_eq!(columns.len(), 4);
    for (week, column) in columns.iter().enumerate() {
        let sat = v.x_of(((week * 7 + 5) as i64 * DAY) as f64);
        assert!((f64::from(column.pos[0]) - sat).abs() <= 0.5, "week {week}");
        assert!((f64::from(column.size[0]) - 2.0 * DAY as f64 * v.px_per_minute).abs() <= 1.0);
        assert_eq!(
            column.pos[1], v.axis_height as f32,
            "under the axis band, not in it"
        );
        assert_eq!(column.pos[1] + column.size[1], v.height as f32);
    }
    // Shading draws before the grid and the bars.
    let first = s
        .rects
        .iter()
        .position(|r| r.color == THEME.nonwork)
        .unwrap();
    let grid = s.rects.iter().position(|r| r.color == THEME.grid).unwrap();
    assert!(first < grid && first < s.bar_start);
}

#[test]
fn a_holiday_is_shaded_and_joins_its_weekend() {
    let day = vec![(480, 1020)];
    let calendar = Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[11], // Friday 16 Jan: with the weekend, one three-day run
    )
    .unwrap();
    let mut v = Viewport::new(1200.0, 300.0, 0);
    v.fit(0.0, 14.0 * DAY as f64);
    let s = shaded(&v, &calendar);
    // Sat+Sun of week 1 (days 5-6), then Fri-Sun (days 11-13).
    assert_eq!(s.shaded, 2);
    let wide = s
        .rects
        .iter()
        .filter(|r| r.color == THEME.nonwork)
        .map(|r| r.size[0])
        .fold(0.0, f32::max);
    assert!((f64::from(wide) - 3.0 * DAY as f64 * v.px_per_minute).abs() <= 1.0);
}

#[test]
fn shading_needs_a_day_wider_than_three_pixels_and_is_bounded_by_the_canvas() {
    let calendar = mon_fri();
    // 1200 px over 400 days is 3 px a day: no shading.
    let mut v = Viewport::new(1200.0, 300.0, 0);
    v.fit(0.0, 400.0 * DAY as f64);
    assert!(v.px_per_minute * DAY as f64 <= MIN_SHADE_DAY_PX);
    assert_eq!(shaded(&v, &calendar).shaded, 0, "[shading] zoomed out");
    // Just over 3 px a day: one column per weekend, never more than the
    // canvas holds, however long the schedule behind it.
    v.fit(0.0, 380.0 * DAY as f64);
    let s = shaded(&v, &calendar);
    assert!(v.px_per_minute * DAY as f64 > MIN_SHADE_DAY_PX);
    assert!(
        (54..=56).contains(&s.shaded),
        "[shading] a weekend a week over 380 days, got {}",
        s.shaded
    );
    assert!(s.shaded as f64 <= v.width / MIN_SHADE_DAY_PX / 2.0 + 1.0);
    // Scrolled a long way along a long timeline: the same bound.
    v.t0 = 1_000_000.0 * DAY as f64;
    assert!(shaded(&v, &calendar).shaded as f64 <= v.width / MIN_SHADE_DAY_PX / 2.0 + 1.0);
}

#[test]
fn days_before_the_origin_are_not_shaded() {
    let calendar = mon_fri();
    let mut v = Viewport::new(1200.0, 300.0, 0);
    v.fit(-14.0 * DAY as f64, 14.0 * DAY as f64);
    let s = shaded(&v, &calendar);
    // Only the weekends on and after the origin: days 5-6 and 12-13, 19-20, 26-27 clipped at 14.
    assert!(
        s.rects
            .iter()
            .filter(|r| r.color == THEME.nonwork)
            .all(|r| f64::from(r.pos[0]) >= v.x_of(0.0) - 1.0)
    );
    assert_eq!(s.shaded, 2);
}

#[test]
fn the_data_date_is_a_line_with_a_tag_on_the_axis() {
    let d = dates(3);
    let order: Vec<u32> = (0..3).collect();
    let mut v = Viewport::new(1200.0, 300.0, 3);
    v.fit(0.0, 20.0 * DAY as f64);
    let at = 5 * DAY + 8 * 60;
    let backdrop = Backdrop {
        calendar: None,
        data_date: Some(at),
        ..Backdrop::default()
    };
    let s = build_marked(
        &v,
        1.0,
        &order,
        &d,
        &Summaries::default(),
        RowMarks::default(),
        None,
        backdrop,
        &THEME,
    );
    let x = v.x_of(at as f64);
    assert_eq!(s.data_date_x, Some(x), "[data-date]");
    let line = s
        .rects
        .iter()
        .find(|r| r.color == THEME.today && r.size[0] == 1.0)
        .unwrap();
    assert_eq!(line.pos[0], x.round() as f32);
    assert_eq!(
        line.pos[1],
        (v.axis_height / 2.0) as f32,
        "starts at the tag"
    );
    let last = s
        .rects
        .iter()
        .rfind(|r| r.color == THEME.today && r.size[0] == 1.0)
        .unwrap();
    let (dash, gap) = v.today_dash;
    let end = f64::from(last.pos[1] + last.size[1]);
    assert!(
        end <= v.height && end + dash + gap > v.height,
        "runs to the bottom"
    );
    // The tag is in the minor tier, right of the line, and its text is bar-text.
    let tag = s
        .rects
        .iter()
        .find(|r| r.color == THEME.today && r.size[0] > 1.0)
        .unwrap();
    assert_eq!(tag.pos[0], x.round() as f32);
    assert!(
        tag.pos[1] >= (v.axis_height / 2.0) as f32
            && tag.pos[1] + tag.size[1] <= v.axis_height as f32
    );
    let label = s
        .labels
        .iter()
        .find(|l| l.text == TAG_TEXT)
        .expect("[data-date] tag text");
    assert_eq!(label.color, Some(THEME.bar_text));
    assert!(label.x >= tag.pos[0] && label.right <= tag.pos[0] + tag.size[0]);
    // The line draws before the bars.
    let line_at = s.rects.iter().position(|r| r == line).unwrap();
    assert!(line_at < s.bar_start);
    // No tick label runs under the tag.
    for l in s
        .labels
        .iter()
        .filter(|l| l.text != TAG_TEXT && l.y >= (v.axis_height / 2.0) as f32)
    {
        // A tick label is centred between `x` and `right`; its text is about half an em a letter.
        let half = l.text.chars().count() as f32 * l.px * 0.55 / 2.0;
        let centre = f32::midpoint(l.x, l.right);
        assert!(
            centre - half >= tag.pos[0] + tag.size[0] || centre + half <= tag.pos[0],
            "label {l:?}"
        );
    }
}

#[test]
fn the_data_date_tag_flips_left_near_the_right_edge_and_off_screen_draws_nothing() {
    let d = dates(3);
    let order: Vec<u32> = (0..3).collect();
    let mut v = Viewport::new(1200.0, 300.0, 3);
    v.fit(0.0, 20.0 * DAY as f64);
    let build_at = |at: Instant| {
        let backdrop = Backdrop {
            calendar: None,
            data_date: Some(at),
            ..Backdrop::default()
        };
        build_marked(
            &v,
            1.0,
            &order,
            &d,
            &Summaries::default(),
            RowMarks::default(),
            None,
            backdrop,
            &THEME,
        )
    };
    let near_end = build_at(19 * DAY + 20 * 60);
    let x = near_end.data_date_x.unwrap();
    let tag = near_end
        .rects
        .iter()
        .find(|r| r.color == THEME.today && r.size[0] > 1.0)
        .unwrap();
    assert!(f64::from(tag.pos[0] + tag.size[0]) <= x + 1.5 && tag.pos[0] + tag.size[0] <= 1200.0);
    for at in [-DAY, 21 * DAY] {
        let off = build_at(at);
        assert_eq!(off.data_date_x, None, "[data-date] off screen {at}");
        assert!(off.rects.iter().all(|r| r.color != THEME.today));
        assert!(off.labels.iter().all(|l| l.text != TAG_TEXT));
    }
    let none = build(&v, 1.0, &order, &d, &Summaries::default(), None, None);
    assert_eq!(none.data_date_x, None);
}

/// tl#1215: with colour removed, the data date is a dashed line and a critical bar has a heavier
/// edge than an ordinary one; the dashes do not grow with the task count.
#[test]
fn the_data_date_is_dashed_and_a_critical_edge_is_heavier_without_colour() {
    let mut v = Viewport::new(1200.0, 300.0, 200);
    v.fit(0.0, 10.0 * DAY as f64);
    let (sums, data_date) = (Summaries::default(), Some(5 * DAY));
    let scene = |n: usize| {
        let (d, order) = (dates(n), (0..n as u32).collect::<Vec<_>>());
        let backdrop = Backdrop {
            data_date,
            ..Backdrop::default()
        };
        build_marked(
            &v,
            1.0,
            &order,
            &d,
            &sums,
            RowMarks::default(),
            None,
            backdrop,
            &THEME,
        )
    };
    let s = scene(2); // task 0 is critical, task 1 has float
    let line = |r: &&Rect| r.color == THEME.today && r.size[0] == 1.0;
    let dashes: Vec<_> = s.rects.iter().filter(line).collect();
    assert!(dashes.len() > 1, "[dash] more than one segment");
    for p in dashes.windows(2) {
        assert!(p[0].pos[1] + p[0].size[1] < p[1].pos[1], "[dash] a gap");
    }
    let many = scene(200).rects.iter().filter(line).count();
    assert_eq!(
        dashes.len(),
        many,
        "[dash] bounded by the height, not the tasks"
    );
    let rect =
        |task, list: &[(u32, usize)]| list.iter().find(|e| e.0 == task).map(|e| s.rects[e.1]);
    let fill = rect(0, &s.bar_rects).unwrap();
    let edge = rect(0, &s.bar_extras).expect("[edge] a critical bar has an edge");
    assert!(
        rect(1, &s.bar_extras).is_none(),
        "[edge] an ordinary bar has none"
    );
    let (ends, sides) = (edge.size[0] - fill.size[0], edge.size[1] - fill.size[1]);
    assert!(
        ends >= 4.0 && sides >= 4.0,
        "[edge] 2px or more, not a 1px hairline"
    );
}
