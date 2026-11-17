import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, pageErrors, test, trackLogs, waitReady, type Gantt, openFilters } from "./support/gantt";
import { backend, dpr, n, url } from "./support/knobs";
import { checkTable } from "./support/table";

// The filter journeys that verify.mjs (removed in 3f191ea6) used to run (gs#97 P4): the critical-only filter here; the longest-path
// filter is step 3 of journey-j2.spec.ts. One 50k page is shared by the whole file (ruling devteam-1-90): the tests run in order on it. The page is shrunk to the
// 320 px window verify.mjs ran these journeys in (ruling devteam-1-90 (4)); P9 owns the bound checks. Which rows each filter
// keeps, that one replaces the other, that turning it off restores the rows, which leaves c walks and that find keeps to the
// filtered rows are checked on `Session` in crates/app/tests/filter.rs and crates/app/tests/longest_path.rs; these check the
// buttons, the keys, the count, the spacer and the table.

import { AXIS, ROW } from "./support/pixels";

/** A `table_rows` row: task, three columns the journeys do not read, critical, kind (0 is a leaf). */
type Row = [number, unknown, unknown, unknown, boolean, number];

const nextFrame = (page: Page, k = 2): Promise<void> =>
  page.evaluate(
    (frameCount) =>
      new Promise<void>((done) => {
        const target = window.gantt!.status().frames + frameCount;
        const wait = (): void => (window.gantt!.status().frames >= target ? done() : void requestAnimationFrame(wait));
        requestAnimationFrame(wait);
      }),
    k,
  );

const status = (page: Page) => page.evaluate(() => window.gantt!.status());

test.describe.serial("filter journeys", () => {
  let page: Page;
  let logs: string[];
  let box: { x: number; y: number; width: number; height: number };

  test.beforeAll(async ({ browser }) => {
    page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dpr });
    logs = trackLogs(page);
    const query = new URLSearchParams({ n: String(n) });
    if (backend === "webgl") query.set("backend", "webgl");
    await page.goto(`${url}?${query}`);
    await waitReady(page, { frames: 30, timeout: 180_000 });
    const bounds = await page.locator("#timeline").boundingBox();
    if (!bounds) throw new Error("#timeline has no bounding box");
    box = bounds;
    // verify.mjs ran these journeys in the 320 px window it shrank to after the table checks.
    const tall = (await status(page)).rows;
    await page.setViewportSize({ width: 1280, height: 320 });
    await page.waitForFunction((t) => window.gantt!.status().rows[1] - window.gantt!.status().rows[0] < t, tall[1] - tall[0], {
      timeout: 10_000,
    });
  });

  test.afterAll(async ({}, testInfo) => {
    await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
    const errors = pageErrors(logs);
    expect.soft(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`).toBe(true);
    await page.close();
  });

  test("critical-only: the button, c and Shift+C reach the filter, and the table, the count and the spacer follow it", { tag: "@smoke" }, async () => {
    const before = (await status(page)).rowCount;
    await openFilters(page);
    await page.locator("#critical-toggle").click();
    await page.waitForFunction(() => window.gantt!.status().criticalOnly === true);
    await nextFrame(page, 3);
    const filtered = await page.evaluate(() => ({
      rowCount: window.gantt!.status().rowCount,
      kept: window.gantt!.status().keptLeaves,
      pressed: document.getElementById("critical-toggle")!.getAttribute("aria-pressed"),
      count: document.getElementById("row-count")!.textContent!,
      spacer: document.getElementById("table-rows")!.getBoundingClientRect().height,
    }));
    expect.soft(filtered.rowCount < before, `critical-only filters ${before} rows to ${filtered.rowCount}`).toBe(true);
    expect.soft(filtered.pressed === "true", `critical-only aria-pressed ${filtered.pressed}, want true`).toBe(true);
    expect
      .soft(
        filtered.count.includes(new Intl.NumberFormat("en-US").format(filtered.kept!)) && filtered.count.includes("of"),
        `critical-only count reads ${JSON.stringify(filtered.count)}`,
      )
      .toBe(true);
    expect
      .soft(Math.abs(filtered.spacer - filtered.rowCount * ROW) <= 1, `critical-only spacer ${filtered.spacer}px, want ${filtered.rowCount * ROW}px`)
      .toBe(true);
    await checkTable(page, box, "critical only");

    await page.evaluate((y) => (document.getElementById("table-body")!.scrollTop = y), filtered.rowCount > 60 ? ROW * 30 : 0);
    await nextFrame(page, 3);
    const topRow = () =>
      page.evaluate(() => {
        const app = window.gantt!.app;
        const s = window.gantt!.status();
        return { scrollY: s.scrollY, top: JSON.parse(app.table_rows(s.rows[0], s.rows[0] + 1))[0] as Row };
      });
    const criticalKeyBefore = await topRow();
    await page.keyboard.press("c");
    await checkTable(page, box, "critical only, c key");
    const criticalKeyAfter = await topRow();
    expect
      .soft(criticalKeyAfter.top[5] === 0 && criticalKeyAfter.top[4] === true, `c landed on critical leaf task ${criticalKeyAfter.top[0]}`)
      .toBe(true);
    expect
      .soft(
        criticalKeyAfter.scrollY !== criticalKeyBefore.scrollY ||
          (criticalKeyBefore.top[0] === criticalKeyAfter.top[0] && criticalKeyBefore.top[5] === 0 && criticalKeyBefore.top[4] === true),
        `c moved scrollY ${criticalKeyBefore.scrollY} -> ${criticalKeyAfter.scrollY}, or the top row was already that critical leaf`,
      )
      .toBe(true);

    await page.keyboard.press("Shift+C");
    await page.waitForFunction(() => window.gantt!.status().criticalOnly === false);
    await nextFrame(page, 3);
    const toggled = await page.evaluate(() => ({
      pressed: document.getElementById("critical-toggle")!.getAttribute("aria-pressed"),
      count: document.getElementById("row-count")!.textContent!,
    }));
    expect.soft(toggled.pressed === "false", `Shift+C aria-pressed ${toggled.pressed}, want false`).toBe(true);
    expect.soft(!toggled.count.includes("of"), `unfiltered count has no "of": ${JSON.stringify(toggled.count)}`).toBe(true);
    await page.keyboard.press("Shift+C");
    await page.waitForFunction(() => window.gantt!.status().criticalOnly === true);
    await nextFrame(page, 3);
    await openFilters(page);
    await page.locator("#critical-toggle").click();
    await page.waitForFunction(() => window.gantt!.status().criticalOnly === false);
    await nextFrame(page, 3);
    await checkTable(page, box, "critical only off");
  });
});

// The activity code filter (gs#703, story 12). The ledger fixture already assigns a Phase-like code to every task through its
// TASKACTV table but defines no code type or value, so the tests add the two definition tables (a type "Phase" with six values) and
// a second type "Area" on the first ten tasks. The expected rows come from the file's own tables read here, not from the app.
const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");
const PLAIN = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));
const PHASES = ["Civil", "MEP", "Site", "Fit-out", "Test", "Handover"];
const AREA_TASKS = 10;
const CODED = Buffer.from(
  LEDGER.replace(
    /%E\s*$/,
    Array.from({ length: AREA_TASKS }, (_, i) => `%R\t${1000000 + i}\t7001\t${7200 + (i % 2)}\t1000\n`).join("") +
      "%T\tACTVTYPE\n%F\tactv_code_type_id\tactv_code_type\n%R\t7000\tPhase\n%R\t7001\tArea\n" +
      "%T\tACTVCODE\n%F\tactv_code_id\tactv_code_type_id\tshort_name\n" +
      PHASES.map((phase, i) => `%R\t${7100 + i}\t7000\t${phase}\n`).join("") +
      "%R\t7200\t7001\tNorth\n%R\t7201\t7001\tSouth\n%E\n",
  ),
);

/** Every table of an XER text as rows keyed by column name. */
function xerTables(text: string): Map<string, Record<string, string>[]> {
  const result = new Map<string, Record<string, string>[]>();
  let name = "";
  let fields: string[] = [];
  for (const line of text.split(/\r?\n/)) {
    const cells = line.split("\t");
    if (cells[0] === "%T") {
      name = cells[1]!;
      result.set(name, []);
    } else if (cells[0] === "%F") fields = cells.slice(1);
    else if (cells[0] === "%R") result.get(name)!.push(Object.fromEntries(fields.map((field, i) => [field, cells[i + 1] ?? ""])));
  }
  return result;
}

/** What the file says the filter keeps for `type` = `value`: the names of its leaves, and the names of the WBS rows above them. */
function expectedFor(type: string, value: string): { leaves: string[]; summaries: string[] } {
  const data = xerTables(CODED.toString("utf8"));
  const types = new Map(data.get("ACTVTYPE")!.map((row) => [row["actv_code_type_id"]!, row["actv_code_type"]!]));
  const values = new Map(data.get("ACTVCODE")!.map((row) => [row["actv_code_id"]!, row["short_name"]!]));
  const carried = data
    .get("TASKACTV")!
    .filter((row) => types.get(row["actv_code_type_id"]!) === type && values.get(row["actv_code_id"]!) === value)
    .map((row) => row["task_id"]!);
  const wbs = new Map(data.get("PROJWBS")!.map((row) => [row["wbs_id"]!, row]));
  const project = data.get("PROJECT")![0]!["proj_short_name"]!;
  const leaves: string[] = [];
  const summaries = new Set<string>();
  for (const task of data.get("TASK")!.filter((row) => carried.includes(row["task_id"]!))) {
    leaves.push(task["task_name"]!);
    for (let node = wbs.get(task["wbs_id"]!); node !== undefined; node = wbs.get(node["parent_wbs_id"]!)) {
      summaries.add(node["proj_node_flag"] === "Y" ? project : node["wbs_name"]!);
    }
  }
  return { leaves: leaves.sort(), summaries: [...summaries].sort() };
}

async function openCoded(g: Gantt, name: string, buffer: Buffer, epoch: number): Promise<void> {
  await g.page.setInputFiles("#open-input", { name, mimeType: "application/octet-stream", buffer });
  await g.page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch, { timeout: 60_000 });
  await g.nextFrame(3);
}

/** The names of the table's rows now, split into leaves and summaries, from the app (not from the DOM, which draws a window). */
const shownRows = (page: Page) =>
  page.evaluate(() => {
    const app = window.gantt!.app;
    const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as [number, unknown, unknown, unknown, boolean, number][];
    const names = (summary: boolean): string[] => rows.filter((row) => (row[5] !== 0) === summary).map((row) => app.task_label(row[0])).sort();
    return { leaves: names(false), summaries: names(true) };
  });

const chooseCode = async (page: Page, type: string, value: string): Promise<void> => {
  await openFilters(page);
  await page.selectOption("#code-type", type);
  await openFilters(page);
  await page.selectOption("#code-value", value);
  await page.waitForFunction(() => window.gantt!.status().filter === "code");
};

test("a file with no activity codes shows no activity code control, and one with codes shows it", async ({ g }) => {
  await g.open({ n: 200 });
  await openFilters(g.page);
  await expect(g.page.locator("#code-filter"), "the demo has no activity codes").toBeHidden();
  expect(await g.page.evaluate(() => window.gantt!.app.activity_codes())).toBe("[]");
  await openCoded(g, "coded.xer", CODED, 1);
  await openFilters(g.page);
  await expect(g.page.locator("#code-filter")).toBeVisible();
  await openFilters(g.page);
  await expect(g.page.locator("#code-value"), "the value choice waits for a type").toBeHidden();
  await openFilters(g.page);
  await expect(g.page.locator("#code-clear")).toBeHidden();
  expect(await g.page.locator("#code-type option").allTextContents()).toEqual(["Activity code", "Area", "Phase"]);
  await openCoded(g, "summary.xml", PLAIN, 2);
  await openFilters(g.page);
  await expect(g.page.locator("#code-filter"), "an MS Project file takes the control away").toBeHidden();
});

test("choosing an activity code type and value keeps exactly the tasks that carry it, the header names it, and Clear brings them back", async ({ g }) => {
  await g.open({ n: 200 });
  await openCoded(g, "coded.xer", CODED, 1);
  const before = await g.status();
  await openFilters(g.page);
  await g.page.selectOption("#code-type", "Phase");
  expect(await g.page.locator("#code-value option").allTextContents(), "the values of the type, in order").toEqual(["Value", ...[...PHASES].sort()]);
  await chooseCode(g.page, "Phase", "Civil");
  await g.nextFrame(3);
  const phase = expectedFor("Phase", "Civil");
  expect(phase.leaves.length, "the check keeps some tasks and drops others").toBeGreaterThan(0);
  expect(phase.leaves.length).toBeLessThan(40);
  expect(await shownRows(g.page)).toEqual(phase);
  const on = await g.status();
  expect(on.rowCount).toBe(phase.leaves.length + phase.summaries.length);
  expect(await g.page.locator("#row-count").textContent()).toBe(`Phase = Civil · ${on.rowCount} of ${on.n} rows`);
  await expect(g.page.locator("#code-clear")).toHaveAttribute("aria-pressed", "true");

  // A second type, and the filter replacing the first.
  await openFilters(g.page);
  await g.page.selectOption("#code-type", "Area");
  await g.page.waitForFunction(() => window.gantt!.status().filter === "off");
  await chooseCode(g.page, "Area", "North");
  await g.nextFrame(3);
  expect(await shownRows(g.page)).toEqual(expectedFor("Area", "North"));
  expect(await g.page.locator("#row-count").textContent()).toContain("Area = North · ");

  // The critical-only filter replaces it: one row filter at a time, and the choices go back to rest.
  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await g.page.waitForFunction(() => window.gantt!.status().filter === "critical");
  await g.nextFrame(3);
  await openFilters(g.page);
  await expect(g.page.locator("#code-clear")).toBeHidden();
  await expect(g.page.locator("#code-type")).toHaveValue("");
  expect(await g.page.locator("#row-count").textContent()).toMatch(/^Critical only · /);
  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();

  await chooseCode(g.page, "Phase", "MEP");
  await openFilters(g.page);
  await g.page.locator("#code-clear").click();
  await g.page.waitForFunction(() => window.gantt!.status().filter === "off");
  await g.nextFrame(3);
  const after = await g.status();
  expect(after.rowCount).toBe(before.rowCount);
  expect(await g.page.locator("#row-count").textContent()).toBe(`${before.rowCount} rows`);
  await expect(g.page.locator("#code-type")).toHaveValue("");
  await openFilters(g.page);
  await expect(g.page.locator("#code-value")).toBeHidden();
});

test("the activity code control and its header text are written only when they change", async ({ g }) => {
  await g.open({ n: 200 });
  await openCoded(g, "coded.xer", CODED, 1);
  // Mutations to the group and the header over 12 frames: at rest, then with the filter on. An unguarded write per frame would show here.
  const writes = (): Promise<number> =>
    g.page.evaluate(
      () =>
        new Promise<number>((done) => {
          const watched = [document.getElementById("code-filter")!, document.getElementById("row-count")!];
          let count = 0;
          const observer = new MutationObserver((records) => (count += records.length));
          for (const element of watched) observer.observe(element, { attributes: true, childList: true, subtree: true, characterData: true });
          const target = window.gantt!.status().frames + 12;
          const wait = (): void => {
            if (window.gantt!.status().frames < target) return void requestAnimationFrame(wait);
            observer.disconnect();
            done(count + observer.takeRecords().length);
          };
          requestAnimationFrame(wait);
        }),
    );
  expect(await writes(), "at rest").toBe(0);
  await chooseCode(g.page, "Phase", "Site");
  await g.nextFrame(3);
  expect(await writes(), "with the filter on").toBe(0);
});

// gs#772: a change refused during a bar press or a cell edit must leave both lists matching the filter that is on. The press is
// held while the two lists are changed, so the refusal is the app's own (`dragging_bar`), not the test's.
test("a refused activity code change leaves both lists on the filter that is on", async ({ g }) => {
  await g.open({ n: 200 });
  await openCoded(g, "coded.xer", CODED, 1);
  const box = await g.box();
  const lists = () =>
    g.page.evaluate(() => ({
      type: (document.getElementById("code-type") as HTMLSelectElement).value,
      value: (document.getElementById("code-value") as HTMLSelectElement).value,
      valueOptions: [...(document.getElementById("code-value") as HTMLSelectElement).options].map((o) => o.value),
      label: window.gantt!.app.activity_code_filter(),
    }));
  // Hold a press on the first bar the pointer finds, then change the lists under it.
  const press = async (): Promise<void> => {
    for (let row = 0; row < 8; row++) {
      const y = box.y + AXIS + row * ROW + ROW / 2;
      // Only a leaf's bar can be pressed into a drag; a summary row's cannot.
      const start = await g.page.evaluate((r) => {
        const app = window.gantt!.app;
        const [, , , , , kind] = JSON.parse(app.table_rows(r, r + 1))[0] as [number, unknown, unknown, unknown, boolean, number];
        return kind === 0 ? app.row_start(r) : undefined;
      }, row);
      if (start === undefined || start === null) continue;
      const x = box.x + (await g.page.evaluate((s) => window.gantt!.app.x_of(s), start)) + 6;
      await g.page.mouse.move(x, y);
      await g.nextFrame(2);
      if ((await g.status()).hover !== null) {
        await g.page.mouse.down();
        for (let k = 1; k <= 8; k++) await g.page.mouse.move(x + 15 * k, y);
        await g.nextFrame(2);
        return;
      }
    }
    throw new Error("no bar under the pointer");
  };
  const held = (): Promise<boolean> => g.page.evaluate(() => window.gantt!.app.dragging_bar());

  await chooseCode(g.page, "Phase", "Civil");
  await g.nextFrame(3);
  const phase = expectedFor("Phase", "Civil");
  await press();
  expect(await held(), "a bar press is held").toBe(true);
  await openFilters(g.page);
  await g.page.selectOption("#code-type", "Area");
  await openFilters(g.page);
  await g.page.selectOption("#code-value", "Civil");
  expect(await lists(), "type refused: both lists still show Phase = Civil").toMatchObject({ type: "Phase", value: "Civil", label: "Phase = Civil" });
  await openFilters(g.page);
  await g.page.selectOption("#code-value", "MEP");
  expect(await lists(), "value refused: the list goes back to the filter on").toMatchObject({ type: "Phase", value: "Civil", label: "Phase = Civil" });
  expect((await lists()).valueOptions, "the value list is still Phase's").toEqual(["", ...[...PHASES].sort()]);
  expect(await shownRows(g.page), "the rows are unchanged").toEqual(phase);
  await g.page.mouse.up();
  await g.nextFrame(3);

  // No filter on: a refused type change leaves both lists empty.
  await openFilters(g.page);
  await g.page.locator("#code-clear").click();
  await g.page.waitForFunction(() => window.gantt!.status().filter === "off");
  await press();
  await openFilters(g.page);
  await g.page.selectOption("#code-type", "Area");
  expect(await lists(), "no filter: the type goes back to empty").toMatchObject({ type: "", label: "" });
  await openFilters(g.page);
  await expect(g.page.locator("#code-value")).toBeHidden();
  await g.page.mouse.up();
  await g.nextFrame(3);

  // Released, the same change is taken.
  await chooseCode(g.page, "Area", "North");
  expect(await lists()).toMatchObject({ type: "Area", value: "North", label: "Area = North" });

  // Opening a file drops the filter, so the remembered one goes too: a refused change must not bring it back.
  await openCoded(g, "coded.xer", CODED, 2);
  expect(await lists(), "reopened: the filter is off").toMatchObject({ type: "", label: "" });
  await press();
  await openFilters(g.page);
  await g.page.selectOption("#code-type", "Phase");
  expect(await lists(), "reopened, refused: the old filter does not come back").toMatchObject({ type: "", label: "" });
  await openFilters(g.page);
  await expect(g.page.locator("#code-value")).toBeHidden();
  await g.page.mouse.up();
});
