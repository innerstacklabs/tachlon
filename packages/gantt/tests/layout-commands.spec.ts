import { expect, test, type Gantt } from "./support/gantt";
import type { Commands } from "../src/commands";

// `layoutJson` and `applyLayout` (Part of gantt#703): the chart gives the page its print setup, the list columns included, and takes a saved one back.

declare global {
  interface Window {
    hostCommands?: Commands;
  }
}

async function open(g: Gantt): Promise<void> {
  await g.page.addInitScript(() => {
    document.addEventListener("gantt:ready", (e) => {
      window.hostCommands = (e as CustomEvent<Commands>).detail;
    });
  });
  await g.open({ n: 200 });
  await g.page.waitForFunction(() => window.hostCommands !== undefined);
}

test("a layout read, then the columns changed, then applied back, puts the same columns in the same order", async ({ g }) => {
  const { page } = g;
  await open(g);
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  const columns = (json: unknown): string[] => (JSON.parse(json as string) as { columns: string[] }).columns;
  const read = () => page.evaluate(() => window.hostCommands!.layoutJson("Saved"));
  const saved = await read();
  expect(columns(saved), "the layout names the columns the screen shows, in its order").toEqual(["ID", "WBS", "Task", "Dur", "Start", "Finish", "Float", "Start variance", "Finish variance", "Float variance"]);

  // Hide Float, move Finish up before Start, and hide the variance columns, as a person would from the toolbar.
  await page.locator("#columns-button").click();
  await page.locator("#columns-float").click();
  await page.locator('#columns-panel [data-row="finish"] button[data-move="-1"]').click();
  await page.keyboard.press("Escape");
  await page.locator("#variance-toggle").click();
  await g.nextFrame(3);
  expect(columns(await read()), "the read follows the screen").toEqual(["ID", "WBS", "Task", "Dur", "Finish", "Start"]);

  const result = await page.evaluate((json) => window.hostCommands!.applyLayout(json as string), saved);
  expect(result, "all went in but the two columns this file has none of").toEqual({
    ok: true,
    notApplied: ["column ID: not applied, the file has no such column", "column WBS: not applied, the file has no such column"],
  });
  await g.nextFrame(3);
  expect(columns(await read()), "the same columns in the same order").toEqual(columns(saved));
  await expect(page.locator("#table")).not.toHaveAttribute("data-hide-float", /.*/);
  await expect(page.locator("#variance-toggle")).toHaveAttribute("aria-pressed", "true");
});

test("a chart mounted without the toolbar takes the Progress and Variance columns back from a layout", async ({ g }) => {
  const { page } = g;
  await page.addInitScript(() => {
    new MutationObserver(() => document.getElementById("toolbar")?.remove()).observe(document, { childList: true, subtree: true });
  });
  await open(g);
  expect(await page.locator("#progress-toggle").count(), "no toolbar, so no buttons").toBe(0);
  const columns = (json: unknown): string[] => (JSON.parse(json as string) as { columns: string[] }).columns;
  const read = () => page.evaluate(() => window.hostCommands!.layoutJson("Saved"));
  const layout = JSON.parse((await read()) as string) as { columns: string[] };
  expect(layout.columns, "Progress is off and Variance is on to start").toEqual(["ID", "WBS", "Task", "Dur", "Start", "Finish", "Float", "Start variance", "Finish variance", "Float variance"]);
  const wanted = ["ID", "WBS", "Task", "% complete", "Actual start", "Actual finish", "Remaining", "Dur", "Start", "Finish", "Float"];
  const result = await page.evaluate((json) => window.hostCommands!.applyLayout(json), JSON.stringify({ ...layout, columns: wanted }));
  expect(result).toMatchObject({ ok: true });
  await g.nextFrame(3);
  expect(columns(await read()), "Progress is on and Variance is off, as the layout says").toEqual(wanted);
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await expect(page.locator("#table")).toHaveAttribute("data-variance-hidden", "true");
});

test("a paper the app refuses reads false, and text that is not a layout is refused with nothing changed", async ({ g }) => {
  const { page } = g;
  await open(g);
  expect(await page.evaluate(() => window.hostCommands!.layoutJson("Saved", "folio" as "a4"))).toBe(false);
  const refused = await page.evaluate(() => window.hostCommands!.applyLayout("not json"));
  expect(refused).toMatchObject({ ok: false, reason: "other" });
  await expect(page.locator("#table")).not.toHaveAttribute("data-hide-float", /.*/);
});
