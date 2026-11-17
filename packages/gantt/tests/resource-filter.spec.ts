// The Resource filter (gs#703): one choice in the Filters panel, after Status, keeps the tasks that name one resource. The rule is
// checked in crates/app/tests/resource_filter.rs; this checks the choice, the rows, the row count and the Filters count.
import type { Page } from "@playwright/test";
import { expect, openFilters, test } from "./support/gantt";

// Hull holds Keel (Crew A) and Deck (A and Crew B); Fit-out holds Mast (B).
const task = (uid: number, name: string, level: number, summary: number): string =>
  `<Task><UID>${uid}</UID><Name>${name}</Name><OutlineLevel>${level}</OutlineLevel><Summary>${summary}</Summary>${summary ? "" : "<Duration>PT8H0M0S</Duration>"}</Task>`;
const assign = (t: number, r: number): string => `<Assignment><TaskUID>${t}</TaskUID><ResourceUID>${r}</ResourceUID></Assignment>`;
const PROJECT = Buffer.from(
  `<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-01-07T08:00:00</StartDate><Tasks>${task(1, "Hull", 1, 1)}${task(2, "Keel", 2, 0)}${task(3, "Deck", 2, 0)}${task(4, "Fit-out", 1, 1)}${task(5, "Mast", 2, 0)}</Tasks>` +
    `<Resources><Resource><UID>1</UID><Name>Crew A</Name></Resource><Resource><UID>2</UID><Name>Crew B</Name></Resource></Resources><Assignments>${assign(2, 1)}${assign(3, 1)}${assign(3, 2)}${assign(5, 2)}</Assignments></Project>`,
);

const leaves = (page: Page): Promise<string[]> =>
  page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0'] .name")].map((el) => (el.textContent ?? "").trim()));

test("the Resource choice keeps the tasks that name one resource and shows only for a file with resources", async ({ g }) => {
  const { page } = g;
  test.setTimeout(120_000);
  page.setDefaultTimeout(20_000);
  await g.open({ n: 50 });
  await openFilters(page);
  await expect(page.locator("#resource-filter")).toBeHidden(); // the demo names no resource
  await page.keyboard.press("Escape");
  await page.setInputFiles("#open-input", { name: "boat.xml", mimeType: "application/xml", buffer: PROJECT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await expect.poll(() => leaves(page)).toEqual(["Keel", "Deck", "Mast"]);
  const all = (await g.status()).rowCount;

  await openFilters(page);
  const choice = page.locator("#resource-filter");
  expect(await choice.locator("option").allTextContents()).toEqual(["Resource", "Crew A", "Crew B"]);
  const ids = await page.locator("#filters-panel > *").evaluateAll((els) => els.map((el) => el.id));
  expect(ids.slice(ids.indexOf("status-filter"), ids.indexOf("status-filter") + 5)).toEqual(["status-filter", "check-filter", "resource-filter", "group-by", "start-constraint-toggle"]);

  await choice.selectOption("Crew A");
  await expect.poll(() => leaves(page)).toEqual(["Keel", "Deck"]);
  await expect(page.locator("#filters-button")).toHaveText("Filters · 1");
  await expect(page.locator("#row-count")).toContainText("Resource = Crew A · ");
  expect(await g.status()).toMatchObject({ filter: "resource", keptLeaves: 2 });

  await openFilters(page);
  await choice.selectOption("Crew B");
  await expect.poll(() => leaves(page)).toEqual(["Deck", "Mast"]);

  // A host's setFilter moves the choice, and another filter takes it back to the first entry.
  await page.evaluate(() => window.gantt!.api!.commands.setFilter({ resource: "Crew A" }));
  await expect(choice).toHaveValue("Crew A");
  await openFilters(page);
  await page.locator("#status-filter").selectOption("not-started");
  await expect(choice).toHaveValue("");

  await openFilters(page);
  await choice.selectOption("Crew A");
  await openFilters(page);
  await choice.selectOption("");
  await expect.poll(async () => (await g.status()).filter).toBe("off");
  expect((await g.status()).rowCount).toBe(all);
  await expect(page.locator("#filters-button")).toHaveText("Filters");
});
