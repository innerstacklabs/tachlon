// The task table with every column a file can turn on (gs#805, ruled by karpathy): the ID, WBS and four progress columns at 1280 px.
// The Task column keeps a minimum width from a token, the table scrolls sideways when the columns need more than the pane, and the
// Task, ID and WBS columns stay at the left while the rest scroll. one-project.xer has activity IDs, WBS codes and actuals.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// A 45-row office fit-out project as MS Project XML, with task names up to 43 characters (gs#821).
const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));
const XER = readFileSync(new URL("./fixtures/one-project.xer", import.meta.url));
const HEAD = ["ID", "WBS", "Task", "Dur", "Start", "Finish", "Float", "Actual start", "Actual finish", "Remaining", "% complete"];

async function openProject(g: Gantt): Promise<void> {
  await g.open({ n: 200 });
  await g.page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: XER });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
}

/** The left edge of the first row's cell with this class, and of the head cell with it (the Task head has no class, so it is the third). */
async function lefts(page: Page): Promise<{ row: Record<string, number>; head: Record<string, number> }> {
  return page.evaluate(() => {
    const row: Record<string, number> = {};
    for (const cell of document.querySelectorAll<HTMLElement>("#table-rows .row:first-child > span")) row[cell.className] = cell.getBoundingClientRect().left;
    const head: Record<string, number> = {};
    const cells = [...document.querySelectorAll<HTMLElement>("#table-head .cols > span")];
    for (const cell of cells) head[cell.textContent ?? ""] = cell.getBoundingClientRect().left;
    return { row, head };
  });
}

test("with every column on, the Task column keeps its minimum width and names are readable", async ({ g }) => {
  await openProject(g);
  const facts = await g.page.evaluate(() => {
    const table = document.getElementById("table")!;
    const head = [...document.querySelectorAll("#table-head .cols > span")].map((cell) => cell.textContent ?? "");
    const probe = (width: string): number => {
      const el = document.createElement("div");
      el.style.cssText = `position:absolute;visibility:hidden;width:${width}`;
      document.querySelector("#table-rows .row")!.append(el);
      const px = el.getBoundingClientRect().width;
      el.remove();
      return px;
    };
    const names = [...document.querySelectorAll<HTMLElement>("#table-rows .row > .name")];
    return {
      flags: [table.dataset.ids, table.dataset.wbs, table.dataset.progress],
      head,
      token: getComputedStyle(document.getElementById("table")!).getPropertyValue("--gantt-task-min").trim(),
      tokenPx: probe("var(--gantt-task-min)"),
      sixteenCh: probe("16ch"),
      names: names.map((name) => ({ width: name.getBoundingClientRect().width, indent: parseFloat(getComputedStyle(name).paddingLeft) })),
    };
  });
  expect(facts.flags, "ID, WBS and progress columns are on").toEqual(["true", "true", "true"]);
  expect(facts.head).toEqual(HEAD);
  expect(facts.token, "the minimum comes from a token").not.toBe("");
  expect(facts.tokenPx, "the token is about 16 characters of the body font").toBeGreaterThanOrEqual(facts.sixteenCh - 0.5);
  expect(facts.sixteenCh, "16 characters measure something").toBeGreaterThan(60);
  expect(facts.names.length).toBeGreaterThan(5);
  for (const name of facts.names) {
    expect(name.width, "the Task column is at least the token").toBeGreaterThanOrEqual(facts.tokenPx - 0.5);
    if (name.indent <= 14) expect(name.width - name.indent, "a top-level name has room for 16 characters").toBeGreaterThanOrEqual(facts.sixteenCh - 0.5);
  }
});

test("the_id_track_fits_its_longest_id_and_a_wbs_code_shows_in_full", async ({ g }) => {
  await g.page.setViewportSize({ width: 1280, height: 800 });
  await openProject(g);
  const facts = await g.page.evaluate(() => {
    const textWidth = (cell: HTMLElement): number => {
      const range = document.createRange();
      range.selectNodeContents(cell);
      return range.getBoundingClientRect().width;
    };
    const cells = (cls: string): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(`#table-rows .row > .${cls}`)];
    const widest = (cls: string): number => Math.max(...cells(cls).map(textWidth));
    const token = (name: string): number => {
      const probe = document.createElement("div");
      probe.style.cssText = `position:absolute;visibility:hidden;width:var(${name})`;
      document.getElementById("table")!.append(probe);
      const px = probe.getBoundingClientRect().width;
      probe.remove();
      return px;
    };
    const track = (cls: string): number => document.querySelector<HTMLElement>(`#table-head .cols > .${cls}`)!.getBoundingClientRect().width;
    const name = document.querySelector<HTMLElement>("#table-rows .row > .name")!;
    return {
      idTrack: track("code"),
      idText: widest("code"),
      idMax: token("--gantt-id-max"),
      wbsTrack: track("wbs"),
      wbsText: widest("wbs"),
      wbsMax: token("--gantt-wbs-max"),
      wbsCut: cells("wbs").filter((cell) => cell.scrollWidth > cell.clientWidth).map((cell) => cell.textContent),
      idCut: cells("code").filter((cell) => cell.scrollWidth > cell.clientWidth).map((cell) => cell.textContent),
      nameWidth: name.getBoundingClientRect().width,
      taskMin: token("--gantt-task-min"),
    };
  });
  expect(facts.idText, "the fixture has activity IDs").toBeGreaterThan(20);
  expect(facts.idCut, "no ID is cut").toEqual([]);
  expect(facts.idTrack, "the ID track holds its longest ID").toBeGreaterThanOrEqual(facts.idText);
  expect(facts.idTrack, "and is not much wider than it: the longest ID plus padding").toBeLessThanOrEqual(Math.min(facts.idText + 24, facts.idMax));
  expect(facts.wbsText, "the fixture has WBS codes").toBeGreaterThan(20);
  expect(facts.wbsCut, "every WBS code shows in full at 1280 px").toEqual([]);
  expect(facts.wbsTrack, "the WBS track holds its longest code").toBeGreaterThanOrEqual(facts.wbsText);
  expect(facts.wbsTrack, "and stays within its cap").toBeLessThanOrEqual(facts.wbsMax + 0.5);
  expect(facts.nameWidth, "the Task column keeps its minimum").toBeGreaterThanOrEqual(facts.taskMin - 0.5);
});

test("the office fit-out opens with every task name in full at 1280 px (gs#821)", async ({ g }) => {
  await g.page.setViewportSize({ width: 1280, height: 1000 });
  await g.open({ n: 200 });
  await g.page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  const facts = await g.page.evaluate(() => {
    const names = [...document.querySelectorAll<HTMLElement>("#table-rows .row > .name")];
    const dates = [...document.querySelectorAll<HTMLElement>("#table-rows .row > .start, #table-rows .row > .finish")];
    return {
      dateWidths: [...new Set(dates.map((cell) => cell.getBoundingClientRect().width))],
      token: parseFloat(getComputedStyle(document.getElementById("table")!).getPropertyValue("--gantt-date-col")),
      widest: Math.max(...names.map((name) => name.getBoundingClientRect().width)),
      cut: names.filter((name) => name.scrollWidth > name.clientWidth).map((name) => `${name.textContent} (${name.scrollWidth} > ${name.clientWidth})`),
      count: names.length,
    };
  });
  expect(facts.count, "the table shows the fit-out's rows").toBeGreaterThan(30);
  expect(facts.cut, "no name is cut").toEqual([]);
  expect(facts.dateWidths, "Start and Finish keep the width of their token").toEqual([facts.token]);
  expect(facts.widest, "Task takes the spare width").toBeGreaterThan(250);
});

test("when the columns need more than the pane, the table scrolls sideways with Task, ID and WBS frozen", async ({ g }) => {
  await openProject(g);
  const before = await lefts(g.page);
  const sizes = await g.page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    return { scroll: body.scrollWidth, client: body.clientWidth };
  });
  expect(sizes.scroll, "the table is wider than its pane").toBeGreaterThan(sizes.client);

  await g.page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    body.scrollLeft = body.scrollWidth;
  });
  await g.nextFrame(3);
  const after = await lefts(g.page);
  const moved = await g.page.evaluate(() => document.getElementById("table-body")!.scrollLeft);
  expect(moved, "the table scrolled").toBeGreaterThan(100);
  for (const cls of ["code", "wbs", "name"]) expect(after.row[cls], `the ${cls} cell stays at the left`).toBeCloseTo(before.row[cls]!, 0);
  for (const label of ["ID", "WBS", "Task"]) expect(after.head[label], `the ${label} head stays at the left`).toBeCloseTo(before.head[label]!, 0);
  for (const cls of ["dur", "start", "percent"]) expect(after.row[cls]!, `the ${cls} cell scrolled`).toBeLessThan(before.row[cls]! - 100);
  for (const [label, cls] of [["Dur", "dur"], ["% complete", "percent"]] as const) {
    expect(after.head[label], `the ${label} head scrolls with its column`).toBeCloseTo(after.row[cls]!, 0);
  }

  // The last column is reachable: its right edge is inside the pane once scrolled to the end.
  const reach = await g.page.evaluate(() => {
    const body = document.getElementById("table-body")!.getBoundingClientRect();
    const last = document.querySelector<HTMLElement>("#table-rows .row:first-child > .percent")!.getBoundingClientRect();
    return { paneRight: body.right, cellRight: last.right };
  });
  expect(reach.cellRight, "the % complete column ends inside the pane").toBeLessThanOrEqual(reach.paneRight + 0.5);
});

for (const scheme of ["light", "dark"] as const) {
  test(`${scheme}: a frozen cell covers what scrolls under it`, async ({ g }) => {
    await g.page.emulateMedia({ colorScheme: scheme });
    await openProject(g);
    await g.page.evaluate(() => {
      const body = document.getElementById("table-body")!;
      body.scrollLeft = body.scrollWidth;
    });
    await g.nextFrame(3);
    const covered = await g.page.evaluate(() => {
      const hit = (selector: string): { top: boolean; opaque: boolean } => {
        const el = document.querySelector<HTMLElement>(selector)!;
        const box = el.getBoundingClientRect();
        const at = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
        const alpha = getComputedStyle(el).backgroundColor.match(/^rgba\(.*,\s*([\d.]+)\)$/);
        return { top: at === el || el.contains(at), opaque: alpha === null || Number(alpha[1]) === 1 };
      };
      return { code: hit("#table-rows .row[data-kind='0'] > .code"), wbs: hit("#table-rows .row[data-kind='0'] > .wbs"), name: hit("#table-rows .row[data-kind='0'] > .name") };
    });
    for (const [cls, fact] of Object.entries(covered)) {
      expect(fact.top, `the ${cls} cell is the top element at its centre, nothing drawn over it`).toBe(true);
      expect(fact.opaque, `the ${cls} cell has a fill, so scrolled cells do not show through`).toBe(true);
    }
  });
}

/** How many pixels of `colour` (an "rgb(r, g, b)" string, exact) a screenshot of this rect holds, and how many per column of the rect. */
async function paint(page: Page, rect: { x: number; y: number; width: number; height: number }, colour: string): Promise<{ total: number; columns: number[] }> {
  const png = (await page.screenshot({ clip: rect })).toString("base64");
  return page.evaluate(
    async ({ png, colour }) => {
      const bytes = Uint8Array.from(atob(png), (c) => c.charCodeAt(0));
      const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const ctx = canvas.getContext("2d")!;
      ctx.drawImage(bitmap, 0, 0);
      const [r, g, b] = colour.match(/\d+/g)!.map(Number);
      const data = ctx.getImageData(0, 0, bitmap.width, bitmap.height).data;
      const columns = new Array<number>(bitmap.width).fill(0);
      for (let y = 0; y < bitmap.height; y++)
        for (let x = 0; x < bitmap.width; x++) {
          const i = (y * bitmap.width + x) * 4;
          if (data[i] === r && data[i + 1] === g && data[i + 2] === b) columns[x]!++;
        }
      return { total: columns.reduce((a, b) => a + b, 0), columns };
    },
    { png, colour },
  );
}

/** A token's resolved colour. */
const resolved = (page: Page, token: string): Promise<string> =>
  page.evaluate((name) => {
    const probe = document.createElement("div");
    probe.style.background = `var(${name})`;
    document.getElementById("app")!.append(probe);
    const colour = getComputedStyle(probe).backgroundColor;
    probe.remove();
    return colour;
  }, token);

for (const scheme of ["light", "dark"] as const) {
  test(`${scheme}: the gutter's markers stay on screen when the table scrolls sideways`, async ({ g }) => {
    await g.page.emulateMedia({ colorScheme: scheme });
    await openProject(g);
    // A critical row, selected, with the row below it in the range: the diamond, the selected edge and the range edge.
    const critical = await g.page.evaluate(() => Number(document.querySelector<HTMLElement>("#table-rows .row.critical")?.dataset.row ?? -1));
    expect(critical, "the project has a critical row on screen").toBeGreaterThanOrEqual(0);
    await g.page.locator(`#table-rows .row[data-row="${critical}"] > .name`).click();
    await g.page.keyboard.press("Shift+ArrowDown");
    await g.nextFrame(3);
    const rows = await g.page.evaluate(() => ({
      selected: Number(document.querySelector<HTMLElement>("#table-rows .row[data-selected='true']")?.dataset.row ?? -1),
      ranged: [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-range='true']")].map((row) => Number(row.dataset.row)),
    }));
    expect(rows.selected, "the clicked row is the selected one").toBe(critical);
    const other = rows.ranged.find((row) => row !== critical);
    expect(other, "a second row is in the range").toBeDefined();

    const crit = await resolved(g.page, "--gantt-crit");
    const edge = await resolved(g.page, "--gantt-sel-edge");
    const gutter = async (row: number, colour: string) => {
      const box = await g.page.evaluate((r) => {
        const rect = document.querySelector(`#table-rows .row[data-row="${r}"]`)!.getBoundingClientRect();
        const pane = document.getElementById("table-body")!.getBoundingClientRect();
        return { x: pane.left, y: rect.top, width: 18, height: rect.height };
      }, row);
      return { box, ...(await paint(g.page, box, colour)) };
    };

    for (const phase of ["at rest", "scrolled to the end"]) {
      if (phase !== "at rest") {
        await g.page.evaluate(() => {
          const body = document.getElementById("table-body")!;
          body.scrollLeft = body.scrollWidth;
        });
        await g.nextFrame(3);
        expect(await g.page.evaluate(() => document.getElementById("table-body")!.scrollLeft), "the table scrolled").toBeGreaterThan(100);
      }
      const diamond = await gutter(critical, crit);
      expect(diamond.total, `${phase}: the critical diamond is in the gutter`).toBeGreaterThanOrEqual(12);
      const selected = await gutter(critical, edge);
      expect(selected.columns.slice(0, 3).every((n) => n >= selected.box.height - 2), `${phase}: the selected row's 3px edge runs the row's height`).toBe(true);
      const ranged = await gutter(other!, edge);
      expect(ranged.columns[0]! >= ranged.box.height - 2, `${phase}: the range row's 1px edge runs the row's height`).toBe(true);
      expect(ranged.columns[1], `${phase}: and is one pixel wide`).toBeLessThan(4);
    }
  });
}

// The finish line of gs#851: at 1280 px with the default split no table column is cut mid-value. The pane edge falls between columns (or a
// cell shows an ellipsis). The files differ in how wide their ID and WBS columns are, so the fit cannot rest on one file's widths. Both files here carry progress columns, which is what puts the edge through a column.
const SAMPLES = [
  { file: "one-project.xer", type: "application/octet-stream", buffer: XER, schemes: ["light", "dark"] as const },
  { file: "ledger-timeline.xer", type: "application/octet-stream", buffer: readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url)), schemes: ["light"] as const },
];
for (const sample of SAMPLES) {
  for (const scheme of sample.schemes) {
    test(`at 1280 px the pane edge falls between table columns, not through one (gs#851, ${sample.file}, ${scheme})`, async ({ g }) => {
      await g.page.emulateMedia({ colorScheme: scheme });
      await g.page.setViewportSize({ width: 1280, height: 800 });
      await g.open({ n: 200 });
      await g.page.setInputFiles("#open-input", { name: sample.file, mimeType: sample.type, buffer: sample.buffer });
      await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
      await g.nextFrame(3);
      await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
      const facts = await g.page.evaluate(() => {
        const body = document.getElementById("table-body")!;
        const edge = body.getBoundingClientRect().left + body.clientWidth;
        const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row")];
        const spans = [...document.querySelectorAll<HTMLElement>("#table-head .cols > span"), ...rows.flatMap((row) => [...row.querySelectorAll<HTMLElement>(":scope > span")])];
        const boxes = spans.map((cell) => ({ cell, box: cell.getBoundingClientRect() })).filter(({ box }) => box.width > 0);
        // The pane clips a cell that straddles its edge, whatever the cell's own text-overflow says, so any straddling cell is a cut one.
        const cut = boxes.filter(({ box }) => box.left < edge - 0.5 && box.right > edge + 0.5);
        const lastRight = Math.max(...boxes.filter(({ box }) => box.right <= edge + 0.5).map(({ box }) => box.right));
        const last = boxes.filter(({ box }) => box.right === lastRight);
        return {
          scrollLeft: body.scrollLeft,
          rows: rows.length,
          cut: cut.map(({ cell, box }) => `${cell.className || cell.textContent} ${box.left}..${box.right} at ${edge}`),
          lastCells: last.length,
        };
      });
      expect(facts.scrollLeft, "the table is at rest").toBe(0);
      expect(facts.rows, "several rows are on screen").toBeGreaterThan(5);
      expect(facts.cut, "no column is cut by the pane edge").toEqual([]);
      expect(facts.lastCells, "the last column shows in the head and in every visible row").toBe(facts.rows + 1);
    });
  }
}
