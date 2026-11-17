import { readFileSync } from "node:fs";
import { expect, test, type Gantt } from "./support/gantt";

// The card from the keyboard (gs#703): with a row selected, Alt+Enter shows the hover card of that row under the row. Escape, any
// other key or a pointer move over the canvas hides it. The pointer stays outside the canvas, so no hover card can show instead.

const cell = (g: Gantt, row: number, column: string) => g.page.locator(`#table-rows .row[data-row="${row}"] > .${column}`);

const card = (g: Gantt) => g.page.locator("#hover-card");

/** Click a row's Dur cell, then park the pointer outside the canvas. */
async function selectRow(g: Gantt, row: number): Promise<number> {
  await cell(g, row, "dur").click();
  await g.page.mouse.move(1, 1);
  await g.nextFrame(2);
  const selected = (await g.status()).selected;
  expect(selected, "the click selected a row").not.toBeNull();
  return selected!.task;
}

test("alt_enter_shows_the_card_of_the_selected_row_and_escape_hides_it", async ({ g }) => {
  await g.open({ n: 200 });
  const task = await selectRow(g, 3);
  await expect(card(g), "no card before the key").toBeHidden();

  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(2);
  await expect(card(g)).toBeVisible();
  await expect(card(g)).toHaveAttribute("data-task", String(task));

  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  await expect(card(g)).toBeHidden();
});

// gs#975: the pinned card must not hide the task names of the rows the cursor moves to next.
test("the_pinned_card_does_not_cover_the_task_cells_of_the_selected_row_and_the_two_below", async ({ g }) => {
  await g.open({ n: 200 });
  await selectRow(g, 3);
  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(2);
  await expect(card(g)).toBeVisible();

  const cardBox = (await card(g).boundingBox())!;
  for (const row of [3, 4, 5]) {
    const name = (await cell(g, row, "name").boundingBox())!;
    const apart = cardBox.x >= name.x + name.width || cardBox.x + cardBox.width <= name.x || cardBox.y >= name.y + name.height || cardBox.y + cardBox.height <= name.y;
    expect(apart, `the card ${JSON.stringify(cardBox)} stays off the Task cell of row ${row} ${JSON.stringify(name)}`).toBe(true);
  }
});

test("any_other_key_hides_the_pinned_card", async ({ g }) => {
  await g.open({ n: 200 });
  await selectRow(g, 3);
  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(2);
  await expect(card(g)).toBeVisible();

  await g.page.keyboard.press("ArrowDown");
  await g.nextFrame(2);
  await expect(card(g)).toBeHidden();
});

test("a_pointer_move_over_the_canvas_hides_the_pinned_card", async ({ g }) => {
  await g.open({ n: 200 });
  await selectRow(g, 3);
  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(2);
  await expect(card(g)).toBeVisible();

  const canvas = await g.box();
  // The axis band holds no bar, so no hover card replaces the pinned one.
  await g.page.mouse.move(canvas.x + 20, canvas.y + 4);
  await g.nextFrame(2);
  await expect(card(g)).toBeHidden();
});

// gs#703: a screen reader gets word that the card opened: Alt+Enter fills a visually hidden status region, Escape empties it.
test("alt_enter_announces_the_card_in_the_status_region_and_escape_empties_it", async ({ g }) => {
  await g.open({ n: 200 });
  await selectRow(g, 3);
  const live = g.page.locator("#card-live");
  await expect(live).toHaveAttribute("role", "status");

  // The click said the row; ArrowRight empties the region and keeps the selection, so only Alt+Enter can fill it again.
  await g.page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  await expect(live).toHaveText("");
  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(2);
  const name = (await cell(g, 3, "name").innerText()).trim();
  await expect(live).toContainText(name);
  await expect(live).toContainText(/Start .*, finish .*, float .*, critical (yes|no)/);

  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  await expect(live).toHaveText("");
});

test("hover_leaves_the_status_region_silent", async ({ g }) => {
  await g.open({ n: 200 });
  await selectRow(g, 3);
  const said = await g.page.locator("#card-live").textContent();
  const bar = await g.box();
  await g.page.mouse.move(bar.x + 20, bar.y + 4);
  await g.nextFrame(2);
  await expect(g.page.locator("#card-live")).toHaveText(said!);
});

// Each way out of the pinned card empties the status region as well (gs#703).
async function pinAndRead(g: Gantt): Promise<void> {
  await selectRow(g, 3);
  await g.page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  await expect(g.page.locator("#card-live")).toHaveText("");
  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(2);
  await expect(g.page.locator("#card-live"), "announced after the key").not.toHaveText("");
}

test("a_pointer_move_over_the_canvas_empties_the_status_region", async ({ g }) => {
  await g.open({ n: 200 });
  await pinAndRead(g);
  const canvas = await g.box();
  await g.page.mouse.move(canvas.x + 20, canvas.y + 4);
  await g.nextFrame(2);
  await expect(g.page.locator("#card-live")).toHaveText("");
});

test("any_other_key_empties_the_status_region", async ({ g }) => {
  await g.open({ n: 200 });
  await pinAndRead(g);
  await g.page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  await expect(g.page.locator("#card-live")).toHaveText("");
});

test("opening_a_new_schedule_empties_the_status_region", async ({ g }) => {
  await g.open({ n: 200 });
  await pinAndRead(g);
  await g.page.setInputFiles("#open-input", {
    name: "one-project.xer",
    mimeType: "application/octet-stream",
    buffer: readFileSync(new URL("./fixtures/one-project.xer", import.meta.url)),
  });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await expect(g.page.locator("#card-live")).toHaveText("");
});

test("a_pinned_summary_row_is_announced_with_only_start_and_finish", async ({ g }) => {
  await g.open({ n: 200 });
  const row = await g.page.evaluate(() => Number(document.querySelector<HTMLElement>('#table-rows .row[data-kind="1"]')?.dataset.row ?? -1));
  expect(row, "a summary row in view").toBeGreaterThanOrEqual(0);
  const task = await selectRow(g, row);
  const details = await g.page.evaluate((t) => window.gantt!.app.task_details(t) as { name: string; summary: boolean; start: string; finish: string }, task);
  expect(details.summary, "the selected row is a summary").toBe(true);

  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(2);
  await expect(g.page.locator("#card-live")).toHaveText(`${details.name}. Start ${details.start}, finish ${details.finish}`);
});

// gantt#1152: a cell edit drops the pin, so the card does not come back when the edit closes.
test("a_cell_edit_drops_the_pinned_card_for_good", async ({ g }) => {
  await g.open({ n: 200 });
  await selectRow(g, 3);
  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(2);
  await expect(card(g)).toBeVisible();

  await cell(g, 5, "dur").dblclick();
  await g.nextFrame(2);
  await expect(g.page.locator("#table-rows input.cell-edit")).toBeVisible();
  await g.page.keyboard.press("Escape");
  await g.nextFrame(3);
  await expect(card(g), "the edit closed without the card coming back").toBeHidden();
});

// gantt#1152: a pinned card with its row not drawn shows nowhere, not in the corner.
test("a_pin_with_its_row_scrolled_out_of_the_table_shows_no_card", async ({ g }) => {
  await g.open({ n: 200 });
  await selectRow(g, 3);
  await g.page.evaluate(() => (document.getElementById("table-body")!.scrollTop = 1e6));
  await g.nextFrame(3);
  await g.page.keyboard.press("Alt+Enter");
  await g.nextFrame(3);
  await expect(card(g), "no card without a row to sit beside").toBeHidden();
});
