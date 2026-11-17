// The README story: four steps; length in docs/DEMOS.md. Fixed (gs#384, gs#392, gs#508, gs#393). A PR that adds a user-visible feature adds its step to
// steps-features.mjs, not here (docs/DEMOS.md). The video opens on the critical path with a drag, not on the fast scroll. All of it runs on the
// 50,000-task schedule; the P6 steps (open-p6, code-filter, edit-link, show-moved) live in steps-features.mjs.

import { close, criticalView, drag, fling, open } from "./steps/story.mjs";
import { openingView } from "./steps/opening.mjs";

// The pre-roll (record/browser.mjs, preRoll): runs once after the frame-rate check, before the cut. The opening view sets the
// zoom and reads the project length; the critical view then puts the critical path on the chart for the first frame.
export async function setup(ctx) {
  await openingView(ctx);
  await criticalView(ctx);
}

export const steps = [open, drag, fling, close];
