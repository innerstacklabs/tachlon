//! Text shaping: the font and the cache of shaped bar-label buffers.
//!
//! Shaping is the expensive step of drawing text. A bar label's shaped
//! buffer depends only on its text and font size, so it is kept across
//! frames, keyed by task: panning and zooming move and clip it but never
//! reshape it. An entry is dropped at the end of the first frame that does
//! not use it, so the cache holds the visible set and nothing more.

use crate::Theme;
use crate::labels::BarLabel;
use crate::scene::{Label, Scene};
use glyphon::cosmic_text::Align;
use glyphon::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer, Viewport as TextViewport, Wrap, fontdb,
};
use std::collections::HashMap;

/// The font for axis and bar labels: Liberation Sans Regular, SIL OFL 1.1
/// (see `assets/LiberationSans-OFL.txt`). The browser has no system fonts.
const FONT: &[u8] = include_bytes!("../assets/LiberationSans-Regular.ttf");
const FONT_FAMILY: &str = "Liberation Sans";
const DEFAULT_LOCALE: &str = "en-US";

/// A font system holding only the bundled font.
pub(crate) fn font_system() -> FontSystem {
    let mut db = fontdb::Database::new();
    db.load_font_data(FONT.to_vec());
    db.set_sans_serif_family(FONT_FAMILY);
    FontSystem::new_with_locale_and_db(DEFAULT_LOCALE.to_string(), db)
}

/// Add font faces, set the shaping locale and clear every shaped-label cache.
pub(crate) fn add_fonts(
    fonts: &mut FontSystem,
    axis: &mut AxisText,
    bars: &mut BarText,
    locale: &str,
    faces: Vec<Vec<u8>>,
) -> usize {
    let mut db = fonts.db().clone();
    let before = db.len();
    for face in faces {
        db.load_font_data(face);
    }
    let added = db.len() - before;
    *fonts = FontSystem::new_with_locale_and_db(locale.to_string(), db);
    axis.clear();
    bars.clear();
    added
}

/// Basic shaping never looks for a glyph in another face, so a chart that was given faces shapes with fallback.
fn shaping(fonts: &FontSystem) -> Shaping {
    if fonts.db().len() > 1 {
        Shaping::Advanced
    } else {
        Shaping::Basic
    }
}

/// Line height for a font of `px`, matching the axis labels.
pub(crate) fn line_height(px: f32) -> f32 {
    (px * 1.2).ceil()
}

struct Entry {
    text: String,
    px: f32,
    buffer: Buffer,
    width: f32,
    seen: u64,
}

/// Shaped buffers of the bar labels of the last frames, by task.
#[derive(Default)]
pub(crate) struct BarText {
    entries: HashMap<u32, Entry>,
    frame: u64,
    shapes: u64,
}

impl BarText {
    fn clear(&mut self) {
        self.entries.clear();
    }

    /// The width in physical pixels of `text` at font size `px`, shaping it
    /// only if task `task` has no buffer for exactly this text and size.
    pub(crate) fn measure(
        &mut self,
        fonts: &mut FontSystem,
        task: u32,
        text: &str,
        px: f32,
    ) -> f32 {
        let frame = self.frame;
        #[expect(
            clippy::float_cmp,
            reason = "a buffer is reused only for exactly the size it was shaped at, so any size that differs in a bit shapes again"
        )]
        let same_size = |e: &Entry| e.px == px;
        if let Some(e) = self.entries.get_mut(&task)
            && same_size(e)
            && e.text == text
        {
            e.seen = frame;
            return e.width;
        }
        let metrics = Metrics::new(px, line_height(px));
        let mut buffer = match self.entries.remove(&task) {
            Some(e) => e.buffer,
            None => Buffer::new(fonts, metrics),
        };
        buffer.set_metrics(metrics);
        buffer.set_wrap(Wrap::None);
        buffer.set_size(None, None);
        buffer.set_text(
            text,
            &Attrs::new().family(Family::SansSerif),
            shaping(fonts),
            None,
        );
        buffer.shape_until_scroll(fonts, false);
        let width = buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0, f32::max);
        self.shapes += 1;
        self.entries.insert(
            task,
            Entry {
                text: text.to_string(),
                px,
                buffer,
                width,
                seen: frame,
            },
        );
        width
    }

    /// The buffer measured for `task` this frame, if any.
    pub(crate) fn buffer(&self, task: u32) -> Option<&Buffer> {
        self.entries
            .get(&task)
            .filter(|e| e.seen == self.frame)
            .map(|e| &e.buffer)
    }

    /// End a frame: forget the tasks it did not measure.
    pub(crate) fn end_frame(&mut self) {
        let frame = self.frame;
        self.entries.retain(|_, e| e.seen == frame);
        self.frame += 1;
    }

    /// How many times a buffer has been shaped since creation.
    #[cfg(test)]
    pub(crate) fn shapes(&self) -> u64 {
        self.shapes
    }
}

/// What an axis label's shaped buffer depends on: its text, its font size and its alignment.
type AxisKey = (String, u32, bool);

/// Shaped buffers of the axis labels of the last frame, in label order.
#[derive(Default)]
pub(crate) struct AxisText {
    pub(crate) used: Vec<(AxisKey, Buffer)>,
    shapes: u64,
}

impl AxisText {
    fn clear(&mut self) {
        self.used.clear();
    }

    /// Lay out `labels`: a label whose text, size and alignment the last frame had keeps that
    /// buffer and is not shaped again; any other takes a spare buffer and is shaped. The buffers
    /// of the last frame that no label takes are dropped, so the cache is the last frame's labels.
    pub(crate) fn shape(&mut self, fonts: &mut FontSystem, labels: &[Label]) {
        let mut spare: HashMap<AxisKey, Vec<Buffer>> = HashMap::new();
        for (key, buf) in self.used.drain(..) {
            spare.entry(key).or_default().push(buf);
        }
        let attrs = Attrs::new().family(Family::SansSerif);
        for l in labels {
            let key = (l.text.clone(), l.px.to_bits(), l.center);
            let kept = spare.get_mut(&key).and_then(Vec::pop);
            let fresh = kept.is_none();
            let mut buf = kept
                .or_else(|| spare.values_mut().find_map(Vec::pop))
                .unwrap_or_else(|| Buffer::new(fonts, Metrics::new(12.0, 15.0)));
            buf.set_metrics(Metrics::new(l.px, line_height(l.px)));
            buf.set_size(Some((l.right - l.x).max(1.0)), Some(l.px * 1.5));
            if fresh {
                let align = l.center.then_some(Align::Center);
                buf.set_text(&l.text, &attrs, shaping(fonts), align);
                self.shapes += 1;
            }
            buf.shape_until_scroll(fonts, false);
            self.used.push((key, buf));
        }
    }

    /// How many labels have been shaped since creation.
    #[cfg(test)]
    pub(crate) fn shapes(&self) -> u64 {
        self.shapes
    }
}

/// The glyphon state the renderer draws labels with.
pub(crate) struct Text {
    pub(crate) fonts: FontSystem,
    pub(crate) swash: SwashCache,
    pub(crate) viewport: TextViewport,
    pub(crate) atlas: TextAtlas,
    pub(crate) renderer: TextRenderer,
    /// Shaped buffers of the axis labels, kept across frames.
    pub(crate) axis: AxisText,
    /// Shaped buffers of the bar labels, kept across frames.
    pub(crate) bars: BarText,
}

fn text_color(c: [f32; 4]) -> Color {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a colour component is 0 to 1, so the byte is 0 to 255"
    )]
    let byte = |v: f32| (v * 255.0).round() as u8;
    Color::rgba(byte(c[0]), byte(c[1]), byte(c[2]), byte(c[3]))
}

/// A bar label's parts as (from x, to x, ink), none empty: the part left of its `done` edge, then the rest.
pub(crate) fn bar_spans(l: &BarLabel) -> impl Iterator<Item = (f32, f32, [f32; 4])> {
    let cut = l.done.map_or(l.x, |d| d.0.clamp(l.x, l.right));
    let ink = l.done.map_or(l.color, |d| d.1);
    let spans = [(l.x, cut, ink), (cut, l.right, l.color)];
    spans.into_iter().filter(|p| p.0 < p.1)
}

impl Text {
    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        (w, h): (u32, u32),
        theme: &Theme,
    ) -> Result<(), String> {
        let t = self;
        t.viewport.update(
            queue,
            Resolution {
                width: w,
                height: h,
            },
        );
        t.axis.shape(&mut t.fonts, &scene.labels);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "l.x, l.y and l.right are physical-pixel positions on the canvas, far inside i32"
        )]
        let areas = t
            .axis
            .used
            .iter()
            .map(|(_, buf)| buf)
            .zip(&scene.labels)
            .map(|(buf, l)| TextArea {
                buffer: buf,
                left: l.x,
                top: l.y,
                scale: 1.0,
                bounds: TextBounds {
                    left: l.x as i32,
                    top: l.y as i32,
                    right: l.right as i32,
                    bottom: (l.y + l.px * 1.5) as i32,
                },
                default_color: text_color(l.color.unwrap_or(theme.text)),
                custom_glyphs: &[],
            });
        let bars = &t.bars;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "l.x, l.y and l.right are physical-pixel positions on the canvas, far inside i32"
        )]
        let bar_areas = scene.bar_labels.iter().flat_map(|l| {
            // The name in `color`, but the part left of its `done` edge in that color's own ink.
            let part = move |(from, to, color): (f32, f32, [f32; 4])| {
                Some(TextArea {
                    buffer: bars.buffer(l.task)?,
                    left: l.x,
                    top: l.y,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: from as i32,
                        top: l.y as i32,
                        right: to as i32,
                        bottom: (l.y + line_height(l.px)) as i32,
                    },
                    default_color: text_color(color),
                    custom_glyphs: &[],
                })
            };
            bar_spans(l).filter_map(part)
        });
        let areas = areas.chain(bar_areas);
        t.renderer
            .prepare(
                device,
                queue,
                &mut t.fonts,
                &mut t.atlas,
                &t.viewport,
                areas,
                &mut t.swash,
            )
            .map_err(|e| format!("text prepare: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(text: &str, x: f32, px: f32, center: bool) -> Label {
        let (right, color) = (x + 60.0, None);
        Label {
            text: text.into(),
            x,
            y: 0.0,
            right,
            px,
            color,
            center,
        }
    }

    #[test]
    fn an_axis_label_is_shaped_again_only_when_its_text_size_or_alignment_changes() {
        let mut fonts = font_system();
        let mut t = AxisText::default();
        let frame = |dx: f32| {
            [
                label("Nov", dx, 12.0, true),
                label("2026", 80.0 + dx, 12.0, false),
            ]
        };
        t.shape(&mut fonts, &frame(0.0));
        t.shape(&mut fonts, &frame(7.0));
        assert_eq!(t.shapes(), 2, "a label that only moves is not shaped again");
        let kept = t.used[0].1.layout_runs().map(|r| r.line_w).sum::<f32>();
        let mut fresh = AxisText::default();
        fresh.shape(&mut fonts, &frame(7.0));
        assert_eq!(
            fresh.used[0].1.layout_runs().map(|r| r.line_w).sum::<f32>(),
            kept
        );
        t.shape(
            &mut fonts,
            &[
                label("Dec", 0.0, 12.0, true),
                label("2026", 80.0, 14.0, false),
            ],
        );
        assert_eq!(t.shapes(), 4, "a new text and a new size each shape once");
        t.shape(&mut fonts, &[label("Dec", 0.0, 12.0, false)]);
        assert_eq!(t.shapes(), 5, "a new alignment shapes");
        assert_eq!(t.used.len(), 1, "the cache is the last frame's labels");
    }

    /// A glyph's x, y, glyph id and cluster (start and end byte).
    type Placed = (f32, f32, u16, usize, usize);

    /// Every glyph of every kept label, per laid-out run: where it sits and which cluster it is.
    fn placed(t: &AxisText) -> Vec<Vec<Placed>> {
        let runs = |buf: &Buffer| {
            buf.layout_runs()
                .flat_map(|run| {
                    let line_y = run.line_y;
                    run.glyphs
                        .iter()
                        .map(move |g| (g.x, line_y + g.y, g.glyph_id, g.start, g.end))
                })
                .collect()
        };
        t.used.iter().map(|(_, buf)| runs(buf)).collect()
    }

    #[test]
    fn a_kept_axis_label_puts_every_letter_where_a_fresh_one_would() {
        let mut fonts = font_system();
        let mut kept = AxisText::default();
        let wide = |l: Label| Label {
            right: l.right + 40.0,
            ..l
        };
        // The first label pans, widens, changes its text and narrows; the second stays put.
        let frame = |first: Label| vec![first, label("2026", 87.0, 12.0, false)];
        let frames = [
            frame(label("Nov", 0.0, 12.0, true)),
            // Only a pan: both buffers are kept.
            frame(label("Nov", 7.0, 12.0, true)),
            // The same text in a wider slot: kept, but the centred letters move.
            frame(wide(label("Nov", 7.0, 12.0, true))),
            // Another text in the slot: a spare buffer is shaped again.
            frame(wide(label("Dec", 7.0, 12.0, true))),
            // The slot narrows again.
            frame(label("Dec", 7.0, 12.0, true)),
        ];
        for (i, labels) in frames.iter().enumerate() {
            kept.shape(&mut fonts, labels);
            let mut fresh = AxisText::default();
            fresh.shape(&mut fonts, labels);
            assert!(placed(&fresh).iter().all(|glyphs| !glyphs.is_empty()));
            assert_eq!(placed(&kept), placed(&fresh), "frame {i}");
        }
        assert!(kept.shapes() <= 4, "the kept buffers were used");
    }

    #[test]
    fn a_label_is_shaped_once_while_it_stays_visible() {
        let mut fonts = font_system();
        let mut t = BarText::default();
        let w = t.measure(&mut fonts, 7, "Pour slab", 11.0);
        assert!(w > 20.0 && w < 100.0, "width {w}");
        t.end_frame();
        for _ in 0..5 {
            assert_eq!(t.measure(&mut fonts, 7, "Pour slab", 11.0), w);
            assert!(t.buffer(7).is_some());
            t.end_frame();
        }
        assert_eq!(t.shapes(), 1);
    }

    #[test]
    fn a_font_added_later_is_used_and_cached_labels_are_reshaped() {
        const TESTBOX: &[u8] = include_bytes!("../assets/TestBox.ttf");
        let (mut fonts, mut axis, mut bars) =
            (font_system(), AxisText::default(), BarText::default());
        let text = "\u{e000}\u{e000}";
        let w0 = bars.measure(&mut fonts, 1, text, 20.0);
        assert!((w0 - 40.0).abs() > 1.0);
        axis.shape(&mut fonts, &[label(text, 0.0, 20.0, false)]);
        assert_eq!(
            add_fonts(
                &mut fonts,
                &mut axis,
                &mut bars,
                "de",
                vec![TESTBOX.to_vec()]
            ),
            1
        );
        assert_eq!(fonts.locale(), "de");
        assert!((bars.measure(&mut fonts, 1, text, 20.0) - 40.0).abs() < 0.01);
        axis.shape(&mut fonts, &[label(text, 0.0, 20.0, false)]);
        let width = axis.used[0]
            .1
            .layout_runs()
            .map(|run| run.line_w)
            .sum::<f32>();
        assert!((width - 40.0).abs() < 0.01);
        assert_eq!(
            add_fonts(
                &mut fonts,
                &mut axis,
                &mut bars,
                "de",
                vec![b"not a font".to_vec()]
            ),
            0
        );
    }

    #[test]
    fn a_new_text_or_size_reshapes_and_an_unused_label_is_dropped() {
        let mut fonts = font_system();
        let mut t = BarText::default();
        let short = t.measure(&mut fonts, 1, "Pour", 11.0);
        t.end_frame();
        let long = t.measure(&mut fonts, 1, "Pour slab and strip forms", 11.0);
        assert!(long > short);
        t.end_frame();
        let big = t.measure(&mut fonts, 1, "Pour slab and strip forms", 22.0);
        assert!(big > long * 1.5);
        assert_eq!(t.shapes(), 3);
        t.end_frame();
        // Task 1 is not measured this frame: it is gone after it.
        t.measure(&mut fonts, 2, "Other", 22.0);
        assert!(t.buffer(1).is_none());
        t.end_frame();
        assert!(
            t.buffer(2).is_none(),
            "a buffer is only served in the frame that measured it"
        );
        t.measure(&mut fonts, 1, "Pour slab and strip forms", 22.0);
        assert_eq!(t.shapes(), 5);
    }
}
