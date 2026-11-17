import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt, openFilters } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

// The link editor in the page (gs#703): with a link selected, Enter opens a small box beside the alert line holding the link's
// type and lag (`FS`, `SS+2d`); Enter applies it through `App.set_link`, Escape closes it, and a refusal shows the engine's
// reason in the alert line and changes nothing. The fixture is `fixtures/links.xml`: A -> C -> E and B -> D under one summary.

test.use({ actionTimeout: 15_000 });
const fixture = readFileSync(new URL("./fixtures/links.xml", import.meta.url));

const BOX = "#link-edit input";
const ALERT = "#edit-error-text";

interface World {
  g: Gantt;
  page: Page;
  id: Record<string, number>;
  row: Record<string, number>;
}

async function openLinks(g: Gantt, xml: Buffer = fixture): Promise<World> {
  await g.open({ n: 50 });
  await g.page.setInputFiles("#open-input", { name: "links.xml", mimeType: "application/xml", buffer: xml });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const rows = await g.page.evaluate(() => {
    const w = window.gantt!;
    const table = JSON.parse(w.app.table_rows(0, w.status().rowCount)) as [number][];
    return table.map((r, at) => ({ at, task: r[0], name: w.app.task_name(r[0]).split(" / ").at(-1)! }));
  });
  const id: Record<string, number> = {};
  const row: Record<string, number> = {};
  for (const r of rows) {
    id[r.name] = r.task;
    row[r.name] = r.at;
  }
  return { g, page: g.page, id, row };
}

/** Task, start and finish of every row: what an undo has to bring back. */
const datesOf = (page: Page): Promise<unknown[]> =>
  page.evaluate(() => {
    const w = window.gantt!;
    return (JSON.parse(w.app.table_rows(0, w.status().rowCount)) as unknown[][]).map((r) => [r[0], r[1], r[2]]);
  });

/** A point inside the bar of `name`, `inset` CSS px right of its start, in page coordinates. */
async function barPoint(w: World, name: string, inset = 20): Promise<{ x: number; y: number }> {
  const box = await w.g.box();
  const status = await w.g.status();
  const x = await w.page.evaluate((r) => window.gantt!.app.x_of(window.gantt!.app.row_start(r)!), w.row[name]);
  return { x: box.x + x + inset, y: box.y + AXIS + w.row[name] * ROW - status.scrollY + ROW / 2 };
}

/** Click task `name`'s bar, then step to its first link with `]`: A has one link only, A to C. */
async function selectLinkOf(w: World, name: string): Promise<number> {
  const at = await barPoint(w, name);
  await w.page.mouse.click(at.x, at.y);
  await w.g.nextFrame(2);
  await w.page.keyboard.press("]");
  await w.g.nextFrame(2);
  const selected = (await w.g.status()).link.selected;
  expect(selected, `a link of ${name} is selected`).not.toBeNull();
  return selected!;
}

const linkText = (page: Page, dep: number): Promise<string | undefined> => page.evaluate((d) => window.gantt!.app.link_text(d), dep);
const movedNote = (page: Page): Promise<string> => page.evaluate(() => document.getElementById("moved-note")?.textContent ?? "");

test("Enter on a selected link opens a box with its text, SS+2d moves the successor, and Ctrl+Z restores", async ({ g }) => {
  const w = await openLinks(g);
  const dep = await selectLinkOf(w, "A");
  expect(await linkText(w.page, dep)).toBe("FS");
  const before = await datesOf(w.page);
  const rev = (await g.status()).revision;

  await w.page.keyboard.press("Enter");
  await expect(w.page.locator(BOX)).toBeFocused();
  await expect(w.page.locator(BOX)).toHaveValue("FS");

  await w.page.keyboard.press("Control+a");
  await w.page.keyboard.type("SS+2d");
  await w.page.keyboard.press("Enter");
  await g.nextFrame(3);
  await expect(w.page.locator(BOX)).toBeHidden();
  expect(await linkText(w.page, dep)).toBe("SS+2d");
  expect((await g.status()).revision, "one history step").toBe(rev + 1);
  const after = await datesOf(w.page);
  expect(after, "the successor moved").not.toEqual(before);
  await expect.poll(() => movedNote(w.page), "the what-moved note appears").not.toBe("");

  await w.page.keyboard.press("Control+z");
  await g.nextFrame(3);
  expect(await linkText(w.page, dep)).toBe("FS");
  expect(await datesOf(w.page)).toEqual(before);
});

test("a refused text shows the engine's reason in the alert line and changes nothing", async ({ g }) => {
  const w = await openLinks(g);
  const dep = await selectLinkOf(w, "A");
  const before = await datesOf(w.page);
  const rev = (await g.status()).revision;

  await w.page.keyboard.press("Enter");
  await w.page.keyboard.press("Control+a");
  await w.page.keyboard.type("XX");
  await w.page.keyboard.press("Enter");
  await g.nextFrame(2);
  await expect(w.page.locator(ALERT), "the engine's reason for an unknown type").toHaveText("start with FS, SS, FF or SF, such as SS+2d");
  await expect(w.page.locator("#edit-error")).toBeVisible();
  await expect(w.page.locator(BOX), "the box stays open for another try").toBeVisible();
  expect(await linkText(w.page, dep)).toBe("FS");
  expect(await datesOf(w.page)).toEqual(before);
  expect((await g.status()).revision).toBe(rev);

  await w.page.keyboard.press("Escape");
  await expect(w.page.locator(BOX)).toBeHidden();
});

test("Escape closes the box and changes nothing", async ({ g }) => {
  const w = await openLinks(g);
  const dep = await selectLinkOf(w, "A");
  const before = await datesOf(w.page);
  const rev = (await g.status()).revision;
  await w.page.keyboard.press("Enter");
  await expect(w.page.locator(BOX), "the box is open before Escape").toBeFocused();
  await w.page.keyboard.press("Control+a");
  await w.page.keyboard.type("FF+3d");
  await w.page.keyboard.press("Escape");
  await g.nextFrame(2);
  await expect(w.page.locator(BOX)).toBeHidden();
  expect(await linkText(w.page, dep)).toBe("FS");
  expect(await datesOf(w.page)).toEqual(before);
  expect((await g.status()).revision).toBe(rev);
});

test("Enter opens the box while a link is selected, and goes to the cell cursor once the link is deselected", async ({ g }) => {
  const w = await openLinks(g);
  await selectLinkOf(w, "A");
  await w.page.keyboard.press("Enter");
  await expect(w.page.locator(BOX), "a selected link takes Enter").toBeFocused();
  await expect(w.page.locator("input.cell-edit")).toHaveCount(0);
  await w.page.keyboard.press("Escape");
  await expect(w.page.locator(BOX)).toBeHidden();

  // A click on a row selects the row and clears the link: Enter is the cell cursor's again.
  await w.page.locator(`#table-rows .row[data-row="${w.row.A}"] .name`).click();
  await g.nextFrame(2);
  expect((await g.status()).link.selected).toBeNull();
  await w.page.keyboard.press("Enter");
  await expect(w.page.locator("input.cell-edit")).toBeVisible();
  await expect(w.page.locator(BOX)).toBeHidden();
});

test("clicking the find box while the box is open closes it and leaves focus in the find box", async ({ g }) => {
  const w = await openLinks(g);
  const dep = await selectLinkOf(w, "A");
  await w.page.keyboard.press("Enter");
  await expect(w.page.locator(BOX)).toBeFocused();
  await w.page.locator("#find-input").click();
  await expect(w.page.locator(BOX)).toBeHidden();
  await expect(w.page.locator("#find-input"), "the find box keeps the focus it was given").toBeFocused();
  expect(await linkText(w.page, dep)).toBe("FS");
});

test("a type that repeats another link between the same two tasks is refused with the reason and changes nothing", async ({ g }) => {
  // The box cannot close a loop (it changes a live link's type and lag only), but a file can hold two links between a pair with
  // different types: here A to C is finish-to-start and also start-to-start. Asking the first for SS repeats the second.
  const both = fixture
    .toString()
    .replace(
      "<PredecessorUID>2</PredecessorUID><Type>1</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink>",
      "<PredecessorUID>2</PredecessorUID><Type>1</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink>" +
        "<PredecessorLink><PredecessorUID>2</PredecessorUID><Type>3</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink>",
    );
  expect(both).not.toBe(fixture.toString());
  const w = await openLinks(g, Buffer.from(both));
  let dep = await selectLinkOf(w, "A");
  if ((await linkText(w.page, dep)) !== "FS") {
    await w.page.keyboard.press("]");
    await g.nextFrame(2);
    dep = (await g.status()).link.selected!;
  }
  expect(await linkText(w.page, dep)).toBe("FS");
  const before = await datesOf(w.page);
  const rev = (await g.status()).revision;
  await w.page.keyboard.press("Enter");
  await w.page.keyboard.press("Control+a");
  await w.page.keyboard.type("SS");
  await w.page.keyboard.press("Enter");
  await g.nextFrame(2);
  await expect(w.page.locator("#edit-error")).toBeVisible();
  await expect(w.page.locator(ALERT), "the engine's reason for a repeated link").toHaveText(/is already linked to/);
  await expect(w.page.locator(BOX), "the box stays open for another try").toBeVisible();
  expect(await linkText(w.page, dep)).toBe("FS");
  expect(await datesOf(w.page)).toEqual(before);
  expect((await g.status()).revision).toBe(rev);
});

test("a link removed while the box is open closes it on Enter, with no refusal and no change", async ({ g }) => {
  const w = await openLinks(g);
  const dep = await selectLinkOf(w, "A");
  await w.page.keyboard.press("Enter");
  await expect(w.page.locator(BOX)).toBeFocused();
  await w.page.keyboard.press("Control+a");
  await w.page.keyboard.type("SS+2d");
  // A call through the API takes the link away without moving focus, so the box is still open over a link that is gone.
  await w.page.evaluate(() => window.gantt!.app.remove_selected_link());
  await g.nextFrame(3);
  expect(await linkText(w.page, dep), "the link is gone").toBeUndefined();
  await expect(w.page.locator(BOX)).toBeVisible();
  const before = await datesOf(w.page);
  const rev = (await g.status()).revision;

  await w.page.locator(BOX).press("Enter");
  await g.nextFrame(2);
  await expect(w.page.locator(BOX)).toBeHidden();
  await expect(w.page.locator("#edit-error"), "a gone link is not a refusal").toBeHidden();
  expect(await datesOf(w.page)).toEqual(before);
  expect((await g.status()).revision).toBe(rev);
});

test("a file opened while the box is open closes it on Enter and applies nothing to the new file's link", async ({ g }) => {
  const w = await openLinks(g);
  const dep = await selectLinkOf(w, "A");
  await w.page.keyboard.press("Enter");
  await expect(w.page.locator(BOX)).toBeFocused();
  await w.page.keyboard.press("Control+a");
  await w.page.keyboard.type("SS+2d");
  // Opening a file gives the chart focus, and the box closes on that blur. Hold the blur back so the box stays open over the new
  // file, as it would for a route that moves no focus.
  await w.page.evaluate(() => {
    window.addEventListener("blur", (e) => { if ((e.target as Element | null)?.id === "link-edit-input") e.stopImmediatePropagation(); }, true);
  });
  await w.page.setInputFiles("#open-input", { name: "links.xml", mimeType: "application/xml", buffer: fixture });
  await w.page.waitForFunction(() => window.gantt!.status().epoch === 2, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await expect(w.page.locator(BOX), "the box is still open over the new file").toBeVisible();
  expect(await linkText(w.page, dep), "the id still names a link in the new file, so only the epoch tells them apart").toBe("FS");
  const before = await datesOf(w.page);
  const rev = (await g.status()).revision;

  await w.page.locator(BOX).press("Enter");
  await g.nextFrame(2);
  await expect(w.page.locator(BOX)).toBeHidden();
  expect(await linkText(w.page, dep), "the text was not applied to the new file's link").toBe("FS");
  expect(await datesOf(w.page)).toEqual(before);
  expect((await g.status()).revision).toBe(rev);
});

test("with a link selected, Enter on a focused toolbar button presses the button and opens no box", async ({ g }) => {
  const w = await openLinks(g);
  await selectLinkOf(w, "A");
  await openFilters(w.page);
  await w.page.focus("#critical-toggle");
  await expect(w.page.locator("#critical-toggle")).toHaveAttribute("aria-pressed", "false");
  await w.page.keyboard.press("Enter");
  await g.nextFrame(2);
  await expect(w.page.locator("#critical-toggle"), "the button took its Enter").toHaveAttribute("aria-pressed", "true");
  await expect(w.page.locator(BOX)).toBeHidden();
});
