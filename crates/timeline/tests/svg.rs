//! A scene as an SVG fragment (gs#703): rects, diamonds, dashed rings and clipped labels.

use timeline::Scene;
use timeline::scene::{Label, Rect, svg};

fn rect(pos: [f32; 2], size: [f32; 2], color: [f32; 4], radius: f32, stroke: f32) -> Rect {
    Rect {
        pos,
        size,
        color,
        radius,
        stroke,
    }
}

fn label(text: &str) -> Label {
    Label {
        text: text.into(),
        x: 10.0,
        y: 4.0,
        right: 90.0,
        px: 12.0,
        color: None,
        center: false,
    }
}

#[test]
fn a_bar_a_diamond_and_a_label_become_a_rect_a_polygon_and_a_text() {
    let scene = Scene {
        rects: vec![
            rect([10.0, 20.0], [100.0, 16.0], [1.0, 0.0, 0.0, 1.0], 3.0, 0.0),
            rect([200.0, 20.0], [16.0, 16.0], [0.0, 0.0, 1.0, 1.0], -1.0, 0.0),
        ],
        labels: vec![label("A & B")],
        ..Scene::default()
    };
    let out = svg(&scene);
    assert!(out.starts_with("<g>") && out.ends_with("</g>"), "{out}");
    assert_eq!(
        out.matches("<rect").count(),
        1 + 1,
        "bar + label clip: {out}"
    );
    assert_eq!(out.matches("<polygon").count(), 1, "{out}");
    assert!(out.contains("rx=\"3\""), "{out}");
    assert!(out.contains("fill=\"#ff0000\""), "{out}");
    assert!(
        out.contains("200,20") || out.contains("208,20"),
        "diamond points: {out}"
    );
    assert!(out.contains("A &amp; B"), "{out}");
    assert!(!out.contains("A & B"), "{out}");
}

#[test]
fn colour_alpha_becomes_fill_opacity() {
    let scene = Scene {
        rects: vec![
            rect([0.0, 0.0], [10.0, 10.0], [0.0, 1.0, 0.0, 0.5], 0.0, 0.0),
            rect([0.0, 0.0], [10.0, 10.0], [0.0, 1.0, 0.0, 1.0], 0.0, 0.0),
        ],
        ..Scene::default()
    };
    let out = svg(&scene);
    assert!(
        out.contains("fill=\"#00ff00\" fill-opacity=\"0.5\""),
        "{out}"
    );
    assert_eq!(
        out.matches("fill-opacity").count(),
        1,
        "opaque has none: {out}"
    );
}

#[test]
fn a_stroke_rect_is_a_dashed_ring_inside_its_edge() {
    let scene = Scene {
        rects: vec![rect(
            [10.0, 10.0],
            [40.0, 20.0],
            [0.0, 0.0, 1.0, 1.0],
            0.0,
            2.0,
        )],
        ..Scene::default()
    };
    let out = svg(&scene);
    assert!(out.contains("fill=\"none\""), "{out}");
    assert!(out.contains("stroke=\"#0000ff\""), "{out}");
    assert!(out.contains("stroke-width=\"2\""), "{out}");
    assert!(out.contains("stroke-dasharray"), "{out}");
    // Inset by half the ring width so the ring sits inside the quad.
    assert!(
        out.contains("x=\"11\"") && out.contains("width=\"38\""),
        "{out}"
    );
}

#[test]
fn a_label_is_clipped_at_right_and_centred_text_anchors_in_the_middle() {
    let mut centred = label("Mar");
    centred.center = true;
    centred.color = Some([1.0, 0.0, 0.0, 1.0]);
    let scene = Scene {
        labels: vec![label("left"), centred],
        ..Scene::default()
    };
    let out = svg(&scene);
    assert_eq!(out.matches("<clipPath").count(), 2, "{out}");
    assert_eq!(out.matches("<text").count(), 2, "{out}");
    assert!(out.contains("width=\"80\""), "clip spans x..right: {out}");
    assert!(out.contains("text-anchor=\"middle\""), "{out}");
    assert!(out.contains("x=\"50\""), "centre of 10..90: {out}");
    assert!(out.contains("fill=\"#ff0000\""), "{out}");
}

#[test]
fn text_is_escaped() {
    let scene = Scene {
        labels: vec![label("<b>\"x\" & 'y'</b>")],
        ..Scene::default()
    };
    let out = svg(&scene);
    assert!(
        out.contains("&lt;b&gt;&quot;x&quot; &amp; &#39;y&#39;&lt;/b&gt;"),
        "{out}"
    );
    assert!(!out.contains("<b>"), "{out}");
}

#[test]
fn svg_writes_a_triangle() {
    let triangle = |radius| {
        let rects = vec![rect([10.0, 20.0], [4.0, 7.0], [0.0; 4], radius, 0.0)];
        svg(&Scene {
            rects,
            ..Scene::default()
        })
    };
    assert!(
        triangle(-2.0).contains("points=\"14,23.5 10,20 10,27\""),
        "tip right"
    );
    assert!(
        triangle(-3.0).contains("points=\"10,23.5 14,20 14,27\""),
        "tip left"
    );
}
