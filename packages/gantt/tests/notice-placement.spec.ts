// The alert lines stand clear of the column headers and have an info style beside the error style (gs#1055). The spec shows a line on
// the office fit-out at 1280 px, asserts that no column header (Task, Dur, Start, Finish, Float) meets the line's box, and that the
// info style is not the error style's colour in the light and the dark theme. Placement is a DOM overlay and CSS; no frame is drawn.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));

/** Open the fit-out, then offer a file the readers refuse, so `#open-error` shows. */
async function openFitoutThenRefuse(g: Gantt): Promise<void> {
  const { page } = g;
  await g.open({ n: 50 });
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  await page.setInputFiles("#open-input", { name: "notes.txt", mimeType: "text/plain", buffer: Buffer.from("not a schedule") });
  await expect(page.locator("#open-error")).toBeVisible();
}

/** Show `#edit-error` the way row-edit.ts does, as the error line or as the info line. */
const showEdit = (page: Page, kind: "error" | "info", message = "Nothing saved yet. Press Enter to finish the edit, then save."): Promise<void> =>
  page.evaluate(([k, m]) => {
    const line = document.getElementById("edit-error")!;
    document.getElementById("edit-error-text")!.textContent = m;
    if (k === "info") line.dataset.kind = "info";
    else delete line.dataset.kind;
    line.hidden = false;
  }, [kind, message] as const);

/** The boxes of the five column headers, and the box of `line`. */
const boxes = (page: Page, line: string) =>
  page.evaluate((id) => {
    const rect = (e: Element) => {
      const r = e.getBoundingClientRect();
      return { name: e.textContent ?? "", top: r.top, bottom: r.bottom, left: r.left, right: r.right };
    };
    return {
      heads: [...document.querySelectorAll("#table-head .cols > span")].map(rect),
      line: rect(document.getElementById(id)!),
    };
  }, line);

const colours = (page: Page) =>
  page.evaluate(() => {
    const cs = getComputedStyle(document.getElementById("edit-error")!);
    return { bg: cs.backgroundColor, fg: cs.color };
  });

async function expectClear(page: Page, line: string): Promise<void> {
  const b = await boxes(page, line);
  expect(b.heads.map((h) => h.name.trim().toLowerCase())).toEqual(["task", "dur", "start", "finish", "float"]);
  for (const h of b.heads) {
    const meets = h.left < b.line.right && h.right > b.line.left && h.top < b.line.bottom && h.bottom > b.line.top;
    expect(meets, `header ${h.name} ${JSON.stringify(h)} meets the line ${JSON.stringify(b.line)}`).toBe(false);
  }
  // No toolbar control is under the line either: each visible button and input of the toolbar (the line's own Dismiss apart) is clear of it.
  const controls = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("#toolbar button, #toolbar input")]
      .filter((e) => e.closest("#open-error, #edit-error") === null && e.getClientRects().length > 0)
      .map((e) => {
        const r = e.getBoundingClientRect();
        return { name: e.id || e.textContent || "", left: r.left, right: r.right, top: r.top, bottom: r.bottom };
      }),
  );
  expect(controls.length, "the toolbar has controls to check").toBeGreaterThan(8);
  for (const c of controls) {
    const meets = c.left < b.line.right && c.right > b.line.left && c.top < b.line.bottom && c.bottom > b.line.top;
    expect(meets, `toolbar control ${JSON.stringify(c)} is under the line ${JSON.stringify(b.line)}`).toBe(false);
  }
  // The line stays in the toolbar's row: it ends where the table and the chart begin, so they do not move when it comes or goes.
  const top = await page.evaluate(() => Math.min(document.getElementById("table")!.getBoundingClientRect().top, document.getElementById("timeline")!.getBoundingClientRect().top));
  expect(b.line.bottom, `line bottom ${b.line.bottom} against the body's top ${top}`).toBeLessThanOrEqual(top + 0.5);
}

for (const scheme of ["light", "dark"] as const) {
  test(`a shown alert line covers no column header, and info differs from error, in ${scheme}`, async ({ g, page }) => {
    test.setTimeout(120_000);
    await page.emulateMedia({ colorScheme: scheme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await openFitoutThenRefuse(g);
    await expectClear(page, "open-error");
    await page.locator("#open-error-close").click();

    await showEdit(page, "error");
    await expectClear(page, "edit-error");
    const error = await colours(page);
    await showEdit(page, "info");
    await expectClear(page, "edit-error");
    const info = await colours(page);
    expect(info.bg, `info ${info.bg} against error ${error.bg}`).not.toBe(error.bg);
    expect(info.fg !== error.fg || info.bg !== error.bg).toBe(true);
    if (process.env.SHOTS_DIR) {
      await page.screenshot({ path: `${process.env.SHOTS_DIR}/notice-info-${scheme}.png` });
      await showEdit(page, "error");
      await page.screenshot({ path: `${process.env.SHOTS_DIR}/notice-error-${scheme}.png` });
    }
  });
}

test("Ctrl+S in a cell editor shows its hint in the info style, clear of the headers and the toolbar", async ({ g, page }) => {
  await g.open({ n: 20 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await expect(page.locator("input.cell-edit").first(), "the editor is open").toBeVisible();
  // The editor's own input stops its keys at the input, so the chart's guard is reached by a key sent to the chart itself.
  await page.locator("#app").dispatchEvent("keydown", { key: "s", ctrlKey: true, metaKey: true, bubbles: true, cancelable: true });
  await expect(page.locator("#edit-error-text")).toHaveText("Nothing saved yet. Press Enter to finish the edit, then save.");
  await expect(page.locator("#edit-error"), "the hint is the info style, not the error style").toHaveAttribute("data-kind", "info");
  const info = await colours(page);
  await expectClear(page, "edit-error");
  await showEdit(page, "error");
  expect(info.bg, "the hint's colour is not the error's").not.toBe((await colours(page)).bg);
});

for (const scheme of ["light", "dark"] as const) {
  test(`a long notice is cut at 420px with its text in a title, and Find and the ? button stay whole, in ${scheme}`, async ({ g, page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await g.open({ n: 20 });
    const long = "A long notice that says what went wrong and what to do about it, with the file name and the reason, and it goes on past what fits. ".repeat(3);
    await showEdit(page, "info", long);
    const m = await page.evaluate(() => {
      const box = (id: string) => {
        const r = document.getElementById(id)!.getBoundingClientRect();
        return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width };
      };
      const span = document.getElementById("edit-error-text")!;
      return { toolbar: box("toolbar"), line: box("edit-error"), find: box("find"), findInput: box("find-input"), keys: box("keys-button"), title: span.title, cut: span.scrollWidth > span.clientWidth };
    });
    expect(m.line.width, "the notice is at most about 420px").toBeLessThanOrEqual(421);
    expect(m.cut, "the long notice is cut").toBe(true);
    expect(m.title, "the full text is the title while it is cut").toBe(long);
    for (const [name, b] of [["find", m.find], ["find input", m.findInput], ["? button", m.keys]] as const) {
      expect(b.width, `${name} has a width`).toBeGreaterThan(8);
      const whole = b.left >= m.toolbar.left - 0.5 && b.right <= m.toolbar.right + 0.5 && b.top >= m.toolbar.top - 0.5 && b.bottom <= m.toolbar.bottom + 0.5;
      expect(whole, `${name} ${JSON.stringify(b)} is inside the toolbar ${JSON.stringify(m.toolbar)}`).toBe(true);
    }
    await expectClear(page, "edit-error");
    if (process.env.SHOTS_DIR) await page.screenshot({ path: `${process.env.SHOTS_DIR}/notice-long-${scheme}.png` });
    await showEdit(page, "info", "Short.");
    await expect(page.locator("#edit-error-text"), "no title while the text is whole").toHaveAttribute("title", "");
  });
}

for (const scheme of ["light", "dark"] as const) {
  test(`Find and the ? button lie inside the 1280px viewport, with and without a notice, in ${scheme}`, async ({ g, page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.setViewportSize({ width: 1280, height: 800 });
    await g.open({ n: 50 });
    await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
    await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
    await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
    const inside = async (state: string): Promise<void> => {
      const m = await page.evaluate(() => {
        const box = (id: string) => {
          const r = document.getElementById(id)!.getBoundingClientRect();
          return { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
        };
        return { find: box("find"), input: box("find-input"), keys: box("keys-button"), toolbar: box("toolbar"), width: window.innerWidth, height: window.innerHeight, pad: parseFloat(getComputedStyle(document.getElementById("toolbar")!).paddingRight), scroll: document.getElementById("toolbar")!.scrollWidth - document.getElementById("toolbar")!.clientWidth };
      });
      expect(m.width, "the viewport is 1280px").toBe(1280);
      for (const [name, b] of [["find", m.find], ["find input", m.input], ["? button", m.keys]] as const) {
        const whole = b.left >= 0 && b.right <= m.width && b.top >= 0 && b.bottom <= m.height && b.right <= m.toolbar.right - m.pad + 0.5;
        expect(whole, `${state}: ${name} ${JSON.stringify(b)} is whole inside the viewport and the toolbar ${JSON.stringify(m.toolbar)}`).toBe(true);
      }
      expect(m.scroll, `${state}: the toolbar does not overflow its box`).toBeLessThanOrEqual(0);
      expect(m.keys.right - m.keys.left, `${state}: the ? button is wide enough to click`).toBeGreaterThanOrEqual(18);
      expect(m.find.right, `${state}: Find ends before the ? button begins`).toBeLessThanOrEqual(m.keys.left + 0.5);
    };
    await inside("no notice");
    if (process.env.SHOTS_DIR) await page.screenshot({ path: `${process.env.SHOTS_DIR}/toolbar-plain-${scheme}.png` });
    await showEdit(page, "info", "Nothing saved yet. Press Enter to finish the edit, then save.");
    await inside("info notice");
    if (process.env.SHOTS_DIR) await page.screenshot({ path: `${process.env.SHOTS_DIR}/toolbar-notice-${scheme}.png` });
    await showEdit(page, "error", "could not open notes.txt: XML error: text appears before the root element at byte 14");
    await inside("error notice");
  });
}
