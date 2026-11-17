//! The legend in the footer of each printed page (gs#703).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use common::outline_session;
use timeline::Theme;

const ENTRIES: [(&str, &str); 11] = [
    ("Task", "bar"),
    ("Critical", "crit"),
    ("Deadline", "crit"),
    ("Start constraint", "text"),
    ("Finish constraint", "text"),
    ("Summary", "sum"),
    ("Dependency", "link"),
    ("Data date", "today"),
    ("Non-working day", "nonwork"),
    ("Proposal ghost", "sel-edge"),
    ("Just-moved outline", "sel-edge"),
];

fn hex(color: [f32; 4]) -> String {
    let [r, g, b, _] = color.map(|c| (c * 255.0).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// The `fill` of each legend swatch in `svg`, in order.
fn swatch_fills(svg: &str) -> Vec<String> {
    svg.split("<rect class=\"legend-swatch\"")
        .skip(1)
        .map(|rest| {
            let tag = rest.split("/>").next().unwrap();
            let fill = tag.split("fill=\"").nth(1).unwrap();
            fill.split('"').next().unwrap().to_string()
        })
        .collect()
}

#[test]
fn the_legend_swatches_are_the_light_theme_colours_of_each_entry() {
    let theme = Theme::light();
    let want: Vec<String> = ENTRIES
        .iter()
        .map(|(_, token)| hex(theme.get(token).unwrap()))
        .collect();
    for svg in outline_session(60).0.print_pages("t", "") {
        assert_eq!(swatch_fills(&svg), want);
    }
}

#[test]
fn the_non_working_swatch_keeps_its_alpha_and_every_swatch_has_the_rule_outline() {
    let theme = Theme::light();
    let outline = format!("stroke=\"{}\"", hex(theme.rule));
    for svg in outline_session(60).0.print_pages("t", "") {
        let tags: Vec<&str> = svg
            .split("<rect class=\"legend-swatch\"")
            .skip(1)
            .map(|rest| rest.split("/>").next().unwrap())
            .collect();
        assert_eq!(tags.len(), ENTRIES.len());
        for (tag, (label, token)) in tags.iter().zip(ENTRIES) {
            assert!(tag.contains(&outline), "{label} has no outline");
            let alpha = theme.get(token).unwrap()[3];
            if alpha < 1.0 {
                assert!(
                    tag.contains(&format!("fill-opacity=\"{alpha}\"")),
                    "{label} lost its alpha"
                );
            } else {
                assert!(!tag.contains("fill-opacity"), "{label} is opaque");
            }
        }
        assert!(tags[8].contains("fill-opacity"));
    }
}

#[test]
fn every_page_labels_each_legend_entry() {
    let pages = outline_session(60).0.print_pages("t", "");
    assert!(pages.len() > 1);
    for svg in pages {
        for (label, _) in ENTRIES {
            assert!(svg.contains(&format!(">{label}</text>")), "{label}");
        }
    }
}

#[test]
fn the_last_legend_label_ends_inside_the_page_margin() {
    // The legend writes its labels 5.6 px a character; the page keeps a 36 px margin.
    let (last, _) = ENTRIES[ENTRIES.len() - 1];
    for svg in outline_session(60).0.print_pages("t", "") {
        let width = svg.split("viewBox=\"0 0 ").nth(1).unwrap();
        let width: f64 = width.split(' ').next().unwrap().parse().unwrap();
        let before = svg.split(&format!(">{last}</text>")).next().unwrap();
        let x = before.rsplit("<text x=\"").next().unwrap();
        let x: f64 = x.split('"').next().unwrap().parse().unwrap();
        let end = x + last.len() as f64 * 5.6;
        assert!(end <= width - 36.0, "{end} of {width}");
    }
}
