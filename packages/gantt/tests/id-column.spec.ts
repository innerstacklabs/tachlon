// The ID column and the data date in the toolbar (gs#87 PR 4). A file with activity IDs gets a first "ID" column;
// a file without them (MS Project XML, the demo) keeps the table it had. Expected IDs come from the fixture's own
// TASK table, read here, not from the app.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const MSPDI = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));
const PROGRESS_HEAD = ["Actual start", "Actual finish", "Remaining", "% complete"];
const PROGRESS_CLASSES = ["actual-start", "actual-finish", "remaining", "percent"];

/** The ID track the page set from the file's longest ID (`--gantt-id-col` on the root, in px), plus its 6px gap. */
const idTrack = (page: Page): Promise<number> =>
  page.evaluate(() => parseFloat(getComputedStyle(document.getElementById("app")!).getPropertyValue("--gantt-id-col")));

/** `task_name` to `task_code` for every activity of the XER fixture, read from its TASK table. */
function codesByName(text: string): Map<string, string> {
  const codes = new Map<string, string>();
  let inTask = false;
  let nameAt = -1;
  let codeAt = -1;
  for (const line of text.split(/\r?\n/)) {
    const cells = line.split("\t");
    if (cells[0] === "%T") inTask = cells[1] === "TASK";
    else if (cells[0] === "%F" && inTask) {
      nameAt = cells.indexOf("task_name");
      codeAt = cells.indexOf("task_code");
    } else if (cells[0] === "%R" && inTask) codes.set(cells[nameAt]!, cells[codeAt]!);
  }
  return codes;
}
const EXPECTED = codesByName(XER.toString("latin1"));

async function openFile(g: Gantt, name: string, buffer: Buffer, epoch: number): Promise<void> {
  await g.page.setInputFiles("#open-input", { name, mimeType: "application/octet-stream", buffer });
  await g.page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
}

interface TableFacts {
  ids: string | undefined;
  head: string[];
  rows: { kind: string; cells: string[]; classes: string[] }[];
  tableWidth: number;
  firstTrack: number;
}

/** What the table shows right now: the head cells and, for every drawn row, its cells in order. */
async function readTable(page: Page): Promise<TableFacts> {
  return page.evaluate(() => {
    const table = document.getElementById("table")!;
    const head = [...document.querySelectorAll("#table-head .cols > span")].map((cell) => cell.textContent ?? "");
    const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row")].map((row) => ({
      kind: row.dataset.kind ?? "",
      cells: [...row.children].map((cell) => cell.textContent ?? ""),
      classes: [...row.children].map((cell) => cell.className),
    }));
    const firstCell = document.querySelector<HTMLElement>("#table-rows .row > span");
    return {
      ids: table.dataset.ids,
      head,
      rows,
      tableWidth: table.getBoundingClientRect().width,
      firstTrack: firstCell === null ? 0 : firstCell.getBoundingClientRect().width,
    };
  });
}

test("a file with activity IDs gets an ID column, and a file without them takes it away again", async ({ g }) => {
  await g.open({ n: 200 });
  const demo = await readTable(g.page);
  expect(demo.ids, "the demo has no activity IDs").toBeUndefined();
  expect(demo.head).toEqual(["Task", "Dur", "Start", "Finish", "Float"]);
  expect(demo.rows.length).toBeGreaterThan(5);
  for (const row of demo.rows) expect(row.classes, "no code cell in a demo row").toEqual(["name", "dur", "start", "finish", "float"]);
  await expect(g.page.locator("#data-date"), "the demo has no data date").toBeHidden();

  await openFile(g, "ledger-timeline.xer", XER, 1);
  const xer = await readTable(g.page);
  expect(xer.ids).toBe("true");
  // The fixture also has WBS short names, so the WBS column (wbs-column.spec.ts) sits between the ID and the name, and it has actuals, so
  // the four progress columns follow the float (table-layout.spec.ts holds how they share the pane).
  expect(xer.head).toEqual(["ID", "WBS", "Task", "Dur", "Start", "Finish", "Float", ...PROGRESS_HEAD]);
  expect(xer.rows.length).toBeGreaterThan(5);
  let activities = 0;
  for (const row of xer.rows) {
    expect(row.classes).toEqual(["code", "wbs", "name", "dur", "start", "finish", "float", ...PROGRESS_CLASSES]);
    const [code, , name] = row.cells as [string, string, string];
    if (row.kind === "0") {
      // A leaf is an activity: its ID is the one the fixture's TASK table gives that activity.
      expect(EXPECTED.has(name), `${name} is an activity of the fixture`).toBe(true);
      expect(code, `the ID of ${name}`).toBe(EXPECTED.get(name));
      activities += 1;
    } else {
      expect(code, `the summary ${name} has no ID`).toBe("");
    }
  }
  expect(activities, "the window holds activities").toBeGreaterThan(0);
  expect(xer.firstTrack, "the ID track").toBeCloseTo(await idTrack(g.page), 0);
  // The default width is half the box with the track in it (gs#866), so the table does not grow: the names give up the track and its gap.
  expect(xer.tableWidth, "the table keeps its half of the box").toBeCloseTo(demo.tableWidth, 0);

  // A file with no codes after one with codes: the column goes, and the table is the demo's again.
  await openFile(g, "summary.xml", MSPDI, 2);
  const mspdi = await readTable(g.page);
  expect(mspdi.ids, "an MSPDI file has no IDs").toBeUndefined();
  expect(mspdi.head).toEqual(["Task", "Dur", "Start", "Finish", "Float"]);
  expect(mspdi.rows.length).toBeGreaterThan(0);
  for (const row of mspdi.rows) expect(row.classes).toEqual(["name", "dur", "start", "finish", "float"]);
  expect(mspdi.tableWidth).toBeCloseTo(demo.tableWidth, 0);

  // And back: the column returns for the next XER file.
  await openFile(g, "ledger-timeline.xer", XER, 3);
  const again = await readTable(g.page);
  expect(again.ids).toBe("true");
  expect(again.head[0]).toBe("ID");
  expect(await g.page.evaluate(() => window.gantt!.app.has_codes())).toBe(true);
});

test("after a scroll every leaf row still shows its own ID", async ({ g }) => {
  await g.open({ n: 200 });
  await openFile(g, "ledger-timeline.xer", XER, 1);
  // Scroll the table; the rows are redrawn from the new window and each leaf still shows its own ID.
  await g.page.evaluate(() => {
    document.getElementById("table-body")!.scrollTop = 200;
  });
  await g.nextFrame(4);
  const scrolled = await readTable(g.page);
  expect(scrolled.rows.length).toBeGreaterThan(5);
  for (const row of scrolled.rows) {
    if (row.kind !== "0") continue;
    expect(row.cells[0], `the ID of ${row.cells[2]}`).toBe(EXPECTED.get(row.cells[2]!));
  }
});

test("Ctrl+F finds an activity by its ID", async ({ g }) => {
  await g.open({ n: 200 });
  await openFile(g, "ledger-timeline.xer", XER, 1);
  await g.page.locator("#find-input").fill("A1-17");
  await expect(g.page.locator("#find-count")).toHaveText(/^1 of \d+$/);
  const selected = await g.page.evaluate(() => window.gantt!.status().selected?.task ?? null);
  expect(selected, "the match is selected").not.toBeNull();
  expect(await g.page.evaluate((task) => window.gantt!.app.task_code(task as number), selected)).toBe("A1-17");
});

test("the toolbar shows the file's data date, and nothing for a file without one", async ({ g }) => {
  await g.open({ n: 200 });
  await expect(g.page.locator("#data-date")).toBeHidden();

  await openFile(g, "ledger-timeline.xer", XER, 1);
  // The file says 2024-01-10 08:00, day 9 from its origin Monday 2024-01-01 (ledger-timeline.spec.ts), and the toolbar
  // writes that date (date-origin.spec.ts).
  await expect(g.page.locator("#data-date")).toBeVisible();
  await expect(g.page.locator("#data-date")).toHaveText("Data date 2024-01-10 08:00");
  const status = await g.status();
  expect(status.dataDate, "the text and the status are the same instant").toBe(9 * 1440 + 8 * 60);
  expect(status.dataDateX, "the line is on the chart").not.toBeNull();

  await openFile(g, "summary.xml", MSPDI, 2);
  await expect(g.page.locator("#data-date")).toBeHidden();
  await expect(g.page.locator("#data-date")).toHaveText("");
  expect((await g.status()).dataDate).toBeNull();

  expect(await g.page.evaluate(() => window.gantt!.app.data_date_text())).toBe("");
});

test("a file without IDs or a data date answers with empty text, not an error", async ({ g }) => {
  await g.open({ n: 200 });
  await openFile(g, "summary.xml", MSPDI, 1);
  expect(await g.page.evaluate(() => window.gantt!.app.has_codes())).toBe(false);
  expect(await g.page.evaluate(() => window.gantt!.app.task_code(0))).toBe("");
  expect(await g.page.evaluate(() => window.gantt!.app.task_code(1_000_000)), "past the schedule").toBe("");
});
