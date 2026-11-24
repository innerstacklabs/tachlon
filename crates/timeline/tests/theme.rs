#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{Calendar, DAY, Dependency, LinkType, TaskDates, TaskId};
use timeline::labels::{self, Placement, Rows};
use timeline::scene::{self, Highlight, LinkIndex};
use timeline::theme::TOKENS;
use timeline::viewport::Density;
use timeline::{Summaries, Theme, Viewport};

#[test]
fn round_trip() {
    let alpha = Theme::light()
        .with_json(r##"{"hov":"#12345678"}"##)
        .unwrap();
    for theme in [Theme::light(), Theme::dark(), alpha] {
        assert_eq!(Theme::default().with_json(&theme.to_json()), Ok(theme));
    }
}

#[test]
fn partial_override() {
    let original = Theme::dark();
    let changed = original.with_json(r##"{"crit":"#00ff00"}"##).unwrap();
    assert_eq!(changed.crit, [0.0, 1.0, 0.0, 1.0]);
    for key in TOKENS {
        if key != "crit" {
            assert_eq!(changed.get(key), original.get(key), "token {key}");
        }
    }
}

#[test]
fn errors_are_loud() {
    let cases = [
        (r##"{"unknown":"#fff"}"##, Some("unknown")),
        (r##"{"crit":"#12"}"##, Some("crit")),
        (r#"{"crit":"red"}"#, Some("crit")),
        (r##"{"crit":"#gggggg"}"##, Some("crit")),
        (r#"{"crit":12}"#, Some("crit")),
        (r##"{"crit":"#fff","crit":"#000"}"##, Some("crit")),
        (r##"{"crit":"#fff""##, None),
        (r##"{"crit":"#fff"} trailing"##, None),
    ];
    for (json, key) in cases {
        let error = Theme::default().with_json(json).unwrap_err();
        let message = error.to_string();
        if let Some(key) = key {
            assert!(message.contains(key), "{json}: {message}");
        } else {
            assert!(message.contains("byte"), "{json}: {message}");
        }
    }
}

fn dates() -> Vec<TaskDates> {
    vec![
        TaskDates {
            es: 0,
            ef: DAY,
            ls: 0,
            lf: DAY,
            float: 0,
            finished: false,
        },
        TaskDates {
            es: DAY,
            ef: DAY + 60,
            ls: DAY + 60,
            lf: DAY + 120,
            float: 60,
            finished: false,
        },
        TaskDates {
            es: 2 * DAY,
            ef: 3 * DAY,
            ls: 2 * DAY,
            lf: 3 * DAY,
            float: 0,
            finished: false,
        },
    ]
}

fn distinct_theme() -> Theme {
    let mut theme = Theme::default();
    for (index, key) in TOKENS.iter().enumerate() {
        let n = index as f32 + 1.0;
        assert!(theme.set(key, [n / 255.0, (n + 32.0) / 255.0, 0.5, 1.0]));
    }
    theme
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test builds the scene and checks every theme token against what the scene paints, and a split would build the same scene in each part"
)]
fn scene_uses_the_theme() {
    let theme = distinct_theme();
    let dates = dates();
    let order = [0, 1, 2];
    let mut summaries = Summaries::new(dates.len());
    summaries.set(2, Some((2 * DAY, 3 * DAY)));
    let mut viewport = Viewport::new(900.0, 240.0, order.len());
    viewport.fit(0.0, 4.0 * DAY as f64);

    let mut scene = scene::build(
        &viewport,
        1.0,
        &order,
        &dates,
        &summaries,
        Some(0),
        Some((0, false)),
        &theme,
    );
    let has = |color| scene.rects.iter().any(|rect| rect.color == color);
    assert!(has(theme.crit_soft), "critical bar");
    assert!(has(theme.bar_soft), "normal bar");
    assert!(has(theme.sum), "summary bar");
    assert!(has(theme.axis), "axis band");
    assert!(has(theme.stripe), "striped row");
    assert!(has(theme.grid), "minor grid");
    assert!(has(theme.grid_major), "major grid");
    assert!(has(theme.hov), "hovered row");
    assert!(has(theme.text), "drag outline");
    assert!(has(theme.crit_edge), "critical bar edge");

    // Non-working days and the data-date line and tag.
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
        &[],
    )
    .unwrap();
    let backdrop = scene::Backdrop {
        calendar: Some(&calendar),
        data_date: Some(DAY + 8 * 60),
        ..scene::Backdrop::default()
    };
    let mut ten_days = viewport.clone();
    ten_days.fit(0.0, 10.0 * DAY as f64);
    let backdropped = scene::build_marked(
        &ten_days,
        1.0,
        &order,
        &dates,
        &summaries,
        scene::RowMarks::default(),
        None,
        backdrop,
        &theme,
    );
    assert!(
        backdropped
            .rects
            .iter()
            .any(|rect| rect.color == theme.nonwork),
        "non-working day column"
    );
    let x = ten_days.x_of((DAY + 8 * 60) as f64);
    let tier_top = (ten_days.axis_height / 2.0) as f32;
    let line = backdropped
        .rects
        .iter()
        .find(|rect| rect.color == theme.today && rect.size[0] == 1.0)
        .expect("data-date line");
    assert_eq!(line.pos[0], x.round() as f32, "data-date line x");
    assert_eq!(line.pos[1], tier_top, "data-date line starts at the tag");
    // The line is dashed (tl#1215): its last dash ends within one dash and gap of the bottom.
    let last = backdropped
        .rects
        .iter()
        .rfind(|rect| rect.color == theme.today && rect.size[0] == 1.0)
        .expect("data-date line");
    let (dash, gap) = ten_days.today_dash;
    let end = f64::from(last.pos[1] + last.size[1]);
    assert!(
        end <= ten_days.height && end + dash + gap > ten_days.height,
        "data-date line runs to the bottom"
    );
    let tag_box = backdropped
        .rects
        .iter()
        .find(|rect| rect.color == theme.today && rect.size[0] > 1.0)
        .expect("data-date tag box");
    assert!(
        tag_box.pos[1] >= tier_top
            && tag_box.pos[1] + tag_box.size[1] <= ten_days.axis_height as f32,
        "the tag sits on the axis"
    );
    let tag = backdropped
        .labels
        .iter()
        .find(|label| label.text == "Data date")
        .expect("the tag text");
    assert_eq!(tag.color, Some(theme.bar_text), "tag text");
    assert!(
        tag.x >= tag_box.pos[0] && tag.y < ten_days.axis_height as f32,
        "the tag text is on the tag box, in the axis band"
    );

    let held = scene::build(
        &viewport,
        1.0,
        &order,
        &dates,
        &summaries,
        None,
        Some((0, true)),
        &theme,
    );
    assert!(held.rects.iter().any(|rect| rect.color == theme.held));

    let dependency = Dependency {
        pred: TaskId(0),
        succ: TaskId(1),
        kind: LinkType::FS,
        lag: 0,
    };
    let links = LinkIndex::new(&[dependency], |_| true, &order);
    scene::add_arrows(&mut scene, &viewport, 1.0, &dates, &links, &theme);
    assert!(scene.rects.iter().any(|rect| rect.color == theme.link));

    let mut highlighted = scene.clone();
    scene::apply_highlight(
        &mut highlighted,
        &Highlight::new(dates.len(), [0], []),
        &theme,
    );
    assert!(
        highlighted
            .rects
            .iter()
            .any(|rect| rect.color == theme.sel_edge)
    );

    labels::add_bar_labels(
        &mut scene,
        &viewport,
        1.0,
        Rows {
            order: &order,
            dates: &dates,
            summaries: &summaries,
            highlight: None,
            theme: &theme,
            terminal_reach: 11.0,
            rings: &[],
        },
        |task| ["inside", "outside", "summary"][task as usize],
        |task, _, _| if task == 0 { 20.0 } else { 40.0 },
    );
    let inside = scene
        .bar_labels
        .iter()
        .find(|label| label.task == 0)
        .unwrap();
    let outside = scene
        .bar_labels
        .iter()
        .find(|label| label.task == 1)
        .unwrap();
    // Names go right of the bar when there is room, in the theme's text colour (gs#316);
    // the inside colour is covered by the labels unit tests.
    assert_eq!(
        (inside.placement, inside.color),
        (Placement::Right, theme.text)
    );
    assert_eq!(
        (outside.placement, outside.color),
        (Placement::Right, theme.text)
    );

    let light = scene::build(
        &viewport,
        1.0,
        &order,
        &dates,
        &summaries,
        Some(0),
        Some((0, false)),
        &Theme::light(),
    );
    let dark = scene::build(
        &viewport,
        1.0,
        &order,
        &dates,
        &summaries,
        Some(0),
        Some((0, false)),
        &Theme::dark(),
    );
    // The line under the axis band is `grid_major` on the canvas; the table
    // head's bottom border (checked in `css_matches_rust_defaults`) must use
    // the same token so the two halves of the line match.
    for (scene, theme) in [(&light, Theme::light()), (&dark, Theme::dark())] {
        assert!(
            scene.rects.iter().any(|rect| rect.color == theme.grid_major
                && rect.pos[1] == viewport.axis_height as f32 - 1.0
                && rect.size == [viewport.width as f32, 1.0]),
            "axis line is not grid_major"
        );
    }
    assert_ne!(light, dark);
    assert_eq!(light.rects.len(), dark.rects.len());
    // The labels sit in the same places in both themes; the minor tier's text is the `muted` token.
    let placed = |scene: &scene::Scene| {
        scene
            .labels
            .iter()
            .map(|l| (l.text.clone(), l.x, l.y, l.right, l.px, l.center))
            .collect::<Vec<_>>()
    };
    assert_eq!(placed(&light), placed(&dark));
    for (scene, theme) in [(&light, Theme::light()), (&dark, Theme::dark())] {
        assert!(
            scene.labels.iter().any(|l| l.color == Some(theme.muted)),
            "the minor tier is not muted"
        );
    }
    assert_eq!(light.bars, dark.bars);
    assert_eq!(light.summary_bars, dark.summary_bars);
    assert_eq!(light.critical_bars, dark.critical_bars);
    assert_eq!(light.bar_rects, dark.bar_rects);
    assert_eq!(light.bar_start, dark.bar_start);
    for (light, dark) in light.rects.iter().zip(&dark.rects) {
        assert_eq!(light.pos, dark.pos);
        assert_eq!(light.size, dark.size);
    }
}

fn css_block_after<'a>(css: &'a str, marker: &str, after: usize) -> &'a str {
    let marker_at = after
        + css[after..]
            .find(marker)
            .unwrap_or_else(|| panic!("missing CSS marker {marker}"));
    let open = marker_at
        + css[marker_at..]
            .find('{')
            .unwrap_or_else(|| panic!("missing block for {marker}"));
    let close = open
        + css[open + 1..]
            .find('}')
            .unwrap_or_else(|| panic!("unterminated block for {marker}"))
        + 1;
    &css[open + 1..close]
}

fn css_hex(block: &str, key: &str) -> String {
    let declaration = format!("--gantt-{key}:");
    let start = block
        .find(&declaration)
        .unwrap_or_else(|| panic!("missing {declaration}"))
        + declaration.len();
    let value = block[start..].trim_start();
    let len = value
        .char_indices()
        .take_while(|(_, ch)| *ch == '#' || ch.is_ascii_hexdigit())
        .last()
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    value[..len].to_ascii_lowercase()
}

fn theme_hex(theme: &Theme, key: &str) -> String {
    let json = theme.to_json();
    let marker = format!(r#""{key}":""#);
    let start = json.find(&marker).expect("the token is in the JSON") + marker.len();
    let end = start + json[start..].find('"').expect("the value is quoted");
    json[start..end].to_string()
}

#[test]
fn css_matches_rust_defaults() {
    // The token blocks live in packages/gantt/src/styles/tokens.css, linked first on the page.
    let css = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/gantt/src/styles/tokens.css"
    ));
    let light = css_block_after(css, ":where(.gantt)", 0);
    let media = css
        .find("@media (prefers-color-scheme: dark)")
        .expect("missing dark colour-scheme media query");
    let dark = css_block_after(css, ":where(.gantt)", media);
    for key in TOKENS {
        assert_eq!(
            css_hex(light, key),
            theme_hex(&Theme::light(), key),
            "light {key}"
        );
        assert_eq!(
            css_hex(dark, key),
            theme_hex(&Theme::dark(), key),
            "dark {key}"
        );
    }
    // The table head's bottom border continues the canvas's axis line, which
    // the scene draws in `grid_major`; both sides use one token.
    let head = css_block_after(css, ".gantt [data-part=table-head]", 0);
    assert!(
        head.contains("border-bottom: 1px solid var(--gantt-grid-major);"),
        "the table head's border-bottom must be --gantt-grid-major"
    );
}

#[test]
fn css_row_height_matches_viewport_default() {
    // `ROW_HEIGHT` is the default for a host with no DOM table; `--gantt-row-h` is the
    // page's source. They are two literals with one job each, and this keeps them equal.
    let css = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/gantt/src/styles/tokens.css"
    ));
    let value = css
        .lines()
        .find_map(|line| line.trim().strip_prefix("--gantt-row-h:"))
        .expect("tokens.css must declare --gantt-row-h")
        .trim()
        .strip_suffix("px;")
        .expect("--gantt-row-h must be a px length");
    let px: f64 = value.parse().expect("--gantt-row-h must be a number of px");
    assert_eq!(px, timeline::viewport::ROW_HEIGHT);
}

#[test]
fn the_density_docs_state_the_heights_the_engine_holds() {
    let read =
        |path| std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR")));
    let types = read("packages/gantt/src/gantt-types.ts").expect("readable");
    let api = read("docs/WASM_API.md").expect("readable");
    let heights = [Density::Compact, Density::Normal, Density::Roomy].map(Density::row_height);
    let said = format!("{}, {} or {} px", heights[0], heights[1], heights[2]);
    let row = api.lines().find(|l| l.starts_with("| `MountOptions` |"));
    assert!(row.expect("the row").contains(&said), "the row says {said}");
    let comment = types.lines().find(|l| l.contains("How tall the rows are"));
    for (name, px) in ["compact", "normal", "roomy"].iter().zip(heights) {
        assert!(
            comment
                .expect("a doc comment")
                .contains(&format!("`\"{name}\"` ({px} px)"))
        );
    }
}
