// The importer's notes show as a one-line notice in the toolbar row ("N import notes"), not as a panel over the chart
// (gs#818). A click lists them below the toolbar; Dismiss makes the notice quiet but a click still lists them. The fixture is one-project.xer, whose
// open reports four notes (see open.spec.ts).
import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

const FIXTURE = readFileSync(new URL("./fixtures/one-project.xer", import.meta.url));

test("a file with import notes shows a one-line notice in the toolbar, and a click lists the notes", async ({ g }) => {
  await g.open({ n: 300 });
  await expect(g.page.locator("#import-notes")).toBeHidden();

  await g.page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: FIXTURE });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);

  const toggle = g.page.locator("#import-notes");
  await expect(toggle).toHaveText("5 import notes");
  await expect(g.page.locator("#unsupported")).toBeHidden();

  // The notice is one line inside the toolbar strip, and nothing covers the chart's top right corner or the toolbar's buttons.
  const fit = await g.page.evaluate(() => {
    const box = (selector: string): DOMRect => document.querySelector(selector)!.getBoundingClientRect();
    const bar = box("#toolbar");
    const notice = box("#import-notes");
    const canvas = box("#timeline");
    const at = (x: number, y: number): string => document.elementFromPoint(x, y)?.id ?? "";
    const insert = box("#insert-row");
    return {
      inside: notice.top >= bar.top && notice.bottom <= bar.bottom && notice.right <= bar.right,
      height: box("#import-notes").height,
      cornerIsCanvas: at(canvas.right - 8, canvas.top + 8),
      insertIsButton: at(insert.left + insert.width / 2, insert.top + insert.height / 2),
    };
  });
  expect(fit.inside, "the notice sits inside the toolbar strip").toBe(true);
  expect(fit.height, "the notice is one button high").toBeLessThanOrEqual(22);
  expect(fit.cornerIsCanvas, "the chart's top right corner is the chart").toBe("timeline");
  expect(fit.insertIsButton, "the Insert row button is not covered").toBe("insert-row");

  // A click lists the notes, below the toolbar; a second click folds them away.
  await toggle.click();
  await expect(g.page.locator("#unsupported")).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await expect(g.page.locator("#unsupported-list")).toContainText("in-progress activities: 2 of 2");
  const lines = await g.page.evaluate(() => document.querySelector("#unsupported-list")!.textContent!.split("\n"));
  expect(lines, "one line per note").toHaveLength(5);
  const panel = await g.page.locator("#unsupported").boundingBox();
  const bar = await g.page.locator("#toolbar").boundingBox();
  expect(panel!.y, "the list opens below the toolbar").toBeGreaterThanOrEqual(bar!.y + bar!.height);
  await toggle.click();
  await expect(g.page.locator("#unsupported")).toBeHidden();
});

test("after Dismiss the notice goes quiet and a click still lists the same notes", async ({ g }) => {
  await g.open({ n: 300 });
  await g.page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: FIXTURE });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const notice = g.page.locator("#import-notes");
  await notice.click();
  const before = await g.page.locator("#unsupported-list").textContent();
  await g.page.locator("#import-notes-dismiss").click();

  await expect(g.page.locator("#unsupported")).toBeHidden();
  await expect(notice).toBeVisible();
  await expect(notice).toHaveText("ⓘ");
  await expect(notice).toHaveAccessibleName("5 import notes");
  await expect(notice).toHaveAttribute("aria-label", "5 import notes");
  await expect(notice).toHaveAttribute("title", "5 import notes");
  await expect(g.page.locator("#import-notes-dismiss")).toHaveAttribute("title", /click still lists the notes/);

  await notice.click();
  await expect(g.page.locator("#unsupported")).toBeVisible();
  await expect(g.page.locator("#unsupported-list")).toHaveText(before!);
});

test("the notice can be dismissed, and the next file with notes brings it back", async ({ g }) => {
  await g.open({ n: 300 });
  const pick = async (epoch: number): Promise<void> => {
    await g.page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: FIXTURE });
    await g.page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch, { timeout: 60_000 });
    await g.nextFrame(3);
  };
  await pick(1);
  await expect(g.page.locator("#import-notes")).toBeVisible();

  await g.page.locator("#import-notes").click();
  await g.page.locator("#import-notes-dismiss").click();
  await expect(g.page.locator("#import-notes")).toHaveText("ⓘ");
  await expect(g.page.locator("#unsupported")).toBeHidden();
  // Dismissing quiets the notice, not the reader's report.
  expect(await g.page.evaluate(() => window.gantt!.unsupported!.length)).toBe(5);

  await pick(2);
  await expect(g.page.locator("#import-notes")).toHaveText("5 import notes");
});

test("focus stays on a control that remains: the notice after the list folds, the chart after a dismiss", async ({ g }) => {
  await g.open({ n: 300 });
  await g.page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: FIXTURE });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const active = (): Promise<string> => g.page.evaluate(() => (document.activeElement as HTMLElement).id || (document.activeElement as HTMLElement).className);

  // The keyboard opens the list, Escape folds it and the focus is on the notice, which opens it again.
  await g.page.locator("#import-notes").focus();
  await g.page.keyboard.press("Enter");
  await expect(g.page.locator("#unsupported")).toBeVisible();
  await g.page.locator("#unsupported-close").focus();
  await g.page.keyboard.press("Escape");
  await expect(g.page.locator("#unsupported")).toBeHidden();
  expect(await active(), "focus returns to the notice").toBe("import-notes");
  await g.page.keyboard.press("Enter");
  await expect(g.page.locator("#unsupported")).toBeVisible();
  await g.page.locator("#unsupported-close").click();
  expect(await active(), "the close button returns focus to the notice").toBe("import-notes");

  // Dismissing hides the button that had the focus; the chart takes it, so its keys still work (Shift+C is Critical only).
  await g.page.locator("#import-notes").click();
  await g.page.locator("#import-notes-dismiss").focus();
  await g.page.keyboard.press("Enter");
  await expect(g.page.locator("#unsupported")).toBeHidden();
  expect(await active(), "focus returns to the chart").toBe("app");
  await g.page.keyboard.press("Shift+C");
  await expect(g.page.locator("#critical-toggle")).toHaveAttribute("aria-pressed", "true");
});
