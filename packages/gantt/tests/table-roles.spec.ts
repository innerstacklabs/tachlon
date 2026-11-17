import { expect, test, type Gantt } from "./support/gantt";
import { n } from "./support/knobs";

// The task list is a tree grid for assistive technology (gs#703): a `treegrid` named "Task list" with its row and column counts, a head
// row of column headers, body rows indexed in the whole list (the head is row 1) and `aria-selected` on the selected row and
// the rows of a range. Read through the accessibility roles, the way a screen reader meets the list.

const SIZE = Math.min(n, 5000);

const rowAt = (g: Gantt, row: number) => g.page.locator(`#table-rows .row[data-row="${row}"]`);

/** The first drawn body row, as the table numbers it. */
const firstDrawn = (g: Gantt): Promise<number> =>
  g.page.evaluate(() => Number(document.querySelector<HTMLElement>("#table-rows .row[data-row]")!.dataset.row));

const drawnIndex = (g: Gantt): Promise<string | null> =>
  g.page.evaluate(() => document.querySelector("#table-rows .row[data-row]")!.getAttribute("aria-rowindex"));

test("the task list is a grid with row indexes and a selected row", async ({ g }) => {
  await g.open({ n: SIZE });
  const grid = g.page.getByRole("treegrid", { name: "Task list" });
  await expect(grid).toHaveCount(1);
  await expect(g.page.getByRole("columnheader")).toHaveText(["Task", "Dur", "Start", "Finish", "Float"]);
  await expect(grid).toHaveAttribute("aria-colcount", "5");

  let first = await firstDrawn(g);
  expect(await drawnIndex(g), "the first drawn row is after the head row").toBe(String(first + 2));

  await g.page.evaluate(() => window.gantt!.app.go_to_date("2027-06-01"));
  await g.nextFrame(3);
  first = await firstDrawn(g);
  expect(first, "the window moved down the list").toBeGreaterThan(0);
  expect(await drawnIndex(g), "the index follows the window").toBe(String(first + 2));

  const s = await g.status();
  await expect(grid).toHaveAttribute("aria-rowcount", String(s.rowCount + 1));

  await g.page.evaluate((r) => window.gantt!.app.set_scroll_y(r * 22), 0);
  await g.nextFrame(3);
  await rowAt(g, 3).locator("> .dur").click();
  await g.nextFrame(2);
  await expect(g.page.getByRole("row", { selected: true })).toHaveCount(1);
  await expect(g.page.getByRole("row", { selected: true })).toHaveAttribute("aria-rowindex", "5");

  await g.page.keyboard.press("Shift+ArrowDown");
  await g.page.keyboard.press("Shift+ArrowDown");
  await g.nextFrame(2);
  const picked = g.page.getByRole("row", { selected: true });
  await expect(picked).toHaveCount(3);
  await expect(picked.nth(0)).toHaveAttribute("aria-rowindex", "5");
  await expect(picked.nth(2)).toHaveAttribute("aria-rowindex", "7");
});

// gantt#1875: a screen reader hears the task the selection moves to, through the same region as Alt+Enter.
test("the selection moving down a row is said in the status region", async ({ g }) => {
  await g.open({ n: SIZE });
  await rowAt(g, 3).locator("> .dur").click();
  await g.nextFrame(2);
  await g.page.keyboard.press("ArrowDown");
  await g.nextFrame(3);
  const name = (await rowAt(g, 4).locator("> .name").innerText()).trim();
  await expect(g.page.locator("#card-live")).toContainText(`${name}. Start `);
});

test("a collapse changes the grid's row count", async ({ g }) => {
  await g.open({ n: SIZE });
  const grid = g.page.getByRole("treegrid", { name: "Task list" });
  const before = (await g.status()).rowCount;
  await expect(grid).toHaveAttribute("aria-rowcount", String(before + 1));
  await g.page.locator("#table-rows .row .disclosure").first().click();
  await g.nextFrame(3);
  const after = (await g.status()).rowCount;
  expect(after, "the collapse hid rows").toBeLessThan(before);
  await expect(grid).toHaveAttribute("aria-rowcount", String(after + 1));
});

test("a row carries its outline depth, and a summary its expanded state", async ({ g }) => {
  await g.open({ n: SIZE });
  const deep = g.page.getByRole("row", { level: 2 }).first();
  await expect(deep, "the list has a row two levels down").toBeVisible();
  const task = Number(await deep.getAttribute("data-task"));
  expect(await g.page.evaluate((t) => window.gantt!.app.task_outline_level(t), task), "aria-level is the outline depth").toBe(2);

  await expect(g.page.locator('#table-rows .row[data-kind="0"]').first(), "a leaf has no expanded state").not.toHaveAttribute("aria-expanded", /.*/);

  const row = Number(await g.page.getByRole("row", { expanded: true }).first().getAttribute("data-row"));
  await rowAt(g, row).locator(".disclosure").click();
  await g.nextFrame(3);
  await expect(rowAt(g, row)).toHaveAttribute("aria-expanded", "false");
  await rowAt(g, row).locator(".disclosure").click();
  await g.nextFrame(3);
  await expect(rowAt(g, row)).toHaveAttribute("aria-expanded", "true");
});
