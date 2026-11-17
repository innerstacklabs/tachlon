import type { Page } from "@playwright/test";
import { expect, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { backend, dpr, n, url } from "./support/knobs";

// The table-editing journey that verify.mjs (removed in 3f191ea6) used to run (gs#97 P5): the finishing task's name and duration
// edited through the real table editor and keyboard events, with the app exposed only as an oracle for the task, the
// schedule and the displayed values. One 50k page is shared by the whole file (ruling devteam-1-90): the tests run in
// order on it, each starting from what the one before left, and the last restores the original duration. verify.mjs ran
// the section in the 1280x800 window, as this page does.

import { AXIS, ROW } from "./support/pixels";

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

test.describe.serial("table edit journey", () => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  let page: Page;
  let logs: string[];
  let box: { x: number; y: number; width: number; height: number };
  // The finishing task and what it was before any edit; the first test sets them and every later test reads them.
  let finishing: { T: number; row: number; rowSelector: string; F0: number; M0: number; es0: number };

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
  });

  test.afterAll(async ({}, testInfo) => {
    await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
    const errors = pageErrors(logs);
    expect.soft(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`).toBe(true);
    await page.close();
  });

  test("the name editor renames the finishing task's leaf and keeps its outline prefix", { tag: "@smoke" }, async () => {
    const T = await page.evaluate(() => window.gantt!.app.test_finishing_task() ?? null);
    const row = T === null ? null : await page.evaluate((task) => window.gantt!.app.test_task_row(task) ?? null, T);
    expect.soft(T !== null && row !== null, `the finishing task ${T} has a table row (${row})`).toBe(true);
    // Without a row every later test is meaningless; failing here skips them (serial mode).
    if (T === null || row === null) return;
    const rowSelector = `#table-rows .row[data-row="${row}"]`;
    const targetScroll = Math.max(0, row * ROW - 5 * ROW);
    await page.evaluate((y) => (document.getElementById("table-body")!.scrollTop = y), targetScroll);
    await page.waitForFunction((selector) => document.querySelector(selector) !== null, rowSelector, { timeout: 10_000 });
    await nextFrame(page, 2);

    const initial = await page.evaluate(
      ([task, taskRow]) => {
        const app = window.gantt!.app;
        return {
          finish: app.test_project_finish(),
          minutes: app.test_task_minutes(task),
          earlyStart: app.row_start(taskRow),
          label: app.task_label(task),
          name: app.task_name(task),
        };
      },
      [T, row],
    );
    const F0 = initial.finish;
    const M0 = initial.minutes;
    // The row is drawn (waited for above), so the engine has a start for it; verify.mjs passed the value through unchecked.
    if (initial.earlyStart === undefined) throw new Error(`task ${T} on row ${row} has no early start`);
    const es0 = initial.earlyStart;
    finishing = { T, row, rowSelector, F0, M0, es0 };
    const oracleF0 = await page.evaluate(([task, start, minutes]) => window.gantt!.app.test_calendar_add(task, start, minutes), [T, es0, M0]);
    expect.soft(oracleF0 === F0, `finishing task ${T}: test_calendar_add(${es0}, ${M0}) is project finish ${oracleF0} (want ${F0})`).toBe(true);

    // Rename the leaf while preserving its outline prefix.
    await page.dblclick(`${rowSelector} .name`);
    await nextFrame(page, 2);
    const renameOpen = await page.evaluate((selector) => {
      const app = window.gantt!.app;
      const card = document.getElementById("hover-card");
      return {
        input: document.querySelector(`${selector} .name input.cell-edit`) !== null,
        dragging: app.dragging_bar(),
        cardHidden: card?.hidden ?? true,
      };
    }, rowSelector);
    expect.soft(renameOpen.input, `task ${T}: double-clicking the name opens input.cell-edit`).toBe(true);
    expect.soft(!renameOpen.dragging, `task ${T}: no bar drag is active while the name editor is open (${renameOpen.dragging})`).toBe(true);
    expect.soft(renameOpen.cardHidden, `task ${T}: the hover card is hidden while the name editor is open`).toBe(true);
    await page.keyboard.press("Control+A");
    await page.keyboard.type("Renamed by verify");
    await page.keyboard.press("Enter");
    await nextFrame(page, 2);
    const renamed = await page.evaluate(
      ([selector, task]) => ({
        input: document.querySelector(`${selector} input.cell-edit`) !== null,
        cell: document.querySelector(`${selector} .name`)?.textContent ?? null,
        label: window.gantt!.app.task_label(task),
        name: window.gantt!.app.task_name(task),
      }),
      [rowSelector, T] as const,
    );
    const oldPrefix = initial.name.slice(0, initial.name.length - initial.label.length);
    expect.soft(!renamed.input, `task ${T}: Enter closes the name editor`).toBe(true);
    expect.soft(renamed.label === "Renamed by verify", `task ${T}: leaf label is ${JSON.stringify(renamed.label)}`).toBe(true);
    expect
      .soft(
        renamed.cell?.endsWith("Renamed by verify") === true,
        `task ${T}: name cell ends with the renamed leaf (${JSON.stringify(renamed.cell)})`,
      )
      .toBe(true);
    expect
      .soft(
        renamed.label === renamed.cell,
        `task ${T}: app leaf label and name cell agree (${JSON.stringify(renamed.label)} / ${JSON.stringify(renamed.cell)})`,
      )
      .toBe(true);
    expect
      .soft(
        renamed.name === `${oldPrefix}Renamed by verify`,
        `task ${T}: rename preserved outline prefix ${JSON.stringify(oldPrefix)} (${JSON.stringify(initial.name)} -> ${JSON.stringify(renamed.name)})`,
      )
      .toBe(true);
  });

  test("Escape cancels a duration edit, and refused durations keep the editor open and change nothing", async () => {
    const { T, rowSelector, F0, M0 } = finishing;
    // Escape cancels a valid-looking duration without applying it.
    await page.dblclick(`${rowSelector} .dur`);
    const durationAtOpen = await page.locator(`${rowSelector} .dur input.cell-edit`).inputValue();
    const durationBeforeCancel = await page.evaluate((task) => window.gantt!.app.task_duration(task), T);
    expect
      .soft(
        durationAtOpen === durationBeforeCancel,
        `task ${T}: duration editor starts at ${JSON.stringify(durationAtOpen)} (app ${JSON.stringify(durationBeforeCancel)})`,
      )
      .toBe(true);
    const revisionInEditor = (await status(page)).revision;
    await page.keyboard.press("Control+Z");
    await nextFrame(page, 2);
    const revisionAfterEditorUndo = (await status(page)).revision;
    expect
      .soft(
        revisionAfterEditorUndo === revisionInEditor,
        `task ${T}: Ctrl+Z in the duration editor left revision ${revisionAfterEditorUndo} unchanged (was ${revisionInEditor})`,
      )
      .toBe(true);
    await page.keyboard.press("Control+A");
    await page.keyboard.type("9d");
    await page.keyboard.press("Escape");
    await nextFrame(page, 2);
    const escaped = await page.evaluate(
      ([selector, task]) => ({
        input: document.querySelector(`${selector} input.cell-edit`) !== null,
        finish: window.gantt!.app.test_project_finish(),
        minutes: window.gantt!.app.test_task_minutes(task),
        cell: document.querySelector(`${selector} .dur`)?.textContent ?? null,
        duration: window.gantt!.app.task_duration(task),
      }),
      [rowSelector, T] as const,
    );
    expect.soft(!escaped.input, `task ${T}: Escape closes the duration editor`).toBe(true);
    expect.soft(escaped.finish === F0, `task ${T}: Escape left project finish at ${escaped.finish} (want ${F0})`).toBe(true);
    expect.soft(escaped.minutes === M0, `task ${T}: Escape left duration at ${escaped.minutes} minutes (want ${M0})`).toBe(true);
    expect
      .soft(
        escaped.cell === escaped.duration,
        `task ${T}: after Escape the duration cell is ${JSON.stringify(escaped.cell)} (app ${JSON.stringify(escaped.duration)})`,
      )
      .toBe(true);

    // Refused commits keep the editor open, display the reason, and change nothing.
    const refusal = () =>
      page.evaluate(
        ([selector, task]) => {
          const input = document.querySelector(`${selector} .dur input.cell-edit`);
          const error = document.querySelector(`${selector} .cell-error`);
          return {
            input: input !== null,
            invalid: input?.classList.contains("invalid") ?? false,
            error: error?.textContent ?? "",
            finish: window.gantt!.app.test_project_finish(),
            minutes: window.gantt!.app.test_task_minutes(task),
          };
        },
        [rowSelector, T] as const,
      );
    await page.dblclick(`${rowSelector} .dur`);
    await page.keyboard.press("Control+A");
    await page.keyboard.type("-99d");
    await page.keyboard.press("Enter");
    const refusedNegative = await refusal();
    expect
      .soft(
        refusedNegative.input && refusedNegative.invalid && refusedNegative.error.trim() !== "",
        `task ${T}: -99d is refused with the editor open, invalid, and error ${JSON.stringify(refusedNegative.error)}`,
      )
      .toBe(true);
    expect
      .soft(refusedNegative.finish === F0, `task ${T}: refused -99d left project finish at ${refusedNegative.finish} (want ${F0})`)
      .toBe(true);
    expect
      .soft(refusedNegative.minutes === M0, `task ${T}: refused -99d left duration at ${refusedNegative.minutes} minutes (want ${M0})`)
      .toBe(true);
    await page.keyboard.press("Control+A");
    await page.keyboard.type("abc");
    await page.keyboard.press("Enter");
    const refusedText = await refusal();
    expect
      .soft(
        refusedText.input && refusedText.invalid && refusedText.error.trim() !== "",
        `task ${T}: abc is refused with the editor open, invalid, and error ${JSON.stringify(refusedText.error)}`,
      )
      .toBe(true);
    expect.soft(refusedText.finish === F0, `task ${T}: refused abc left project finish at ${refusedText.finish} (want ${F0})`).toBe(true);
    expect
      .soft(refusedText.minutes === M0, `task ${T}: refused abc left duration at ${refusedText.minutes} minutes (want ${M0})`)
      .toBe(true);
    await page.keyboard.press("Escape");
    await nextFrame(page, 2);
    const refusedClosed = await page.evaluate(
      (selector) => ({
        input: document.querySelector(`${selector} input.cell-edit`) !== null,
        error: document.querySelector(`${selector} .cell-error`) !== null,
      }),
      rowSelector,
    );
    expect
      .soft(
        !refusedClosed.input && !refusedClosed.error,
        `task ${T}: Escape clears the refused editor and its error (${JSON.stringify(refusedClosed)})`,
      )
      .toBe(true);
  });

  test("a canvas drag begun while a duration is being edited is ignored and only cancels the editor", async () => {
    const { T, row, rowSelector, F0, es0 } = finishing;
    await page.mouse.move(box.x + box.width / 2, box.y + AXIS + 10 * ROW);
    let off0 = Infinity;
    for (let i = 0; i < 200; i++) {
      await nextFrame(page);
      const dx = (await page.evaluate((t) => window.gantt!.app.x_of(t), es0)) - box.width / 5;
      if (Math.abs(dx) < 1 || Math.abs(dx) >= off0) break;
      off0 = Math.abs(dx);
      await page.mouse.wheel(Math.sign(dx) * Math.min(Math.abs(dx), 2000), 0);
    }
    await nextFrame(page, 2);

    // A canvas gesture begun while editing is ignored; its blur only cancels the editor.
    await page.dblclick(`${rowSelector} .dur`);
    const [barX0, barX1] = await page.evaluate(
      ([start, finish]) => {
        const app = window.gantt!.app;
        return [app.x_of(start), app.x_of(finish)];
      },
      [es0, F0] as const,
    );
    const dragStatus = await status(page);
    const editDragRevision = dragStatus.revision;
    const x0 = box.x + barX0;
    const x1 = box.x + barX1;
    const editDragX = Math.max(box.x + 1, Math.min(box.x + box.width - 1, (x0 + Math.max(x1, x0 + 2)) / 2));
    const editDragY = box.y + AXIS + (row * ROW - dragStatus.scrollY) + ROW / 2;
    await page.mouse.move(editDragX, editDragY);
    await nextFrame(page, 2);
    const editDragHover = (await status(page)).hover?.task;
    expect
      .soft(
        editDragHover === T,
        `task ${T}: the canvas press point is on the finishing task's bar (hover ${JSON.stringify(editDragHover)})`,
      )
      .toBe(true);
    await page.mouse.down();
    for (let k = 1; k <= 4; k++) await page.mouse.move(editDragX + 15 * k, editDragY);
    await page.mouse.up();
    await nextFrame(page, 2);
    const ignoredDrag = await status(page);
    const editorAfterDrag = await page.locator(`${rowSelector} input.cell-edit`).count();
    expect
      .soft(
        ignoredDrag.revision === editDragRevision,
        `task ${T}: canvas drag while editing left revision at ${ignoredDrag.revision} (was ${editDragRevision})`,
      )
      .toBe(true);
    expect
      .soft(
        ignoredDrag.drag === null || ignoredDrag.drag?.active !== true,
        `task ${T}: canvas drag while editing stayed inactive (${JSON.stringify(ignoredDrag.drag)})`,
      )
      .toBe(true);
    expect
      .soft(editorAfterDrag === 0, `task ${T}: the canvas press blurred and cancelled the duration editor (${editorAfterDrag} editors)`)
      .toBe(true);
    expect
      .soft(
        ignoredDrag.drag === null,
        `task ${T}: canvas drag while editing recorded no bar drag (${JSON.stringify(ignoredDrag.drag)})`,
      )
      .toBe(true);
  });

  test("a bare number typed in a duration cell is days", async () => {
    const { T, rowSelector, M0 } = finishing;
    await page.dblclick(`${rowSelector} .dur`);
    await page.keyboard.press("Control+A");
    await page.keyboard.type("5");
    await page.keyboard.press("Enter");
    await nextFrame(page, 2);
    const typed = await page.evaluate(
      ([selector, task]) => ({
        input: document.querySelector(`${selector} input.cell-edit`) !== null,
        cell: document.querySelector(`${selector} .dur`)?.textContent ?? null,
        duration: window.gantt!.app.task_duration(task),
      }),
      [rowSelector, T] as const,
    );
    expect.soft(!typed.input, `task ${T}: Enter closes the editor after typing 5`).toBe(true);
    expect.soft(typed.duration === "5d", `task ${T}: typing 5 gave ${JSON.stringify(typed.duration)} (want "5d")`).toBe(true);
    expect.soft(typed.cell === "5d", `task ${T}: the duration cell shows ${JSON.stringify(typed.cell)} (want "5d")`).toBe(true);
    // Put the original duration back for the next test.
    await page.dblclick(`${rowSelector} .dur`);
    await page.keyboard.press("Control+A");
    await page.keyboard.type(`${M0}m`);
    await page.keyboard.press("Enter");
    await nextFrame(page, 2);
  });

  test("a longer duration moves the project finish to the engine calendar's finish, and restoring it puts the finish back", async () => {
    const { T, row, rowSelector, F0, M0, es0 } = finishing;
    // Extend the finishing task and compare the resulting finish with the
    // engine calendar, including the table update in the first two frames.
    const M1 = M0 + 16 * 60;
    await page.dblclick(`${rowSelector} .dur`);
    const revisionBeforeEdit = (await status(page)).revision;
    await page.keyboard.press("Control+A");
    await page.keyboard.type(`${M1}m`);
    await page.keyboard.press("Enter");
    await nextFrame(page, 2);
    const E1 = await page.evaluate(([task, start, minutes]) => window.gantt!.app.test_calendar_add(task, start, minutes), [T, es0, M1]);
    const edited = await page.evaluate(
      ([selector, task, taskRow]) => ({
        input: document.querySelector(`${selector} input.cell-edit`) !== null,
        finish: window.gantt!.app.test_project_finish(),
        minutes: window.gantt!.app.test_task_minutes(task),
        durationCell: document.querySelector(`${selector} .dur`)?.textContent ?? null,
        duration: window.gantt!.app.task_duration(task),
        finishCell: document.querySelector(`${selector} .finish`)?.textContent ?? null,
        tableFinish: (JSON.parse(window.gantt!.app.table_rows(taskRow, taskRow + 1)) as string[][])[0][2],
      }),
      [rowSelector, T, row] as const,
    );
    const revisionAfterEdit = (await status(page)).revision;
    expect.soft(!edited.input, `task ${T}: Enter closes the accepted duration editor`).toBe(true);
    expect.soft(E1 > F0, `task ${T}: longer duration moves the calendar finish from ${F0} to ${E1}`).toBe(true);
    expect
      .soft(
        edited.finish === E1,
        `task ${T}: project finish is ${edited.finish}, engine calendar expected ${E1} (+${E1 - F0} minutes from ${F0})`,
      )
      .toBe(true);
    expect.soft(edited.minutes === M1, `task ${T}: duration is ${edited.minutes} minutes (want ${M1})`).toBe(true);
    expect
      .soft(
        edited.durationCell === edited.duration,
        `task ${T}: duration cell is ${JSON.stringify(edited.durationCell)} (app ${JSON.stringify(edited.duration)})`,
      )
      .toBe(true);
    expect
      .soft(
        revisionAfterEdit === revisionBeforeEdit + 1,
        `task ${T}: accepted duration raised revision ${revisionBeforeEdit} to ${revisionAfterEdit} (want exactly 1)`,
      )
      .toBe(true);
    expect
      .soft(
        edited.finishCell === edited.tableFinish,
        `task ${T}: within two frames finish cell is ${JSON.stringify(edited.finishCell)} (table_rows ${JSON.stringify(edited.tableFinish)})`,
      )
      .toBe(true);

    // Restore the original duration exactly.
    await page.dblclick(`${rowSelector} .dur`);
    await page.keyboard.press("Control+A");
    await page.keyboard.type(`${M0}m`);
    await page.keyboard.press("Enter");
    await nextFrame(page, 2);
    const restored = await page.evaluate(
      (task) => ({
        finish: window.gantt!.app.test_project_finish(),
        minutes: window.gantt!.app.test_task_minutes(task),
      }),
      T,
    );
    expect.soft(restored.finish === F0, `task ${T}: restoring duration restored project finish ${restored.finish} (want ${F0})`).toBe(true);
    expect.soft(restored.minutes === M0, `task ${T}: restoring duration restored ${restored.minutes} minutes (want ${M0})`).toBe(true);
  });
});
