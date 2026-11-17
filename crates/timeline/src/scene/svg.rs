//! A scene as an SVG fragment (gs#703): the print plan's hand-off to a reviewer.
//!
//! Coordinates are the scene's physical pixels, unchanged; the caller sets the `viewBox`.

use super::links::TRIANGLE_RIGHT;
use super::{Label, Rect, Scene};
use std::fmt::Write;

/// Write `scene` as one `<g>`: each [`Rect`] in draw order, then each [`Label`].
///
/// A rect is a `<rect>` (radius as `rx`, alpha as `fill-opacity`). A negative radius is a diamond
/// `<polygon>` in the quad; a triangle marker is a three-point `<polygon>`: the tip, then the two
/// base corners. A rect with a `stroke` is a dashed ring of that width just inside the
/// quad's edge. A label is a `<text>` clipped at its `right`; text is escaped.
#[must_use]
pub fn svg(scene: &Scene) -> String {
    let mut out = String::from("<g>");
    for rect in &scene.rects {
        shape(&mut out, rect);
    }
    for (i, label) in scene.labels.iter().enumerate() {
        text(&mut out, i, label);
    }
    out.push_str("</g>");
    out
}

fn hex(color: [f32; 4]) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the component is clamped to 0 to 1, so the byte is 0 to 255"
    )]
    let [r, g, b, _] = color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// ` <attr>="#rrggbb"` and, when not opaque, ` <attr>-opacity="a"`.
fn paint(out: &mut String, attr: &str, color: [f32; 4]) {
    let _ = write!(out, " {attr}=\"{}\"", hex(color));
    if color[3] < 1.0 {
        let _ = write!(out, " {attr}-opacity=\"{}\"", color[3].clamp(0.0, 1.0));
    }
}

fn shape(out: &mut String, rect: &Rect) {
    let ring = rect.stroke > 0.0;
    // A ring runs along the centre of its width, so inset by half to sit inside the edge.
    let inset = if ring { rect.stroke / 2.0 } else { 0.0 };
    let (x, y) = (rect.pos[0] + inset, rect.pos[1] + inset);
    let (w, h) = (
        (rect.size[0] - 2.0 * inset).max(0.0),
        (rect.size[1] - 2.0 * inset).max(0.0),
    );
    let marker = f64::from(rect.radius);
    if marker < TRIANGLE_RIGHT + 0.5 {
        // The tip, then the two corners of its base.
        let (tip, base, cy) = if marker > TRIANGLE_RIGHT - 0.5 {
            (x + w, x, y + h / 2.0)
        } else {
            (x, x + w, y + h / 2.0)
        };
        let _ = write!(
            out,
            "<polygon points=\"{tip},{cy} {base},{y} {base},{}\"",
            y + h
        );
    } else if rect.radius < 0.0 {
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let _ = write!(
            out,
            "<polygon points=\"{cx},{y} {},{cy} {cx},{} {x},{cy}\"",
            x + w,
            y + h
        );
    } else {
        let _ = write!(
            out,
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\""
        );
        if rect.radius > 0.0 {
            // The shader clamps the radius to half the shorter side.
            let rx = rect.radius.min(w.min(h) / 2.0);
            let _ = write!(out, " rx=\"{rx}\"");
        }
    }
    if ring {
        out.push_str(" fill=\"none\"");
        paint(out, "stroke", rect.color);
        let _ = write!(
            out,
            " stroke-width=\"{}\" stroke-dasharray=\"{} {}\"",
            rect.stroke,
            rect.stroke * 2.0,
            rect.stroke * 2.0
        );
    } else {
        paint(out, "fill", rect.color);
    }
    out.push_str("/>");
}

fn text(out: &mut String, index: usize, l: &Label) {
    let _ = write!(
        out,
        "<clipPath id=\"lc{index}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
        l.x,
        l.y,
        (l.right - l.x).max(0.0),
        l.px * 1.5
    );
    let (x, anchor) = if l.center {
        (f32::midpoint(l.x, l.right), " text-anchor=\"middle\"")
    } else {
        (l.x, "")
    };
    let _ = write!(
        out,
        "<text x=\"{x}\" y=\"{}\" font-size=\"{}\"{anchor} dominant-baseline=\"text-before-edge\" clip-path=\"url(#lc{index})\"",
        l.y, l.px
    );
    match l.color {
        Some(color) => paint(out, "fill", color),
        None => out.push_str(" fill=\"currentColor\""),
    }
    out.push('>');
    for c in l.text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out.push_str("</text>");
}
