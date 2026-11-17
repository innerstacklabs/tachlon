import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// The edge note (gs#309e1): panning into a stretch where no visible row has work shows one note that names where the work
// went; a click brings that work into view; the note takes no focus and a wheel over it still pans. The scan and the jump
// are in crates/app/tests/offscreen.rs; this file is the page: the note, its words, its click and its wheel.

const NOTE = "#offscreen-note";
const WORDS = /^(Work on these rows: (earlier|later), \d{1,2} [A-Z][a-z]{2}|no work \d{1,2} [A-Z][a-z]{2} to \d{1,2} [A-Z][a-z]{2})$/;

/** Pan right a screen at a time, over the canvas, until the app reports an empty stretch. */
async function panIntoTheGap(g: Gantt): Promise<void> {
  const box = await g.box();
  await g.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let step = 0; step < 80; step += 1) {
    if ((await g.status()).offscreen !== null) return;
    await g.page.mouse.wheel(600, 0);
    await g.nextFrame(2);
  }
  throw new Error("no empty stretch after 80 pans");
}

const focused = (page: Page): Promise<string> => page.evaluate(() => document.activeElement?.id ?? "");

test("panning into an empty stretch shows the note, and a click brings work into view", async ({ g }) => {
  await g.open({ n: 2000 });
  await expect(g.page.locator(NOTE), "work is on screen at the start").toBeHidden();
  expect((await g.status()).offscreen).toBeNull();

  await panIntoTheGap(g);
  await g.nextFrame(2);
  await expect(g.page.locator(NOTE)).toBeVisible();
  await expect(g.page.locator(NOTE)).toHaveText(WORDS);
  const rows = (await g.status()).rows;

  await g.page.locator(NOTE).click();
  await g.nextFrame(3);
  const after = await g.status();
  expect(after.offscreen, "the work is in the window").toBeNull();
  expect(after.rows, "the rows stay").toEqual(rows);
  expect(after.selected, "nothing is selected").toBeNull();
  await expect(g.page.locator(NOTE)).toBeHidden();
});

test("the note takes no focus and a wheel over it still pans", async ({ g }) => {
  await g.open({ n: 2000 });
  await g.page.locator("#app").focus();
  await panIntoTheGap(g);
  await g.nextFrame(2);
  const note = g.page.locator(NOTE);
  await expect(note).toBeVisible();
  const box = await note.boundingBox();
  if (box === null) throw new Error("the note has a box");
  const centre = { x: box.x + box.width / 2, y: box.y + box.height / 2 };

  const t0 = (await g.status()).t[0];
  await g.page.mouse.move(centre.x, centre.y);
  await g.page.mouse.wheel(40, 0);
  await g.nextFrame(3);
  expect((await g.status()).t[0], "a wheel over the note panned the chart").toBeGreaterThan(t0);

  const before = await focused(g.page);
  await g.page.mouse.move(centre.x, centre.y);
  await g.page.mouse.down();
  await g.page.mouse.up();
  await g.nextFrame(2);
  expect(await focused(g.page), "a press on the note moves no focus").toBe(before);
});
