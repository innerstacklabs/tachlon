import { expect, type Page } from "@playwright/test";
import { acrossReload, waitFonts } from "./gantt";
import { AXIS } from "./pixels";

// The task-table bound check that verify.mjs (removed in 3f191ea6) `checkTable` runs (copied here the first time a spec needed it,
// gs#97 rule 6; verify.mjs keeps its own copy until its last user is gone).

import { ROW } from "./pixels";

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

const tableState = (page: Page) =>
  page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row")];
    const name = rows.length ? rows[0].querySelector(".name") : null;
    const tops: Record<string, number> = {};
    for (const r of rows) tops[r.dataset.row!] = r.getBoundingClientRect().top;
    return {
      scrollTop: body.scrollTop,
      count: rows.length,
      first: rows.length ? Number(rows[0].dataset.row) : null,
      last: rows.length ? Number(rows[rows.length - 1].dataset.row) : null,
      tops,
      text: rows.length ? rows[0].textContent : "",
      name: name?.textContent ?? "",
      pad: name ? Number.parseFloat(getComputedStyle(name).paddingLeft) : 0,
    };
  });

/** How far the table's `scrollTop` may sit from the canvas's scroll, in pixels. */
const SCROLL_TOLERANCE = 1;

/** The idle count `tableSettled` leaves on the page while it watches. */
interface IdleWatch {
  writes: number;
  frames: number;
  done: boolean;
}

/**
 * Wait until the table is settled, then count the writes to `#table-rows` over 30 frames and return the count. Settled:
 * the page is ready and has drawn 30 frames, the fonts are loaded, the table's `scrollTop` is on the canvas's scroll and
 * the drawn rows cover the visible interval. The observer attaches in the same evaluate that sees that state, so no
 * redraw owed to an earlier scroll or document swap lands in the count. A navigation while the page is still settling is
 * waited out; one during the 30 frames fails with a named error, never a retry.
 */
export async function tableSettled(page: Page): Promise<number> {
  await waitFonts(page);
  await acrossReload(page, () =>
    page.evaluate(
      ({ tolerance }) =>
        new Promise<void>((attached) => {
          const settled = (): boolean => {
            if (!window.gantt?.ready || document.fonts.status !== "loaded") return false;
            const s = window.gantt.status();
            const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-row]")];
            if (s.frames < 30 || rows.length === 0) return false;
            const first = Number(rows[0].dataset.row);
            const last = Number(rows[rows.length - 1].dataset.row);
            const body = document.getElementById("table-body")!;
            return Math.abs(body.scrollTop - s.scrollY) <= tolerance && first <= s.rows[0] && last >= s.rows[1] - 1;
          };
          const poll = (): void => {
            if (!settled()) return void requestAnimationFrame(poll);
            const watch: IdleWatch = { writes: 0, frames: 0, done: false };
            const observer = new MutationObserver((records) => (watch.writes += records.length));
            observer.observe(document.getElementById("table-rows")!, { subtree: true, childList: true, characterData: true, attributes: true });
            const tick = (): void => {
              if (++watch.frames < 30) return void requestAnimationFrame(tick);
              observer.disconnect();
              watch.done = true;
            };
            (window as unknown as { __tableIdle: IdleWatch }).__tableIdle = watch;
            requestAnimationFrame(tick);
            attached();
          };
          poll();
        }),
      { tolerance: SCROLL_TOLERANCE },
    ),
  );
  try {
    return await page.evaluate(
      () =>
        new Promise<number>((resolve, reject) => {
          const watch = (window as unknown as { __tableIdle?: IdleWatch }).__tableIdle;
          if (!watch) return reject(new Error("the table's idle watch is gone"));
          const wait = (): void => (watch.done ? resolve(watch.writes) : void requestAnimationFrame(wait));
          wait();
        }),
    );
  } catch (error) {
    if (/context was destroyed|navigat|idle watch is gone/i.test(String(error))) {
      throw new Error(`the document navigated during the 30 idle frames, so the table was not idle: ${String(error)}`);
    }
    throw error;
  }
}

/**
 * After a scroll: the table holds the visible rows, within the bound, sits on the same y as the canvas's rows, and scrolls
 * by the canvas's amount. `box` is the timeline's bounding box.
 */
export async function checkTable(page: Page, box: { y: number }, label: string) {
  await nextFrame(page, 3);
  const s = await page.evaluate(() => window.gantt!.status());
  const t = await tableState(page);
  const visible = s.rows[1] - s.rows[0];
  const bound = 2 * visible;
  expect
    .soft(
      t.count >= visible && t.count <= bound,
      `${label}: ${t.count} table rows in the DOM for ${visible} visible rows of ${s.n} (bound 2x = ${bound})`,
    )
    .toBe(true);
  expect
    .soft(
      t.first! <= s.rows[0] && t.last! >= s.rows[1] - 1,
      `${label}: the table holds the canvas's rows ${s.rows.join("-")} (DOM rows ${t.first}-${t.last})`,
    )
    .toBe(true);
  expect
    .soft(
      Math.abs(t.scrollTop - s.scrollY) <= SCROLL_TOLERANCE,
      `${label}: table scrollTop ${t.scrollTop} matches the canvas's scroll ${s.scrollY.toFixed(2)}`,
    )
    .toBe(true);
  const want = box.y + AXIS + s.rows[0] * ROW - s.scrollY;
  const got = t.tops[s.rows[0]];
  expect
    .soft(
      got !== undefined && Math.abs(got - want) <= 1.5,
      `${label}: row ${s.rows[0]} is at y ${got} in the table, ${want.toFixed(2)} on the canvas`,
    )
    .toBe(true);
  return { s, t };
}
