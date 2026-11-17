import type { Page } from "@playwright/test";
import type { PlacedLabel } from "./view";

// Geometry of the bar names as drawn (gs#354). Every check here compares rectangles the last frame drew: the label's own `x`, the
// bar rectangles from the scene (`test_bar_rects`), and the canvas size from the status. None of it reads `placement`, `gap` or the
// rule of labels.rs `place`, so a placement that agrees with that rule but draws a label on its bar still fails.
//
// The one number with no second source is the name's width: the label rectangle is `x .. x + textW * dpr`, `textW` being the width the
// shaper measured for the glyphs the renderer draws, not an input the checks reason from.
//
// Precision: the page hands over `f32` values widened to `f64` exactly, the placement code decides in `f64`, and a label's `x` is
// rounded to a whole physical pixel. So every comparison allows `TOL` physical pixels (the rounding, half a pixel, plus slack for the
// `f32` width), stated once here rather than hidden in each check. The margins checked are 4 CSS px or more, so `TOL` cannot hide an overlap.

/** Margin between a bar and a name beside it, CSS px: half of `PAD_PX` in labels.rs, which pads a name on both sides. */
export const NAME_MARGIN = 4;
/** Slack in physical px: rounding of a label's `x` to a whole pixel (0.5) plus `f32` width error. */
export const TOL = 0.51;

/** A bar rectangle the last frame drew, physical px. */
export interface DrawnBar {
  task: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Bars the last frame drew, from the scene (not recomputed from dates). */
export const drawnBars = (page: Page): Promise<DrawnBar[]> =>
  page.evaluate(() => window.gantt!.app.test_bar_rects() as DrawnBar[]);

/**
 * Violations of the drawn geometry, empty when every label is clear. `surfaceW` is the canvas width in physical px.
 * A Right label starts at least `NAME_MARGIN` past its bar and ends before the next bar on its row or the canvas edge;
 * an Inside label lies within its bar.
 */
export function geometryProblems(labels: PlacedLabel[], bars: DrawnBar[], surfaceW: number, dpr: number): string[] {
  const problems: string[] = [];
  for (const l of labels) {
    const bar = bars.find((b) => b.task === l.task);
    if (!bar) {
      problems.push(`"${l.text}": no bar drawn for task ${l.task}`);
      continue;
    }
    const left = l.x;
    const right = l.x + l.textW * dpr;
    const barRight = bar.x + bar.width;
    if (l.placement === "right") {
      if (left < barRight + NAME_MARGIN * dpr - TOL) {
        problems.push(`"${l.text}": right label starts at ${left}, under its bar's right edge ${barRight} plus ${NAME_MARGIN * dpr}`);
      }
      // The next bar on the row: any other bar overlapping this bar's vertical band and starting at or past its right edge.
      const next = bars
        .filter((b) => b.task !== l.task && b.x >= barRight - TOL && b.y < bar.y + bar.height && b.y + b.height > bar.y)
        .reduce((nearest, b) => Math.min(nearest, b.x), surfaceW);
      if (right > Math.min(next, surfaceW) + TOL) {
        problems.push(`"${l.text}": right label ends at ${right}, past the next bar or canvas edge at ${Math.min(next, surfaceW)}`);
      }
    } else if (left < bar.x - TOL || right > barRight + TOL) {
      problems.push(`"${l.text}": inside label ${left}..${right} leaves its bar ${bar.x}..${barRight}`);
    }
  }
  return problems;
}
