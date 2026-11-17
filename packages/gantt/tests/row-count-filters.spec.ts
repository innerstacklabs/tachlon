import { expect, openFilters, test } from "./support/gantt";
import { editDuration, openLedger } from "./support/moved-note";

// The row count names every filter that is on (gs#703): Near critical, Float lost, Joined critical, Left critical and the Status
// choice join Critical only, Longest path, Slipped, Moved, the activity code and Lookahead in the label, as `Near critical · 410
// of 2,000 rows`. Which rows each keeps is checked in the spec of its own button; this checks only the words in `#row-count`,
// including the word that ends it: Near critical counts activities, the other three count rows.
const NAMES: readonly [button: string, name: string, unit: "rows" | "activities"][] = [
  ["#near-critical-toggle", "Near critical", "activities"],
  ["#float-lost-toggle", "Float lost", "rows"],
  ["#joined-critical-toggle", "Joined critical", "rows"],
  ["#left-critical-toggle", "Left critical", "rows"],
];

test("the row count names Near critical, Float lost, Joined critical and Left critical", async ({ g }) => {
  test.setTimeout(120_000);
  const { page } = g;
  await openLedger(g);
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  // Slip the edited task so the baseline has something to compare: float lost, and tasks joining and leaving the critical path.
  await editDuration(page, "40d");
  await g.nextFrame(3);

  for (const [button, name, unit] of NAMES) {
    await openFilters(page);
    await page.locator(button).click();
    await g.nextFrame(2);
    const text = (await page.locator("#row-count").textContent()) ?? "";
    expect(text, button).toContain(`${name} · `);
    expect(text, button).toContain(" of ");
    expect(text, button).toMatch(new RegExp(` ${unit}$`));
  }
});

test("the row count names the Status choice as Status = In progress", async ({ g }) => {
  const { page } = g;
  await openLedger(g);
  await openFilters(page);
  await page.locator("#status-filter").selectOption("not-started");
  await g.nextFrame(2);
  const text = (await page.locator("#row-count").textContent()) ?? "";
  expect(text).toContain("Status = Not started · ");
  expect(text).toContain(" of ");
  await openFilters(page);
  await page.locator("#status-filter").selectOption("in-progress");
  await g.nextFrame(2);
  expect((await page.locator("#row-count").textContent()) ?? "").toContain("Status = In progress · ");
});

// With Critical only, Near critical or Longest path on, the header counts the activities the filter keeps, as Slipped counts its
// tasks (gs#858), not the rows, which also hold the summaries above them.
const number = new Intl.NumberFormat("en-US");
for (const [button, name] of [
  ["#critical-toggle", "Critical only"],
  ["#near-critical-toggle", "Near critical"],
  ["#longest-toggle", "Longest path"],
] as const) {
  test(`with ${name} on, the header counts activities, not rows`, async ({ g }) => {
    await g.open({ n: 300 });
    await openFilters(g.page);
    await g.page.locator(button).click();
    await g.nextFrame(3);
    const s = await g.status();
    expect(s.keptLeaves, "the status holds the kept activities").toBeGreaterThan(0);
    expect(s.keptLeaves, "the rows also hold summaries").toBeLessThan(s.rowCount);
    await expect(g.page.locator("#row-count")).toHaveText(`${name} · ${number.format(s.keptLeaves!)} of ${number.format(s.n)} activities`);
  });
}

// With Longest path on and every activity finished before the status date, no activity is on the path: the header says so, as
// `Longest path · 0 of 3 activities` (gantt#703), and the line at the chart's bottom left stays empty.
test("with Longest path on and no path, the header counts 0 activities", async ({ g }) => {
  // One summary over two tasks that finished before the status date: the summary sits at the status date, the only task that finishes at the project finish.
  const done = (uid: number, day: number): string =>
    `<Task><UID>${uid}</UID><Name>Done ${uid}</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration><ActualStart>2026-01-0${day}T08:00:00</ActualStart><ActualFinish>2026-01-0${day}T16:00:00</ActualFinish></Task>`;
  const body = `<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-01-05T08:00:00</StartDate><StatusDate>2026-02-02T08:00:00</StatusDate><Tasks><Task><UID>1</UID><Name>Phase</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary></Task>${done(2, 5)}${done(3, 6)}</Tasks></Project>`;
  await g.page.route("**/schedule.dat", (route) => route.fulfill({ status: 200, contentType: "application/xml", body }));
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await openFilters(g.page);
  await g.page.locator("#longest-toggle").click();
  await g.nextFrame(3);
  const s = await g.status();
  expect(s.keptLeaves, "no activity is kept").toBe(0);
  expect(s.path, "no path to name").toBeNull();
  await expect(g.page.locator("#row-count")).toHaveText(`Longest path · 0 of ${number.format(s.n)} activities`);
});
