// The WBS column (gs#703, story 13). A file with WBS codes gets a "WBS" column after the ID column; a file without them
// (the demo, an MS Project XML file with no WBS text) keeps the table it had. Expected codes come from the fixture's own
// PROJWBS and TASK tables, read here, not from the app.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const SUMMARY = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));
/** The summary fixture with a WBS text on its first summary and its first child (and none on the rest). */
const MSPDI_WBS = Buffer.from(
  SUMMARY.toString("utf8")
    .replace("<UID>1</UID><Name>First summary", "<UID>1</UID><WBS>1</WBS><Name>First summary")
    .replace("<UID>2</UID><Name>First child", "<UID>2</UID><WBS>1.1</WBS><Name>First child"),
);

/** Every table of an XER text as rows keyed by column name. */
function tables(text: string): Map<string, Record<string, string>[]> {
  const result = new Map<string, Record<string, string>[]>();
  let name = "";
  let fields: string[] = [];
  for (const line of text.split(/\r?\n/)) {
    const cells = line.split("\t");
    if (cells[0] === "%T") {
      name = cells[1]!;
      result.set(name, []);
    } else if (cells[0] === "%F") fields = cells.slice(1);
    else if (cells[0] === "%R") {
      result.get(name)!.push(Object.fromEntries(fields.map((field, i) => [field, cells[i + 1] ?? ""])));
    }
  }
  return result;
}

/** The leaf name of every task of the XER fixture to its WBS code: the short names from the root to its WBS node, joined by dots. */
function expectedCodes(text: string): Map<string, string> {
  const data = tables(text);
  const wbs = new Map(data.get("PROJWBS")!.map((row) => [row["wbs_id"]!, row]));
  const path = (id: string): string => {
    const row = wbs.get(id)!;
    const parent = row["parent_wbs_id"]!;
    return parent === "" ? row["wbs_short_name"]! : `${path(parent)}.${row["wbs_short_name"]}`;
  };
  const codes = new Map<string, string>();
  for (const row of data.get("PROJWBS")!) codes.set(row["wbs_name"]!, path(row["wbs_id"]!));
  for (const row of data.get("TASK")!) codes.set(row["task_name"]!, path(row["wbs_id"]!));
  // The project's own row is the root summary, named by the project's short name.
  const root = data.get("PROJWBS")!.find((row) => row["proj_node_flag"] === "Y")!;
  for (const row of data.get("PROJECT")!) codes.set(row["proj_short_name"]!, path(root["wbs_id"]!));
  return codes;
}
const EXPECTED = expectedCodes(XER.toString("utf8"));

async function openFile(g: Gantt, name: string, buffer: Buffer, epoch: number): Promise<void> {
  await g.page.setInputFiles("#open-input", { name, mimeType: "application/octet-stream", buffer });
  await g.page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
}

interface TableFacts {
  wbs: string | undefined;
  head: string[];
  rows: { cells: string[]; classes: string[] }[];
  tableWidth: number;
  wbsTrack: number;
}

/** What the table shows right now: the head cells and, for every drawn row, its cells in order. */
async function readTable(page: Page): Promise<TableFacts> {
  return page.evaluate(() => {
    const table = document.getElementById("table")!;
    return {
      wbs: table.dataset.wbs,
      head: [...document.querySelectorAll("#table-head .cols > span")].map((cell) => cell.textContent ?? ""),
      rows: [...document.querySelectorAll<HTMLElement>("#table-rows .row")].map((row) => ({
        cells: [...row.children].map((cell) => cell.textContent ?? ""),
        classes: [...row.children].map((cell) => cell.className),
      })),
      tableWidth: table.getBoundingClientRect().width,
      wbsTrack: document.querySelector<HTMLElement>("#table-rows .row > .wbs")?.getBoundingClientRect().width ?? 0,
    };
  });
}

test("the demo has no WBS column", async ({ g }) => {
  await g.open({ n: 200 });
  const demo = await readTable(g.page);
  expect(demo.wbs).toBeUndefined();
  expect(demo.head).toEqual(["Task", "Dur", "Start", "Finish", "Float"]);
  expect(await g.page.evaluate(() => window.gantt!.app.has_wbs_codes())).toBe(false);
  expect(await g.page.evaluate(() => window.gantt!.app.task_wbs_code(0))).toBe("");
});

test("an XER file's table reads each task's WBS code after the ID column, and the column goes with the file", async ({ g }) => {
  await g.open({ n: 200 });
  const demo = await readTable(g.page);
  await openFile(g, "ledger-timeline.xer", XER, 1);
  const xer = await readTable(g.page);
  expect(xer.wbs).toBe("true");
  expect(xer.head.slice(0, 3)).toEqual(["ID", "WBS", "Task"]);
  expect(xer.rows.length).toBeGreaterThan(0);
  let leaves = 0;
  for (const row of xer.rows) {
    expect(row.classes.slice(0, 3)).toEqual(["code", "wbs", "name"]);
    const name = row.cells[2]!;
    expect(row.cells[1], `the WBS code of ${name}`).toBe(EXPECTED.get(name));
    if (row.cells[0] !== "") leaves += 1;
  }
  expect(leaves, "activities are among the rows").toBeGreaterThan(0);
  // The table keeps its half of the box with the ID column in it (id-column.spec.ts) and takes nothing more for WBS: the Task column gives the WBS track's width.
  expect(xer.tableWidth, "the WBS column adds no width").toBeCloseTo(demo.tableWidth, 0);
  expect(xer.wbsTrack, "the WBS track").toBeCloseTo(await g.page.evaluate(() => parseFloat(getComputedStyle(document.getElementById("app")!).getPropertyValue("--gantt-wbs-col"))), 0);
  expect(await g.page.evaluate(() => window.gantt!.app.has_wbs_codes())).toBe(true);

  // A file with neither ID nor WBS text takes the column away.
  await openFile(g, "summary.xml", SUMMARY, 2);
  const plain = await readTable(g.page);
  expect(plain.wbs).toBeUndefined();
  expect(plain.head).toEqual(["Task", "Dur", "Start", "Finish", "Float"]);
  for (const row of plain.rows) expect(row.classes).toEqual(["name", "dur", "start", "finish", "float"]);
  expect(await g.page.evaluate(() => window.gantt!.app.has_wbs_codes())).toBe(false);
  expect(await g.page.evaluate(() => window.gantt!.app.task_wbs_code(1_000_000)), "past the schedule").toBe("");
});

test("an MS Project file with WBS text shows the WBS column without an ID column", async ({ g }) => {
  await g.open({ n: 200 });
  await openFile(g, "wbs.xml", MSPDI_WBS, 1);
  const mspdi = await readTable(g.page);
  expect(mspdi.wbs).toBe("true");
  expect(mspdi.head).toEqual(["WBS", "Task", "Dur", "Start", "Finish", "Float"]);
  expect(mspdi.rows.map((row) => [row.cells[1], row.cells[0]])).toEqual([
    ["First summary", "1"],
    ["First child", "1.1"],
    ["Second child", ""],
    ["Second summary", ""],
    ["Third child", ""],
  ]);
});

test("after a scroll every row still shows its own WBS code", async ({ g }) => {
  await g.open({ n: 200 });
  await openFile(g, "ledger-timeline.xer", XER, 1);
  await g.page.evaluate(() => {
    document.getElementById("table-body")!.scrollTop = 200;
  });
  await g.nextFrame(4);
  const scrolled = await readTable(g.page);
  expect(scrolled.rows.length).toBeGreaterThan(5);
  for (const row of scrolled.rows) expect(row.cells[1], `the WBS code of ${row.cells[2]}`).toBe(EXPECTED.get(row.cells[2]!));
});
