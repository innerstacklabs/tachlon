import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The Checks button (gs#703) opens a dialog with one line for each schedule-quality count: its name, its number and one plain sentence.
// The file is `links.xml`, hand-counted in packages/gantt/tests/checks-report.spec.ts: four tasks missing logic (two open starts, two open ends), no leads or lags, three
// links and all three finish to start, nothing high, no resource, no data date and no baseline. Measured at 1280 x 800, where the toolbar has no room to spare.

test.use({ viewport: { width: 1280, height: 800 } });

const LINKS = readFileSync(new URL("../../../packages/gantt/tests/fixtures/links.xml", import.meta.url), "utf8");

// Three tasks of a day each, two with a resource (Keel, Deck) and one without (Mast): Missing resources reads 1.
const RESOURCED =
  '<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-01-07T08:00:00</StartDate><Tasks>' +
  ["Keel", "Deck", "Mast"].map((name, i) => `<Task><UID>${i + 1}</UID><Name>${name}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>`).join("") +
  '</Tasks><Resources><Resource><UID>1</UID><Name>Crew A</Name></Resource></Resources><Assignments>' +
  [1, 2].map((task) => `<Assignment><TaskUID>${task}</TaskUID><ResourceUID>1</ResourceUID></Assignment>`).join("") +
  "</Assignments></Project>";

const LONG =
  '<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-01-07T08:00:00</StartDate><Tasks>' +
  ["PT8H0M0S", "PT1104H0M0S"].map((d, i) => `<Task><UID>${i + 1}</UID><Name>T${i}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>${d}</Duration>${i ? "<ConstraintType>7</ConstraintType><ConstraintDate>2027-12-31T17:00:00</ConstraintDate>" : ""}</Task>`).join("") +
  "</Tasks></Project>";

/** Open `xml` as a file and wait until the chart has swapped it in. */
async function openXml(page: Page, xml: string): Promise<void> {
  const from = await page.evaluate(() => window.gantt!.status().epoch);
  await page.setInputFiles("#open-input", { name: "links.xml", mimeType: "application/xml", buffer: Buffer.from(xml) });
  await page.waitForFunction((n) => window.gantt!.status().epoch === n, from + 1, { timeout: 60_000 });
}

/** Press the Checks button and read the dialog: each line's name, number and sentence, in order. */
async function readChecks(page: Page): Promise<string[][]> {
  await page.locator("#checks-button").click();
  await expect(page.locator("#checks-dialog")).toBeVisible();
  const rows = await page.locator("#checks-list tr").evaluateAll((trs) => trs.map((tr) => [...tr.querySelectorAll("td")].map((td) => td.textContent ?? "")));
  await page.locator("#checks-close").click();
  await expect(page.locator("#checks-dialog")).toBeHidden();
  return rows;
}

test("the_checks_dialog_lists_each_count_and_says_when_no_data_date_is_set", async ({ page }) => {
  // The data date is an unsaved edit, so opening the next file asks first.
  page.on("dialog", (dialog) => void dialog.accept());
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await openXml(page, LINKS);

  const rows = await readChecks(page);
  expect(rows.map(([name, value]) => [name, value])).toEqual([
    ["Missing logic", "4"],
    ["Open starts", "2"],
    ["Open ends", "2"],
    ["Leads", "0"],
    ["Lags", "0"],
    ["Finish to start", "3 of 3 links"],
    ["High float", "0"],
    ["High duration", "0"],
    ["Hard constraints", "0"],
    ["Negative float", "0"],
    ["Invalid dates", "needs a data date"],
    ["Missing resources", "needs resources"],
    ["Missed", "0"],
    ["Execution index", "none due"],
  ]);
  for (const [name, , says] of rows) expect(says.length, `${name} has a sentence`).toBeGreaterThan(20);

  // A data date makes the check run: the count is a number now, and a file with no links says so.
  await page.evaluate(() => window.gantt!.api!.commands.setDataDate("2026-01-07"));
  expect((await readChecks(page)).find(([name]) => name === "Invalid dates")!.slice(0, 2)).toEqual(["Invalid dates", "0"]);
  // A baseline and a data date after every baselined finish: none of the five leaves has finished, so all five missed, index 0.
  await page.evaluate(() => [window.gantt!.api!.commands.setBaseline(), window.gantt!.api!.commands.setDataDate("2026-01-31")]);
  expect((await readChecks(page)).slice(-2).map(([name, value]) => [name, value])).toEqual([["Missed", "5"], ["Execution index", "0.00"]]);
  await openXml(page, LINKS.replace(/<PredecessorLink>.*?<\/PredecessorLink>/g, ""));
  expect((await readChecks(page)).find(([name]) => name === "Finish to start")![1]).toBe("no links");

  // A file that names a resource makes the check run: Mast has none, so the count is 1 and no longer a sentence.
  await openXml(page, RESOURCED);
  expect((await readChecks(page)).find(([name]) => name === "Missing resources")!.slice(0, 2)).toEqual(["Missing resources", "1"]);
});

// Ten lines can have a Show button (here three do): it sets the Check filter to that check and closes the dialog. In links.xml Open ends keeps the summary Phase and the open ends D and E.
test("Show_on_Open_ends_closes_the_dialog_and_keeps_the_tasks_with_nothing_after_them", async ({ page }) => {
  page.on("dialog", (dialog) => void dialog.accept());
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await openXml(page, LINKS);
  const all = await page.evaluate(() => window.gantt!.status().rowCount);

  await page.locator("#checks-button").click();
  const shows = page.locator("#checks-list [data-part=checks-show]");
  await expect(shows).toHaveCount(3);
  expect(await shows.evaluateAll((buttons) => buttons.map((b) => b.closest("tr")!.querySelector("td")!.textContent))).toEqual(["Missing logic", "Open starts", "Open ends"]);
  await page.getByRole("button", { name: "Show Open ends" }).click();
  await expect(page.locator("#checks-dialog")).toBeHidden();
  expect(await page.evaluate(() => window.gantt!.status().filter)).toBe("check");
  await expect(page.locator("#row-count")).toHaveText(`Check = Open ends · 3 of ${all} rows`);
});

test("Show_on_High_float_keeps_the_tasks_with_float_over_44_days", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await openXml(page, LONG);
  await page.locator("#checks-button").click();
  await expect(page.locator("#checks-list tr", { hasText: "High float" }).locator("td").nth(1)).toHaveText("1");
  await page.getByRole("button", { name: "Show High float" }).click();
  await expect(page.locator("#checks-dialog")).toBeHidden();
  expect(await page.evaluate(() => window.gantt!.status().filter)).toBe("check");
  await expect(page.locator("#row-count")).toHaveText("Check = High float · 1 of 2 rows");
});

test("Show_on_Hard_constraints_keeps_the_tasks_a_fixed_date_holds", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await openXml(page, LONG);
  await page.locator("#checks-button").click();
  await page.getByRole("button", { name: "Show Hard constraints" }).click();
  await expect(page.locator("#row-count")).toHaveText("Check = Hard constraints · 1 of 2 rows");
});

// A line whose count is 0 has no Show button: on the office fit-out Missing logic reads 1, Open starts 0, Open ends 1 and Missed 1, so three lines have one.
test("a_check_line_that_counts_0_has_no_Show_button", async ({ page }) => {
  await page.goto("/?sample=office-fitout");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);

  await page.locator("#checks-button").click();
  const rows = await page.locator("#checks-list tr").evaluateAll((trs) => trs.slice(0, 3).map((tr) => [...tr.querySelectorAll("td")].slice(0, 2).map((td) => td.textContent ?? "")));
  expect(rows).toEqual([["Missing logic", "1"], ["Open starts", "0"], ["Open ends", "1"]]);
  await expect(page.locator("#checks-list [data-part=checks-show]")).toHaveCount(3);
  await expect(page.getByRole("button", { name: "Show Open starts" })).toHaveCount(0);
});

// A line with a problem says why it matters, under its sentence; a line that counts 0 says no more. On the office fit-out the first six lines read 1, 0, 1, 0, 7 and 63 of 69 links.
test("a_flagged_check_line_says_why_and_a_line_that_counts_0_does_not", async ({ page }) => {
  await page.goto("/?sample=office-fitout");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);

  await page.locator("#checks-button").click();
  const why = await page.locator("#checks-list tr").evaluateAll((trs) => trs.slice(0, 6).map((tr) => tr.querySelector("[data-part=checks-why]")?.textContent ?? null));
  expect(why).toEqual(["Nothing moves these tasks when a task before them slips.", null, "Nothing comes after these tasks, so their finish pushes nothing.", null, "A wait is hidden inside a link; a task for the wait shows it.", "Other link types are harder to follow, so most links are usually finish to start."]);
});
