import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import type { SlideStatus } from "../src/api";
import { expect, test, type Gantt } from "./support/gantt";

// The page tells the app whether it asked for no motion (gs#485, story S3.5 of gs#427): `packages/gantt/src/motion.ts` reads
// `prefers-reduced-motion: reduce`, sends it after `App.create` and again on every change, and `status.slide` shows
// the effect. The frame advances the slide (gs#487): the spec edits, draws one frame by hand in the same
// synchronous call (so the page's own frame loop cannot run between them), and reads `status.slide.active`.

// The 43-row ledger fixture of the Rust tests: editing task 16 to a longer duration moves other tasks, and reports an
// edit (a report with `slide` true), which is what starts a slide.
const fixture = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const EDITED = 16;

async function open(page: Page, g: Gantt): Promise<void> {
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }),
  );
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
}

let edits = 0;

/** A duration edit that moves others (each call asks for a different length), then one frame. */
async function editAndStep(page: Page): Promise<{ slide: SlideStatus; others: number; reported: boolean }> {
  edits += 1;
  return page.evaluate(
    ([task, days]) => {
      const gantt = window.gantt!;
      gantt.app.set_duration(task, `${days}d`);
      gantt.app.frame();
      const status = gantt.status();
      return { slide: status.slide, others: status.moved?.others ?? 0, reported: status.moved?.slide === true };
    },
    [EDITED, 10 + (edits % 20)] as const,
  );
}

test("with reduced motion on at the start an edit starts no slide", async ({ page, g }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await open(page, g);
  const idle = await g.status();
  expect(idle.slide, "before any edit").toEqual({ active: false, copyMs: null });
  const after = await editAndStep(page);
  expect(after.others, "the edit moved other tasks").toBeGreaterThan(0);
  expect(after.reported, "the edit made a report that asks for a slide").toBe(true);
  expect(after.slide.active, "reduced motion: no slide").toBe(false);
  expect(after.slide.copyMs, "and nothing was copied").toBeNull();
});

test("with no preference the same edit starts a slide in the next step", async ({ page, g }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await open(page, g);
  const after = await editAndStep(page);
  expect(after.others, "the edit moved other tasks").toBeGreaterThan(0);
  expect(after.slide.active, "a slide runs").toBe(true);
  expect(after.slide.copyMs, "its copy was timed").not.toBeNull();
});

test("a change of the preference reaches the app both ways", async ({ page, g }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await open(page, g);
  expect((await editAndStep(page)).slide.active, "a slide runs before the change").toBe(true);

  // Turning it on reaches the app a moment later (a slide that ran would also end by itself), so edit until one
  // starts no slide, then check the next edit starts none either.
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect
    .poll(async () => (await editAndStep(page)).slide.active, { message: "an edit after the change starts no slide" })
    .toBe(false);
  expect((await editAndStep(page)).slide.active, "and the next one starts none either").toBe(false);

  // Turning it off again lets the next edit slide.
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await expect
    .poll(async () => (await editAndStep(page)).slide.active, { message: "an edit after the change slides again" })
    .toBe(true);
});
