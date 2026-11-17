import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, type Gantt } from "./gantt";
import { AXIS, ROW } from "./pixels";

// What the specs of the "what moved" note (`moved-note.spec.ts`, `moved-note-reduced.spec.ts`) share: the ledger they
// open, the edits they make to it, the fake clock they freeze, a read of the note, and the press-and-drag that releases a
// bar of the edited task.

// The 43-row ledger fixture of the Rust tests (`crates/app/tests/moved.rs`): editing task 16 to ten working days moves
// eight other tasks and the finish by seven days.
// An activity with actuals is fixed at them, and these specs measure what an edit moves, so the actual dates are blanked.
export const fixture = withoutActuals(readFileSync(new URL("../fixtures/ledger-timeline.xer", import.meta.url), "latin1"));
export const EDITED = 16;

export async function openLedger(g: Gantt): Promise<void> {
  await g.page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }),
  );
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await g.nextFrame(3);
}

/** Install the fake clock and freeze it a second ahead, before any edit. */
export async function freezeClock(page: Page): Promise<void> {
  await page.clock.install();
  await page.clock.pauseAt(Date.now() + 1000);
  await page.clock.runFor(32);
}

export const rowOf = (page: Page, task: number): Promise<number> =>
  page.evaluate((t) => {
    const row = window.gantt!.app.test_task_row(t);
    if (row === undefined) throw new Error(`task ${t} has no row`);
    return row;
  }, task);

/** The task whose activity ID is `code`, so a spec does not depend on the order the file's activities list in. */
export const taskOfCode = (page: Page, code: string): Promise<number> =>
  page.evaluate((c) => {
    for (let t = 0; t < 1000; t++) if (window.gantt!.app.task_code(t) === c) return t;
    throw new Error(`no task has the activity ID ${c}`);
  }, code);

/** Edit the edited task's duration in the task list, the way a person does. */
export async function editDuration(page: Page, value: string): Promise<void> {
  const row = await rowOf(page, EDITED);
  await page.dblclick(`#table-rows .row[data-row="${row}"] .dur`);
  await page.keyboard.press("Control+A");
  await page.keyboard.type(value);
  await page.keyboard.press("Enter");
}

/** Rename the edited task in the task list. */
export async function rename(page: Page, name: string): Promise<void> {
  const row = await rowOf(page, EDITED);
  await page.dblclick(`#table-rows .row[data-row="${row}"] .name`);
  await page.keyboard.press("Control+A");
  await page.keyboard.type(name);
  await page.keyboard.press("Enter");
}

export interface NoteState {
  hidden: boolean;
  text: string;
  opacity: string;
  parent: string | undefined;
  focusable: boolean;
}

export const note = (page: Page): Promise<NoteState> =>
  page.evaluate(() => {
    const el = document.getElementById("moved-note")!;
    return {
      hidden: getComputedStyle(el).opacity === "0",
      text: el.textContent ?? "",
      opacity: el.style.opacity,
      parent: el.parentElement?.id,
      focusable: el.hasAttribute("tabindex"),
    };
  });

/**
 * Hold back (`true`) or let through again (`false`) the browser's own `pointerenter` and `pointerleave` for the note: a
 * capturing listener on the document that stops them before the note's listeners see them. While it is on, a
 * `page.mouse` move still moves the pointer, but the note's enter and leave events do not reach the page's handlers.
 */
export async function muteNoteHover(page: Page, mute: boolean): Promise<void> {
  await page.evaluate((on) => {
    const w = window as unknown as { __muteNoteHover?: (e: Event) => void };
    const events = ["pointerenter", "pointerleave"];
    if (w.__muteNoteHover !== undefined) for (const type of events) document.removeEventListener(type, w.__muteNoteHover, true);
    w.__muteNoteHover = undefined;
    if (!on) return;
    w.__muteNoteHover = (e) => {
      if ((e.target as Element | null)?.id === "moved-note") e.stopImmediatePropagation();
    };
    for (const type of events) document.addEventListener(type, w.__muteNoteHover, true);
  }, mute);
}

/**
 * Press on a bar of the edited task and drag it 124 px to the right, leaving the button down, so the caller reads the
 * page while the drag is on and then releases with `page.mouse.up()`. The press looks for the pixel the hover finds on the
 * bar. With `fakeClock` the frames in between come from `page.clock` (the spec froze it with `freezeClock`); without it
 * they come from the page's own `requestAnimationFrame`. Returns the canvas's box.
 */
export async function pressAndDrag(g: Gantt, fakeClock: boolean): Promise<Awaited<ReturnType<Gantt["box"]>>> {
  const settle = (fakeMs: number): Promise<void> => (fakeClock ? g.page.clock.runFor(fakeMs) : g.nextFrame(2));
  const box = await g.box();
  const row = await rowOf(g.page, EDITED);
  const y = box.y + AXIS + row * ROW + ROW / 2;
  const startX = await g.page.evaluate((r) => window.gantt!.app.x_of(window.gantt!.app.row_start(r)!), row);
  // The bar's left end plus a few pixels is on the bar; look for the pixel the hover finds.
  let pressX: number | null = null;
  for (let dx = 4; dx <= 40 && pressX === null; dx += 4) {
    await g.page.mouse.move(box.x + startX + dx, y);
    await settle(64);
    if ((await g.status()).hover?.task === EDITED) pressX = box.x + startX + dx;
  }
  expect(pressX, "a bar of the edited task is under the pointer").not.toBeNull();
  await g.page.mouse.down();
  await g.page.mouse.move(pressX! + 4, y);
  for (let k = 1; k <= 8; k++) await g.page.mouse.move(pressX! + 4 + 15 * k, y);
  await settle(32);
  return box;
}

function withoutActuals(text: string): Buffer {
  let blank: number[] = [];
  let inTask = false;
  const lines = text.split(/\r?\n/).map((line) => {
    const cells = line.split("\t");
    if (cells[0] === "%T") inTask = cells[1] === "TASK";
    else if (inTask && cells[0] === "%F") blank = cells.flatMap((c, i) => (c === "act_start_date" || c === "act_end_date" ? [i] : []));
    else if (inTask && cells[0] === "%R") for (const i of blank) cells[i] = "";
    return cells.join("\t");
  });
  return Buffer.from(lines.join("\n"), "latin1");
}
