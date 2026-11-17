//! The color a host gives a task's bar, and what is drawn from it.

use super::Scene;
use crate::Theme;
use std::fmt::Debug;

/// The share of the host color in a bar's body, the rest being the background. It has no floor
/// for the tested palette (`tests/bar_color.rs`), which passes at every share from 0.05 to 1.0
/// (measured); 0.7 is the chosen value, a look.
const BODY_SHARE: f32 = 0.7;

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0; 4];

/// The app's answer to which color a task's bar takes (`Backdrop::bar_colors`). It is asked once
/// per drawn bar, so a frame costs the rows in view.
pub trait BarColors: Debug {
    /// The color of the bar of `task` (sRGB, straight alpha), or `None` for the theme's. A
    /// critical bar ignores it.
    fn bar_color(&self, task: u32) -> Option<[f32; 4]>;
}

/// The pastel body of a bar of host color `c`: `c` mixed with the background.
pub(super) fn soft(c: [f32; 4], bg: [f32; 4]) -> [f32; 4] {
    let mix = |i: usize| BODY_SHARE * c[i] + (1.0 - BODY_SHARE) * bg[i];
    [mix(0), mix(1), mix(2), c[3]]
}

/// The ink for a name on body `body`: black or white, whichever reads better on it.
fn ink_on(body: [f32; 4]) -> [f32; 4] {
    if contrast(BLACK, body) >= contrast(WHITE, body) {
        BLACK
    } else {
        WHITE
    }
}

/// The progress quad of a bar of host color `c` on body `body`: `c` shaded in hundredths, toward
/// black in a light theme and toward white in a dark one, to the first shade at 2:1 on the body,
/// so the progress always reads as more filled than the body.
pub(super) fn progress(c: [f32; 4], body: [f32; 4], theme: &Theme) -> [f32; 4] {
    let end = if luminance(theme.bg) > 0.5 {
        BLACK
    } else {
        WHITE
    };
    (0..=100u8)
        .map(|i| {
            let t = f32::from(i) / 100.0;
            let mix = |k: usize| (1.0 - t) * c[k] + t * end[k];
            [mix(0), mix(1), mix(2), c[3]]
        })
        .find(|p| contrast(*p, body) >= 2.0)
        .unwrap_or(end)
}

/// Where the quad of `task`'s progress ends (physical x) and the ink for the part of a name left of
/// it, when the body's ink (`ink`) does not reach 4.5:1 on the quad, so the name reads on both.
pub(crate) fn done_ink(scene: &Scene, task: u32, theme: &Theme) -> Option<(f32, [f32; 4])> {
    let &(_, c) = scene.bar_colored.iter().find(|e| e.0 == task)?;
    let &(_, i) = scene.bar_progress.iter().find(|e| e.0 == task)?;
    let body = soft(c, theme.bg);
    let done = progress(c, body, theme);
    let quad = scene.rects[i];
    (contrast(ink_on(body), done) < 4.5).then(|| (quad.pos[0] + quad.size[0], ink_on(done)))
}

/// The ink for a name inside `task`'s bar: the theme's, or, for a bar the scene drew in a host
/// color (`Scene::bar_colored`), its body's ink.
pub(crate) fn ink(scene: &Scene, task: u32, theme: &Theme) -> [f32; 4] {
    scene
        .bar_colored
        .iter()
        .find(|e| e.0 == task)
        .map_or(theme.bar_ink, |&(_, c)| ink_on(soft(c, theme.bg)))
}

fn luminance(c: [f32; 4]) -> f32 {
    let lin = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

fn contrast(a: [f32; 4], b: [f32; 4]) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
