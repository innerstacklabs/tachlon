// The page's slide as the edit phase reads it (gs#486, story S3.6 of gs#427). Part of the
// harness in `../measure.ts`. Every function here reads a `Status`
// with no `slide` as "no slide, not active, no copy time".

/** What the page reports of the slide of the bars to their new dates. */
export interface SlideStatus {
  /** True while the bars are sliding. */
  active: boolean;
  /** The time the copy of the old and new dates took for the last edit, ms; null if it has not run. */
  copyMs: number | null;
}

/** The longest the phase waits for the undo's slide to end before it fails, ms. */
export const SLIDE_WAIT_MS = 1000;

/** `status.slide`, or null when the page has no slide (or does not yet report one). */
export function slideOf(status: object): SlideStatus | null {
  const slide = (status as { slide?: SlideStatus | null }).slide;
  return slide === undefined || slide === null ? null : slide;
}

/**
 * Yield one frame at a time until the slide is not active; a missing slide is not active, so
 * this returns at once. Throws if it is still active after `limitMs`, so the phase cannot hang.
 */
export function* waitSlideIdle(
  read: () => SlideStatus | null,
  now: () => number = () => performance.now(),
  limitMs: number = SLIDE_WAIT_MS,
): Generator<void> {
  const end = now() + limitMs;
  while (read()?.active === true) {
    if (now() >= end) throw new Error(`the slide was still active ${limitMs} ms after the undo`);
    yield;
  }
}
