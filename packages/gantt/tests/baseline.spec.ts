import { expect, openFilters, test } from "./support/gantt";
import { EDITED, fixture, freezeClock, pressAndDrag, rowOf } from "./support/moved-note";
import { AXIS, ROW } from "./support/pixels";

// The baseline (gs#703, story 1): the toolbar's Baseline button keeps the dates as they are, the status line (the `s` key)
// says how many rows it holds, and Ctrl+Z takes it back. These specs check the buttons and the lines; what the baseline
// holds, its summary and its undo steps are checked on `Session` in `crates/app/tests/baseline*.rs`.

const SIZE = 200;
const number = new Intl.NumberFormat("en-US");

test("the button sets it, the status line says when, Ctrl+Z clears it", async ({ page, g }) => {
  await g.open({ n: SIZE });
  const set = page.locator("#baseline-set");
  const clear = page.locator("#baseline-clear");
  const line = page.locator("#status-line");
  const n = (await g.status()).n;
  const says = `Baseline set: ${number.format(n)} rows`;

  await page.keyboard.press("s");
  await expect(line).toBeVisible();
  await expect(set).toHaveAttribute("aria-pressed", "false");
  await expect(clear).toBeHidden();
  await expect(line).not.toContainText("Baseline set");

  await set.click();
  await g.nextFrame(2);
  await expect(set).toHaveAttribute("aria-pressed", "true");
  await expect(set).toHaveAttribute("title", new RegExp(`^${says}`));
  await expect(clear).toBeVisible();
  await expect(line).toContainText(says);

  // Ctrl+Z (the chart has focus again after the click) takes the baseline back; Ctrl+Shift+Z puts it back.
  await page.keyboard.press("Control+z");
  await g.nextFrame(2);
  await expect(set).toHaveAttribute("aria-pressed", "false");
  await expect(clear).toBeHidden();
  await expect(line).not.toContainText("Baseline set");

  await page.keyboard.press("Control+Shift+z");
  await g.nextFrame(2);
  await expect(line).toContainText(says);

  await clear.click();
  await g.nextFrame(2);
  await expect(set).toHaveAttribute("aria-pressed", "false");
  await expect(clear).toBeHidden();
});

test("the toolbar writes the baseline state only when it changes", async ({ page, g }) => {
  await g.open({ n: SIZE });
  // Count the writes the toolbar makes to the Baseline group over many frames, then over one change.
  await page.evaluate(() => {
    const seen = { writes: 0 };
    (window as unknown as { baselineWrites: typeof seen }).baselineWrites = seen;
    const group = document.querySelector('[aria-label="Baseline"]')!;
    new MutationObserver((records) => {
      seen.writes += records.length;
    }).observe(group, { attributes: true, subtree: true });
  });
  await g.nextFrame(30);
  const writes = (): Promise<number> =>
    page.evaluate(async () => {
      await new Promise((done) => setTimeout(done, 0));
      return (window as unknown as { baselineWrites: { writes: number } }).baselineWrites.writes;
    });
  expect(await writes(), "no write across 30 idle frames").toBe(0);

  await page.locator("#baseline-set").click();
  await g.nextFrame(30);
  const afterSet = await writes();
  expect(afterSet, "the change wrote").toBeGreaterThan(0);
  await g.nextFrame(30);
  expect(await writes(), "no write across 30 frames with a baseline set").toBe(afterSet);
});

// The Slipped filter (gs#703): beside Critical only, it keeps the tasks that finish later than the baseline. The button
// shows with a baseline, turns the filter on, and an idle frame writes nothing to it. The rows it keeps are checked on
// `Session` in `crates/app/tests/filter.rs`.
test("the Slipped button turns the filter on and hides with the baseline", async ({ page, g }) => {
  await g.open({ n: SIZE });
  const slipped = page.locator("#slipped-toggle");
  await openFilters(page);
  await expect(slipped, "nothing to press without a baseline").toBeHidden();
  await page.locator("#baseline-set").click();
  await g.nextFrame(2);
  await openFilters(page);
  await expect(slipped).toBeVisible();
  await expect(slipped).toHaveAttribute("aria-pressed", "false");

  await page.evaluate(() => {
    const app = window.gantt!.app;
    const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as [number, string, string, number | null, boolean, number][];
    // The last leaf: little follows it, so only a few tasks slip.
    app.set_duration(rows.filter((r) => r[5] === 0).at(-1)![0], "300d");
  });
  await g.nextFrame(2);

  await openFilters(page);
  await slipped.click();
  await g.nextFrame(2);
  await expect(slipped).toHaveAttribute("aria-pressed", "true");
  expect((await g.status()).filter, "the click reached the app").toBe("slipped");

  // An idle frame writes nothing to the button.
  await page.evaluate(() => {
    const seen = { writes: 0 };
    (window as unknown as { slippedWrites: typeof seen }).slippedWrites = seen;
    new MutationObserver((records) => {
      seen.writes += records.length;
    }).observe(document.querySelector("#slipped-toggle")!, { attributes: true, childList: true, characterData: true, subtree: true });
  });
  await g.nextFrame(30);
  const writes = await page.evaluate(async () => {
    await new Promise((done) => setTimeout(done, 0));
    return (window as unknown as { slippedWrites: { writes: number } }).slippedWrites.writes;
  });
  expect(writes, "no write across 30 idle frames").toBe(0);

  // Clearing the baseline turns the filter off and hides the button.
  await page.locator("#baseline-clear").click();
  await g.nextFrame(2);
  await openFilters(page);
  await expect(slipped).toBeHidden();
  await expect(slipped).toHaveAttribute("aria-pressed", "false");
});

// The status line is written only when its text changed (gs#703, the DOM-write suppression): the loop used to assign the
// whole line every visible frame, which writes its text node even when the text is the same. The spec steps the page's fake
// clock one frame at a time and counts the writes the observer saw in that frame; the frozen clock also keeps the frame
// statistics in the line from moving by themselves.
test("the status line is written only when its text changed", async ({ page, g }) => {
  // WebGL2 from the start: a WebGPU failure reloads the page, and the observer with it.
  await page.route("**/schedule.dat", (route) => route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }));
  await g.open({ params: { schedule: "1", backend: "webgl" }, frames: 5, timeout: 60_000 });
  await g.nextFrame(3);
  await page.keyboard.press("s");
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  await freezeClock(page);
  await page.clock.runFor(4000); // past the 120-frame window of the frame statistics
  await page.evaluate(() => {
    const line = document.querySelector("#status-line")!;
    const seen = { writes: 0 };
    // A text node written in place reports as characterData; a replaced one as childList.
    new MutationObserver((records) => {
      seen.writes += records.length;
    }).observe(line, { childList: true, characterData: true, subtree: true });
    (window as unknown as { lineWrites: typeof seen }).lineWrites = seen;
  });
  // One frame of the fake clock: the writes made in it, and the line's text afterwards.
  const frame = async (): Promise<{ writes: number; text: string }> => {
    await page.clock.runFor(16);
    return page.evaluate(() => {
      const seen = (window as unknown as { lineWrites: { writes: number } }).lineWrites;
      const writes = seen.writes;
      seen.writes = 0;
      return { writes, text: document.querySelector("#status-line")!.textContent ?? "" };
    });
  };
  const run = async (frames: number, before?: (k: number) => Promise<void>): Promise<{ writes: number; changed: number; worst: number }> => {
    let { text } = await frame();
    const out = { writes: 0, changed: 0, worst: 0 };
    for (let k = 0; k < frames; k++) {
      await before?.(k);
      const next = await frame();
      out.writes += next.writes;
      out.changed += next.text === text ? 0 : 1;
      out.worst = Math.max(out.worst, next.writes);
      text = next.text;
    }
    return out;
  };

  // Settle: the frame statistics in the line move for a while after the clock starts. Wait for 45 frames of the same text.
  for (let same = 0, text = ""; same < 45; ) {
    const next = await frame();
    same = next.text === text ? same + 1 : 0;
    text = next.text;
  }
  const idle = await run(30);
  expect(idle.writes, "no write across 30 idle frames").toBe(0);

  // A drag: the line changes as the bar moves; a frame writes it once if its text changed and not at all if it did not.
  const box = await pressAndDrag(g, true);
  expect((await g.status()).drag?.active, "the drag is on").toBe(true);
  const row = await rowOf(page, EDITED);
  const y = box.y + AXIS + row * ROW + ROW / 2;
  const x0 = box.x + (await page.evaluate((r) => window.gantt!.app.x_of(window.gantt!.app.row_start(r)!), row));
  const drag = await run(30, (k) => page.mouse.move(x0 + 40 + 6 * k, y));
  expect(drag.changed, "the drag changed the line").toBeGreaterThan(0);
  expect(drag.worst, "at most one write a frame").toBeLessThanOrEqual(1);
  expect(drag.writes, "one write for each changed text").toBe(drag.changed);
  await page.mouse.up();
  const after = await run(30);
  expect(after.worst).toBeLessThanOrEqual(1);
  expect(after.writes, "one write for each changed text").toBe(after.changed);
});

// "Add new" (gs#703, row 3 of the change-and-re-baseline plan): the button is in the Baseline group and shows only while tasks
// are added since the baseline. What pressing it does to the counts is checked on `Session` in `crates/app/tests/baseline_add.rs`.
test("the Add new button shows only while tasks are added, and pressing it hides it until Ctrl+Z", async ({ page, g }) => {
  await g.open({ n: SIZE });
  const add = page.locator("#baseline-add");
  await expect(add, "no baseline, nothing to add to").toBeHidden();
  await page.locator("#baseline-set").click();
  await g.nextFrame(2);
  await expect(add, "a baseline with nothing added").toBeHidden();
  expect(await add.evaluate((el) => el.closest('[aria-label="Baseline"]') !== null), "it sits in the Baseline group").toBe(true);

  await page.locator("#insert-row").click();
  await g.nextFrame(2);
  await expect(add).toBeVisible();

  // Once it shows, an idle frame writes nothing to the Baseline group.
  await page.evaluate(() => {
    const seen = { writes: 0 };
    (window as unknown as { addWrites: typeof seen }).addWrites = seen;
    new MutationObserver((records) => {
      seen.writes += records.length;
    }).observe(document.querySelector('[aria-label="Baseline"]')!, { attributes: true, childList: true, characterData: true, subtree: true });
  });
  await g.nextFrame(30);
  const writes = (): Promise<number> =>
    page.evaluate(async () => {
      await new Promise((done) => setTimeout(done, 0));
      return (window as unknown as { addWrites: { writes: number } }).addWrites.writes;
    });
  expect(await writes(), "no write across 30 idle frames").toBe(0);

  await add.click();
  await g.nextFrame(2);
  await expect(add).toBeHidden();

  // Ctrl+Z takes it back.
  await page.keyboard.press("Control+z");
  await g.nextFrame(2);
  await expect(add).toBeVisible();
});

// Row 5 of the change-and-re-baseline plan: the tooltip and the status line say the data date the baseline was set at, so
// pressing Baseline again over the same rows (after the data date moved) changes the text and the person sees it took.
test("pressing Baseline again over the same rows changes the tooltip", async ({ page, g }) => {
  await g.open({ n: SIZE });
  const set = page.locator("#baseline-set");
  const line = page.locator("#status-line");
  const n = (await g.status()).n;
  const typeDate = async (day: string): Promise<void> => {
    const box = page.locator("#data-date-input");
    if (!(await box.isVisible())) await page.locator("#data-date").click();
    await box.fill(day);
    await box.press("Enter");
    await g.nextFrame(2);
  };
  await page.keyboard.press("s");

  await typeDate("2026-10-14");
  await set.click();
  await g.nextFrame(2);
  const first = `Baseline set at data date 14 Oct: ${number.format(n)} rows`;
  await expect(set).toHaveAttribute("title", new RegExp(`^${first}`));
  await expect(line).toContainText(first);

  await typeDate("2026-10-21");
  await expect(set, "moving the data date leaves the baseline's text alone").toHaveAttribute("title", new RegExp(`^${first}`));
  await set.click();
  await g.nextFrame(2);
  const second = `Baseline set at data date 21 Oct: ${number.format(n)} rows`;
  await expect(set).toHaveAttribute("title", new RegExp(`^${second}`));
  await expect(line).toContainText(second);
  await expect(line).not.toContainText(first);

  // Undo puts the first snapshot, and its text, back.
  await page.keyboard.press("Control+z");
  await g.nextFrame(2);
  await expect(set).toHaveAttribute("title", new RegExp(`^${first}`));
});
