import type { Page } from "@playwright/test";
import { expect, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { backend, dpr, n, url } from "./support/knobs";
import { AXIS } from "./support/pixels";

// The hover-card, bar-drag, undo and redo journey from verify.mjs (removed in 3f191ea6) (gs#97 P7). One 50k page is shared by the
// file per ruling devteam-1-90, and each test starts from the state the previous left. beforeAll sets the day level with
// the first critical task a fifth of the canvas width across.

import { ROW } from "./support/pixels";

const nextFrame = (page: Page, k = 2): Promise<void> =>
  page.evaluate(
    (frameCount) =>
      new Promise<void>((done) => {
        const target = window.gantt!.status().frames + frameCount;
        const wait = (): void => (window.gantt!.status().frames >= target ? done() : void requestAnimationFrame(wait));
        requestAnimationFrame(wait);
      }),
    k,
  );

const status = (page: Page) => page.evaluate(() => window.gantt!.status());

test.describe.serial("hover and drag journey", () => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  let page: Page;
  let logs: string[];
  let box: { x: number; y: number; width: number; height: number };
  let row: number;
  let dragX: number;
  let dragY: number;
  let text0: string | null;
  let rev0: number;
  let es0: number;
  let hoverTask: number;
  let text1: string | null;
  let rev1: number;
  let es1: number;

  const rowText = () =>
    page.evaluate((r) => document.querySelector(`#table-rows .row[data-row="${r}"]`)?.textContent ?? null, row);
  const card = () =>
    page.evaluate(() => {
      const c = document.getElementById("hover-card")!;
      return { hidden: c.hidden, task: c.dataset.task ?? null, text: c.textContent ?? "" };
    });

  async function rowStart(): Promise<number> {
    const es = await page.evaluate((r) => window.gantt!.app.row_start(r), row);
    if (es === undefined) throw new Error(`row ${row} has no early start`);
    return es;
  }

  test.beforeAll(async ({ browser }) => {
    page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dpr });
    logs = trackLogs(page);
    const query = new URLSearchParams({ n: String(n) });
    if (backend === "webgl") query.set("backend", "webgl");
    await page.goto(`${url}?${query}`);
    await waitReady(page, { frames: 30, timeout: 180_000 });
    const bounds = await page.locator("#timeline").boundingBox();
    if (!bounds) throw new Error("#timeline has no bounding box");
    box = bounds;
    // Zoom in to the day level first (the page starts at the year level), then press c: it scrolls to the first critical task
    // and pans so that task's start is a fifth of the canvas width across, as verify.mjs reached this section.
    const px = box.x + box.width / 2;
    const py = box.y + AXIS + 10 * ROW;
    let steps = 0;
    while ((await status(page)).level !== "day" && steps < 200) {
      await page.mouse.move(px, py);
      await page.keyboard.down("Control");
      await page.mouse.wheel(0, -120);
      await page.keyboard.up("Control");
      await nextFrame(page);
      steps++;
    }
    const level = (await status(page)).level;
    expect(level).toBe("day");
    await page.keyboard.press("c");
    await nextFrame(page, 3);
    row = (await status(page)).rows[0];
    dragX = box.x + box.width * 0.2 + 4; // the bar's start is a fifth of the way across after c
    dragY = box.y + AXIS + ROW / 2;
  });

  test.afterAll(async ({}, testInfo) => {
    await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
    const errors = pageErrors(logs);
    expect.soft(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`).toBe(true);
    await page.close();
  });

  test("the hover card shows the hovered bar's name, start and float", async () => {
    await page.mouse.move(dragX, dragY);
    const onBar = (await status(page)).hover !== null;
    expect.soft(onBar, `after c, a bar is under the pointer on row ${row}`).toBe(true);
    if (!onBar) throw new Error(`after c, no bar is under the pointer on row ${row}`);
    await nextFrame(page, 3);
    hoverTask = (await status(page)).hover!.task;
    const details = await page.evaluate((t) => window.gantt!.app.task_details(t), hoverTask);
    const fullName = await page.evaluate((t) => window.gantt!.app.task_name(t), hoverTask);
    const hoveredCard = await card();
    expect.soft(!hoveredCard.hidden, "the hover card is visible over a bar").toBe(true);
    expect.soft(hoveredCard.task === String(hoverTask), `the hover card identifies hovered task ${hoverTask}`).toBe(true);
    expect.soft(hoveredCard.text.includes(fullName), `the hover card shows the hovered bar's name ${JSON.stringify(fullName)}`).toBe(true);
    expect.soft(hoveredCard.text.includes(details.start), `the hover card shows the task start ${details.start}`).toBe(true);
    expect.soft(hoveredCard.text.includes(details.floatText), `the hover card shows the task float ${JSON.stringify(details.floatText)}`).toBe(true);
    const fonts = await page.evaluate(() => ({
      card: getComputedStyle(document.getElementById("hover-card")!).fontFamily,
      chart: getComputedStyle(document.getElementById("app")!).fontFamily,
      token: getComputedStyle(document.getElementById("app")!).getPropertyValue("--gantt-font").trim(),
    }));
    expect.soft(fonts.card, "the hover card uses --gantt-font, not the page's monospace body font").toBe(fonts.chart);
    expect.soft(fonts.card, "the hover card's font family is the --gantt-font token").toBe(fonts.token);
  });

  test("a bar drag hides the hover card, and the drag recomputes the schedule and the table row", async () => {
    text0 = await rowText();
    rev0 = (await status(page)).revision;
    es0 = await rowStart();
    await page.mouse.down();
    // Just past the drag threshold the pointer is still on the bar, so a hidden card here is the drag's doing, not the
    // pointer leaving the bar.
    await page.mouse.move(dragX + 4, dragY);
    await nextFrame(page, 3);
    const early = await status(page);
    expect.soft(early.drag?.active === true && early.hover?.task === hoverTask, "a drag has started and the pointer is still on the same bar").toBe(true);
    expect.soft((await card()).hidden, "the hover card is hidden as soon as a drag starts, with the pointer still on the bar").toBe(true);
    for (let k = 1; k <= 8; k++) await page.mouse.move(dragX + 15 * k, dragY);
    await nextFrame(page, 3);
    const during = await status(page);
    expect.soft((await card()).hidden, "the hover card is hidden while a bar drag is active").toBe(true);
    await page.mouse.up();
    await nextFrame(page, 3);
    text1 = await rowText();
    rev1 = (await status(page)).revision;
    es1 = await rowStart();
    expect.soft(during.drag?.active === true, "a bar drag is in progress while the pointer is down").toBe(true);
    expect.soft(rev1 > rev0, `the drag recomputed the schedule (revision ${rev0} to ${rev1})`).toBe(true);
    expect.soft(text0 !== null && text1 !== null && text0 !== text1, `the table row shows the dragged task's new dates (${text0} -> ${text1})`).toBe(true);
  });

  test("Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y undo and redo the drag", async () => {
    await page.keyboard.press("Control+Z");
    await nextFrame(page, 2);
    const undo1 = { text: await rowText(), revision: (await status(page)).revision, es: await rowStart() };
    expect.soft(undo1.text === text0, `Ctrl+Z restored the table row (${undo1.text}, want ${text0})`).toBe(true);
    expect.soft(undo1.revision !== rev1, `Ctrl+Z made an edit at revision ${undo1.revision} (drag revision was ${rev1})`).toBe(true);
    expect.soft(undo1.es === es0 && undo1.es !== es1, `Ctrl+Z restored row ${row} start ${undo1.es} (before ${es0}, dragged ${es1})`).toBe(true);

    await page.keyboard.press("Control+Shift+Z");
    await nextFrame(page, 2);
    const shiftRedo = { text: await rowText(), es: await rowStart() };
    expect.soft(shiftRedo.es === es1, `Ctrl+Shift+Z restored row ${row} start ${shiftRedo.es} (want ${es1})`).toBe(true);
    expect.soft(shiftRedo.text === text1, `Ctrl+Shift+Z restored the dragged table row (${shiftRedo.text}, want ${text1})`).toBe(true);
    await page.keyboard.press("Control+Z");
    await nextFrame(page, 2);
    await page.keyboard.press("Control+Y");
    await nextFrame(page, 2);
    const yRedo = { text: await rowText(), es: await rowStart() };
    expect.soft(yRedo.es === es1, `Ctrl+Y restored row ${row} start ${yRedo.es} (want ${es1})`).toBe(true);
    expect.soft(yRedo.text === text1, `Ctrl+Y restored the dragged table row (${yRedo.text}, want ${text1})`).toBe(true);
  });

  test("Ctrl+Z during a drag changes nothing until release, then undoes the drag", async () => {
    const drag2X = box.x + (await page.evaluate((es) => window.gantt!.app.x_of(es), await rowStart())) + 4;
    await page.mouse.move(drag2X, dragY);
    await page.mouse.down();
    for (let k = 1; k <= 4; k++) await page.mouse.move(drag2X + 20 * k, dragY);
    await nextFrame(page, 2);
    const drag2BeforeUndo = await status(page);
    const drag2Es = await rowStart();
    await page.keyboard.press("Control+Z");
    await nextFrame(page, 2);
    const drag2AfterUndo = await status(page);
    expect
      .soft(
        drag2AfterUndo.revision === drag2BeforeUndo.revision,
        `Ctrl+Z during a drag left revision ${drag2AfterUndo.revision} unchanged (was ${drag2BeforeUndo.revision})`,
      )
      .toBe(true);
    expect.soft(drag2AfterUndo.drag?.active === true, `Ctrl+Z during a drag left it active (${JSON.stringify(drag2AfterUndo.drag)})`).toBe(true);
    expect.soft(drag2Es !== es1, `the second drag visibly moved row ${row} start from ${es1} to ${drag2Es}`).toBe(true);
    await page.mouse.up();
    await nextFrame(page, 2);
    await page.keyboard.press("Control+Z");
    await nextFrame(page, 2);
    const drag2Undone = { text: await rowText(), es: await rowStart() };
    expect.soft(drag2Undone.es === es1, `Ctrl+Z after release restored row ${row} start ${drag2Undone.es} (want ${es1})`).toBe(true);
    expect.soft(drag2Undone.text === text1, `Ctrl+Z after release restored the dragged table row (${drag2Undone.text}, want ${text1})`).toBe(true);
  });

  test("the hover card is hidden after the pointer leaves the bar", async () => {
    await page.mouse.move(dragX, box.y + AXIS / 2);
    await nextFrame(page, 3);
    expect.soft((await card()).hidden, "the hover card is hidden after the pointer leaves the bar").toBe(true);
  });

  test("a drag at a reused revision still redraws the table, and Ctrl+Z ends the block at the dragged state", async () => {
    // A restore, then a drag whose revision number is one already used. The revision goes back to its press value on a
    // restore, so the drag after it reaches a number the table has drawn; the table keys on the stamp, which only goes up,
    // so it must still redraw. The restore and the second drag run in one task, so no animation frame draws the table between.
    const y = dragY - box.y;
    const x = (await page.evaluate((es) => window.gantt!.app.x_of(es), await rowStart())) + 4;
    const revBefore = (await status(page)).revision;
    await page.evaluate(
      ([px, py]) => {
        window.gantt!.app.pointer_down(px, py, false);
        window.gantt!.app.pointer_move(px + 100, py);
      },
      [x, y],
    );
    await nextFrame(page, 3);
    const out = await status(page);
    const textOut = await rowText();
    expect.soft(out.revision === revBefore + 1, `a second drag moved the schedule (revision ${revBefore} to ${out.revision})`).toBe(true);
    expect.soft(textOut !== null && textOut !== text1, `the table drew the second drag's dates (${text1} -> ${textOut})`).toBe(true);
    const back = await page.evaluate(
      ([px, py]) => {
        const g = window.gantt!;
        g.app.pointer_move(px, py);
        g.app.frame();
        const restored = g.status().revision;
        g.app.pointer_move(px + 250, py);
        g.app.frame();
        const again = g.status().revision;
        g.app.pointer_up();
        return { restored, again };
      },
      [x, y],
    );
    expect.soft(back.restored === revBefore, `the restore put the revision back (${back.restored}, want ${revBefore})`).toBe(true);
    expect.soft(back.again === out.revision, `the next drag reached revision ${back.again}, one already used (${out.revision})`).toBe(true);
    await nextFrame(page, 3);
    const textAgain = await rowText();
    const truth = await page.evaluate((r) => JSON.parse(window.gantt!.app.table_rows(r, r + 1))[0] as [number, string, string], row);
    expect
      .soft(
        textAgain !== null && textAgain.includes(truth[1]) && textAgain.includes(truth[2]) && textAgain !== textOut && textAgain !== text1,
        `after a restore the table redraws at a reused revision (${textOut} -> ${textAgain})`,
      )
      .toBe(true);
    await page.keyboard.press("Control+Z");
    await nextFrame(page, 2);
    const final = { text: await rowText(), es: await rowStart() };
    expect.soft(final.es === es1 && final.text === text1, `the drag block ends at start ${final.es} and row ${final.text} (want ${es1}, ${text1})`).toBe(true);
  });
});
