import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

// App.checks (gs#703): the schedule checks report as JSON text, for the Checks panel. What each count means and how it is made is
// checked in crates/app/tests/checks_report.rs; this file checks that the page can ask for it and that it reads the open file.
//
// The file is `fixtures/links.xml`, hand-counted: the summary Phase over five leaves, A -> C -> E and B -> D, every link finish to
// start with no lag, each task a few days long and the whole plan a week or less.
//   missing_logic    4  A and B have no predecessor, D and E no successor; C has both; Phase is a summary
//   open_starts      2  A and B have no predecessor
//   open_ends        2  D and E have no successor
//   leads            0  no link has a negative lag
//   lags             0  no link has a positive lag
//   links            3  A -> C, C -> E, B -> D
//   finish_to_start  3  all three
//   pin_broken_links 0  the file holds no pinned task
//   high_float       0  no leaf has more than 44 days of float
//   high_duration    0  no leaf is longer than 44 days
//   hard_constraints 0  the file holds no task to a date
//   negative_float   0  no task has float below 0
//   invalid_dates    null  the file has no data date, so the check never ran (a 0 would read as a clean check)
//   missing_resources null  the file names no resource, so the check never ran
//   missed          0  the file has no baseline
//   execution_index null  no baseline and no data date
const fixture = readFileSync(new URL("./fixtures/links.xml", import.meta.url));

test("app.checks reads the open file's counts", async ({ g }) => {
  await g.open({ n: 50 });
  await g.page.setInputFiles("#open-input", { name: "links.xml", mimeType: "application/xml", buffer: fixture });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  const report = await g.page.evaluate(() => window.gantt!.app.checks());
  expect(JSON.parse(report)).toEqual({
    missing_logic: 4,
    open_starts: 2,
    open_ends: 2,
    leads: 0,
    lags: 0,
    links: 3,
    finish_to_start: 3,
    pin_broken_links: 0,
    high_float: 0,
    high_duration: 0,
    hard_constraints: 0,
    negative_float: 0,
    invalid_dates: null,
    missing_resources: null,
    missed: 0,
    execution_index: null,
  });
});
