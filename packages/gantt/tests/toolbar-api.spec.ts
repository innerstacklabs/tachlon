import { readFileSync } from "node:fs";
import { expect, test, type Gantt } from "./support/gantt";

// gs#1260: the public API gives a host everything the toolbar shows, so a host can build the same toolbar: the slipped count
// (`state().baseline.slipped`), the data date's time of day (`state().dataDateTime`), deleting the selected link
// (`commands.deleteLink`), and the sentence a refusal is shown with (`message`, `EditError`'s Display, the one wording table).

const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");

const api = (g: Gantt) => g.page.evaluate(() => window.gantt!.api!.state());

test("state() gives the data date's time beside its day, and the baseline's slipped count", async ({ g }) => {
  await g.open({ n: 200 });
  expect(await g.page.evaluate(() => window.gantt!.api!.commands.setBaseline())).toEqual({ ok: true });
  await g.nextFrame(3);
  expect((await api(g)).baseline?.slipped, "nothing slipped yet").toBe(0);
  // A data date past the whole demo pushes every task that has not finished: they slip.
  expect(await g.page.evaluate(() => window.gantt!.api!.commands.setDataDate("2031-01-06"))).toEqual({ ok: true });
  await g.nextFrame(3);
  const slipped = (await api(g)).baseline?.slipped;
  expect(slipped).toBeGreaterThan(0);
  expect(slipped, "the toolbar's count").toBe((await g.status()).baseline?.slipped);

  const loaded = await g.page.evaluate(async (text) => {
    const file = new File([new TextEncoder().encode(text)], "ledger.xer", { type: "application/octet-stream" });
    return window.gantt!.api!.load({ file });
  }, LEDGER);
  expect(loaded).toEqual({ ok: true });
  await g.nextFrame(4);
  const s = await api(g);
  expect(s.dataDateTime, "the file's 08:00 is kept").toBe("2024-01-10 08:00");
  expect(s.dataDate).toBe("2024-01-10");
  await expect(g.page.locator("#data-date")).toHaveText(`Data date ${s.dataDateTime}`);
});

test("a refusal carries the sentence the toolbar shows for it", async ({ g }) => {
  await g.open({ n: 200 });
  const line = g.page.locator("#edit-error-text");
  const box = g.page.locator("#data-date-input");
  await box.fill("next week");
  await box.press("Enter");
  await expect(line).toHaveText("Write the data date as YYYY-MM-DD.");
  const shown = await line.textContent();
  expect(await g.page.evaluate(() => window.gantt!.api!.commands.setDataDate("next week"))).toEqual({ ok: false, reason: "badDate", message: shown });
  // A refusal the app makes, not the page: Insert under a row filter.
  expect(await g.page.evaluate(() => window.gantt!.api!.commands.setFilter("critical"))).toBe(true);
  await g.page.locator("#insert-row").click();
  await expect(line).toHaveText("Clear the row filter or window to insert a row.");
  const insert = await line.textContent();
  expect(await g.page.evaluate(() => window.gantt!.api!.commands.insertRow())).toEqual({ ok: false, reason: "filterActive", message: insert });
});

test("deleteLink removes the selected link, one undo step, and refuses with no link selected", async ({ g }) => {
  await g.open({ n: 200 });
  const del = () => g.page.evaluate(() => window.gantt!.api!.commands.deleteLink());
  expect(await del()).toEqual({ ok: false, reason: "noSelection", message: "Select a link first." });
  const dep = await g.page.evaluate(() => {
    const app = window.gantt!.app;
    for (let task = 0; task < 200; task += 1) if (app.select_task(task) && app.step_link(true)) return app.selected_link() ?? null;
    return null;
  });
  expect(dep, "a demo task has a link to select").not.toBeNull();
  const text = await g.page.evaluate((d) => window.gantt!.app.link_text(d), dep!);
  expect(await del()).toEqual({ ok: true, link: dep });
  await g.nextFrame(2);
  expect((await g.status()).link.selected, "nothing is left selected").toBeNull();
  expect(await g.page.evaluate((d) => window.gantt!.app.link_text(d), dep!), "the link is gone").toBeUndefined();
  expect(await del()).toEqual({ ok: false, reason: "noSelection", message: "Select a link first." });
  expect(await g.page.evaluate(() => window.gantt!.api!.commands.undo())).toBe(true);
  expect(await g.page.evaluate((d) => window.gantt!.app.link_text(d), dep!), "undo puts it back").toBe(text);
});
