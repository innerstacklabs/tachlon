//! A frame's draw list, composed in one place (gs#482). `App::frame` in `src/web/` compiles for wasm32
//! only, so a native test cannot reach it; the composition lives here so one can.

use crate::Session;
use timeline::labels::Placement;

impl Session {
    /// This frame's complete draw list: the scene ([`Session::scene`]), the dependency arrows, the
    /// move outlines, the comparison bars, the link overlay, then the bar labels (a proposed task's name sits beside its
    /// ghost ring). `measure(task, text, px)` gives the
    /// width in physical pixels of `text` at `px` physical pixels; the renderer supplies it, and a
    /// test passes a fixed width. This is the one place a frame's draw list is composed.
    pub fn frame_scene(&self, measure: impl FnMut(u32, &str, f32) -> f32) -> timeline::Scene {
        let mut scene = self.scene();
        let timeline = self.timeline();
        let before = scene.rects.len();
        timeline::scene::add_arrows_highlighted(
            &mut scene,
            &timeline.viewport,
            timeline.dpr,
            self.display_dates(),
            self.links(),
            self.revealed_highlight(),
            self.selected_link_drawn(),
            self.theme(),
        );
        // Names go first: the outlines and comparison bars splice in beside the arrows.
        let arrows = scene.bar_start..scene.bar_start + scene.rects.len() - before;
        let rings = self.add_proposal_ghosts(&mut scene);
        timeline::labels::add_bar_labels(
            &mut scene,
            &timeline.viewport,
            timeline.dpr,
            timeline::labels::Rows {
                order: timeline.rows(),
                dates: self.display_dates(),
                summaries: timeline.summaries(),
                highlight: self.revealed_highlight(),
                theme: self.theme(),
                terminal_reach: self.terminal_reach(),
                rings: &rings,
            },
            |task| self.bar_label(task),
            measure,
        );
        // A name sits beside a proposed task's ghost ring, not inside it, unless the ring ends before the name starts.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a name is placed beside a ring on the canvas, far under f32's 2^24 exact range"
        )]
        let px = |css: f64| (css * timeline.dpr).round() as f32;
        let pad = timeline::labels::PAD_PX / 2.0;
        for (task, edge) in rings {
            let Some(l) = scene.bar_labels.iter_mut().find(|l| l.task == task) else {
                continue;
            };
            let mut x = px(edge + pad);
            if l.placement == Placement::Inside {
                // A name inside its bar stays put until the ring's edge is past the name's left.
                if px(edge) <= l.x {
                    continue;
                }
                // Beside the bar as well as the ring, in the ink of a name beside a bar.
                x = x.max(l.right + px(pad));
                (l.placement, l.right) = (Placement::Right, px(timeline.viewport.width));
                l.color[..3].copy_from_slice(&self.theme().text[..3]);
                l.done = None;
            }
            l.x = l.x.max(x);
            // A name steps past a link arrow it would sit on.
            let w = px(f64::from(l.text_w));
            while let Some(a) = scene.rects[arrows.clone()].iter().find(|a| {
                (a.pos[0] < l.x + w && a.pos[0] + a.size[0] > l.x)
                    && (a.pos[1] < l.y + l.px && a.pos[1] + a.size[1] > l.y)
            }) {
                l.x = a.pos[0] + a.size[0] + px(pad);
            }
        }
        self.add_move_outlines(&mut scene);
        self.add_baseline_bars(&mut scene);
        self.add_overlay_bars(&mut scene);
        self.add_link_overlay(&mut scene);
        scene
    }
}
