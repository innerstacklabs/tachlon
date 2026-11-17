import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

// The keys list (gs#703): `?` with the chart focused, or the toolbar's `?` button, opens a dialog of the keys a scheduler uses most;
// Escape or Close shuts it and focus goes back to the chart.

const rootFocused = (page: import("@playwright/test").Page): Promise<boolean> =>
  page.evaluate(() => document.activeElement === document.getElementById("app"));

test("question_mark_opens_the_keys_list_and_escape_closes_it", async ({ g }) => {
  await g.open({ n: 2_000 });
  const dialog = g.page.locator("#keys-help");
  await g.page.evaluate(() => document.getElementById("app")!.focus());

  await g.page.keyboard.press("?");
  await expect(dialog, "? opens the dialog").toBeVisible();
  await expect(dialog.getByRole("heading", { name: "Keyboard shortcuts" })).toBeVisible();
  const keys = await dialog.locator("td:first-child").allTextContents();
  expect(keys, "a row for Ctrl+F").toContain("Ctrl+F");
  expect(keys, "a row for Shift+C").toContain("Shift+C");
  expect(keys, "a row for Shift+Enter, the task dialog's key").toContain("Shift+Enter");
  await expect(dialog.getByRole("link", { name: "All keys" }), "the last line links to the book's keys page").toHaveAttribute("href", /keys\.html$/);

  await g.page.keyboard.press("Escape");
  await expect(dialog, "Escape closes it").toBeHidden();
  expect(await rootFocused(g.page), "focus is back on the chart").toBe(true);

  await g.page.locator("#keys-button").click();
  await expect(dialog, "the toolbar button opens it again").toBeVisible();
  await dialog.getByRole("button", { name: "Close" }).click();
  await expect(dialog, "Close shuts it").toBeHidden();
  expect(await rootFocused(g.page), "focus is back on the chart after Close").toBe(true);

  await g.page.locator("#table-rows .row[data-row=\"2\"] > .dur").click();
  await g.nextFrame(2);
  await g.page.keyboard.press("?");
  await expect(dialog, "with the cell cursor in the list, ? types into the cell").toBeHidden();
  await expect(g.page.locator("#table-rows input"), "the cell editor holds the ?").toHaveValue("?");
});

test("every_key_the_dialog_names_is_in_the_readme_table", async ({ g }) => {
  await g.open({ n: 2_000 });
  await g.page.locator("#keys-button").click();
  const keys = await g.page.locator("#keys-help td:first-child").allTextContents();
  expect(keys.length, "the dialog lists keys").toBeGreaterThan(0);
  const readme = readFileSync(new URL("../../../README.md", import.meta.url), "utf8");
  const table = readme.slice(readme.indexOf("<!-- ANCHOR: keys -->"), readme.indexOf("<!-- ANCHOR_END: keys -->"));
  expect(table.length, "the README keys table was found").toBeGreaterThan(100);
  for (const key of keys) expect(table, `the README table names ${key}`).toContain(key);
});
