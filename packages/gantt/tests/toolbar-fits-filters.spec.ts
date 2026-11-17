import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { closeFilters, expect, openFilters, test, type Gantt } from "./support/gantt";

// The toolbar fits in every filter state (gs#794). The strip is one line high, so a control that does not fit would push the file
// line or the find box out of it. For each state at 1280 and 1440 px: the toolbar does not overflow, the file line and the find box
// lie inside it, and (with a code filter set) the two code choices show the whole of what they hold. The file is the ledger fixture
// with the two code definition tables of filters.spec.ts added (a type "Phase" with six values, a second type "Area").
const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");
const PHASES = ["Civil", "MEP", "Site", "Fit-out", "Test", "Handover"];
const CODED = Buffer.from(
  LEDGER.replace(
    /%E\s*$/,
    "%T\tACTVTYPE\n%F\tactv_code_type_id\tactv_code_type\n%R\t7000\tPhase\n%R\t7001\tArea\n" +
      "%T\tACTVCODE\n%F\tactv_code_id\tactv_code_type_id\tshort_name\n" +
      PHASES.map((phase, i) => `%R\t${7100 + i}\t7000\t${phase}\n`).join("") +
      "%R\t7200\t7001\tNorth\n%R\t7201\t7001\tSouth\n%E\n",
  ),
);

// The same file with long labels, as a P6 file from a real project has them.
const LONG_TYPE = "Construction phase";
const LONG_VALUE = "Mechanical commissioning";
const LONG = Buffer.from(CODED.toString("utf8").replace("%R\t7000\tPhase", `%R\t7000\t${LONG_TYPE}`).replace("%R\t7105\t7000\tHandover", `%R\t7105\t7000\t${LONG_VALUE}`));

// The long-label file with its tasks repeated 500 times over, each copy carrying the long-label code: a row count of five digits.
const COPIES = 500;
function inflated(): Buffer {
  const lines = LONG.toString("utf8").split("\n");
  const first = lines.findIndex((line) => line.startsWith("%T\tTASK"));
  const end = lines.findIndex((line, i) => i > first && line.startsWith("%T"));
  const rows = lines.slice(first + 2, end).filter((line) => line.startsWith("%R"));
  const clones: string[] = [];
  const codes: string[] = [];
  for (let copy = 1; copy <= COPIES; copy += 1) {
    for (const row of rows) {
      const cells = row.split("\t");
      const id = String(Number(cells[1]) + copy * 100_000);
      cells[1] = id;
      clones.push(cells.join("\t"));
      codes.push(`%R\t${id}\t7000\t7105\t1000`);
    }
  }
  const taskActv = lines.findIndex((line) => line.startsWith("%T\tTASKACTV"));
  lines.splice(taskActv + 2, 0, ...codes);
  lines.splice(end, 0, ...clones);
  return Buffer.from(lines.join("\n"));
}

/** Where the toolbar's overflow, the file line and the find box stand now. */
const fit = (page: Page) =>
  page.evaluate(() => {
    const toolbar = document.getElementById("toolbar")!;
    const bar = toolbar.getBoundingClientRect();
    const inside = (id: string): boolean => {
      const box = document.getElementById(id)!.getBoundingClientRect();
      return box.left >= bar.left && box.right <= bar.right && box.top >= bar.top && box.bottom <= bar.bottom;
    };
    // The file group is the strip's shock absorber, but it never gives up the Open button, and the file line keeps room for its first letters (the file group gives the rest up; the full text is its tooltip).
    const group = document.getElementById("toolbar-file")!.getBoundingClientRect();
    const button = document.getElementById("open-button")!.getBoundingClientRect();
    const line = document.getElementById("open-info")!.getBoundingClientRect();
    return {
      overflow: toolbar.scrollWidth - toolbar.clientWidth,
      file: inside("open-info") && button.right <= group.right && line.width >= 20,
      find: inside("find-input"),
      columns: inside("columns-button"),
    };
  });

/** How much room each code choice lacks for the text it shows, in pixels (at most 0 when it shows all of it). */
const shortOf = (page: Page) =>
  page.evaluate(() => {
    const context = document.createElement("canvas").getContext("2d")!;
    return ["code-type", "code-value"].map((id) => {
      const select = document.getElementById(id) as HTMLSelectElement;
      const style = getComputedStyle(select);
      context.font = `${style.fontSize} ${style.fontFamily}`;
      const text = select.selectedOptions[0]!.text;
      // The arrow and the padding take about 20 px of the select's width.
      return context.measureText(text).width + 20 - select.getBoundingClientRect().width;
    });
  });

/** Press a control by its click handler: while the toolbar overflows a control can lie past the window's edge, where a mouse click cannot reach it. */
const press = (page: Page, selector: string): Promise<void> => page.locator(selector).evaluate((el) => (el as HTMLElement).click());

/** Open `buffer` as a file with activity codes (the second open of a page is epoch 2). */
async function openCoded(g: Gantt, buffer: Buffer, epoch: number): Promise<void> {
  await g.page.setInputFiles("#open-input", { name: "coded.xer", mimeType: "application/octet-stream", buffer });
  await g.page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch, { timeout: 60_000 });
  await g.nextFrame(3);
  await openFilters(g.page);
  await expect(g.page.locator("#code-filter")).toBeVisible();
  await closeFilters(g.page);
  // The file's import notes show as a notice in the strip (gs#818); every fit below is measured with it there.
  await expect(g.page.locator("#import-notes")).toBeVisible();
}

const STATES: { name: string; on: (g: Gantt) => Promise<void>; off: (g: Gantt) => Promise<void> }[] = [
  { name: "no filter", on: async () => {}, off: async () => {} },
  { name: "Critical only", on: (g) => press(g.page, "#critical-toggle"), off: (g) => press(g.page, "#critical-toggle") },
  { name: "Longest path", on: (g) => press(g.page, "#longest-toggle"), off: (g) => press(g.page, "#longest-toggle") },
  {
    name: "Slipped",
    on: async (g) => {
      await press(g.page, "#baseline-set");
      await g.nextFrame(2);
      await press(g.page, "#slipped-toggle");
    },
    off: async (g) => {
      await press(g.page, "#slipped-toggle");
      await press(g.page, "#baseline-clear");
    },
  },
  { name: "Lookahead", on: (g) => press(g.page, "#lookahead-toggle"), off: (g) => press(g.page, "#lookahead-toggle") },
  {
    name: "an activity code",
    on: async (g) => {
      await openFilters(g.page);
      await g.page.selectOption("#code-type", "Phase");
      await g.page.selectOption("#code-value", "Handover");
      await g.page.waitForFunction(() => window.gantt!.status().filter === "code");
      await closeFilters(g.page);
    },
    off: (g) => press(g.page, "#code-clear"),
  },
];

test("at 1280 px Find's feedback stays inside the toolbar with the import notice and a crowded filter state", async ({ g }) => {
  await g.page.setViewportSize({ width: 1280, height: 800 });
  await g.open({ n: 300 });
  await openCoded(g, CODED, 1);
  for (const state of STATES.filter((candidate) => ["Slipped", "Lookahead", "an activity code"].includes(candidate.name))) {
    await state.on(g);
    await g.nextFrame(3);
    await g.page.fill("#find-input", "zzzz");
    await expect(g.page.locator("#find-count"), `${state.name}: a search with no match says so`).not.toBeEmpty();
    await g.nextFrame(2);
    const seen = await g.page.evaluate(() => {
      const bar = document.getElementById("toolbar")!.getBoundingClientRect();
      const right = (id: string): number => document.getElementById(id)!.getBoundingClientRect().right;
      const find = document.getElementById("find")!.getBoundingClientRect();
      const count = document.getElementById("find-count")!.getBoundingClientRect();
      return {
        overflow: document.getElementById("toolbar")!.scrollWidth - document.getElementById("toolbar")!.clientWidth,
        countInside: count.right <= bar.right && count.right <= find.right + 1 && right("find-input") <= count.left + 1,
        notice: document.getElementById("import-notes")!.innerText,
      };
    });
    expect.soft(seen.overflow, `${state.name}: the toolbar overflows by`).toBeLessThanOrEqual(0);
    expect.soft(seen.countInside, `${state.name}: the feedback lies inside the toolbar, after the input`).toBe(true);
    expect.soft(seen.notice, `${state.name}: the notice reads in full`).toMatch(/^\d+ import notes$/);
    await g.page.fill("#find-input", "");
    await state.off(g);
    await g.nextFrame(2);
  }
});

for (const width of [1280, 1440]) {
  test(`the toolbar fits at ${width} px with no filter, each filter and an activity code set`, async ({ g }) => {
    await g.page.setViewportSize({ width, height: 800 });
    await g.open({ n: 300 });
    await openCoded(g, CODED, 1);

    for (const state of STATES) {
      await state.on(g);
      await g.nextFrame(3);
      const seen = await fit(g.page);
      expect.soft(seen.overflow, `${state.name}: the toolbar overflows by`).toBeLessThanOrEqual(0);
      expect.soft(seen.file, `${state.name}: the Open button and the file line fit their group, inside the toolbar`).toBe(true);
      expect.soft(seen.find, `${state.name}: the find box is inside the toolbar`).toBe(true);
      expect.soft(seen.columns, `${state.name}: the Columns button is inside the toolbar`).toBe(true);
      // The notice keeps its whole label in every state: the rendered text, not the DOM text, which also holds hidden spans.
      expect.soft(await g.page.locator("#import-notes").innerText(), `${state.name}: the notice reads in full`).toMatch(/^\d+ import notes$/);
      if (state.name === "an activity code") {
        await openFilters(g.page);
        const short = await shortOf(g.page);
        await closeFilters(g.page);
        expect.soft(short[0], "the code type shows its whole name").toBeLessThanOrEqual(0);
        expect.soft(short[1], "the code value shows its whole name").toBeLessThanOrEqual(0);
      }
      await state.off(g);
      await g.nextFrame(2);
    }
  });
}

for (const width of [1280, 1440]) {
  test(`the toolbar fits at ${width} px with a long code type and value, and the row count keeps its numbers`, async ({ g }) => {
    await g.page.setViewportSize({ width, height: 800 });
    await g.open({ n: 300 });
    await openCoded(g, LONG, 1);
    await openFilters(g.page);
    await g.page.selectOption("#code-type", LONG_TYPE);
    await g.page.selectOption("#code-value", LONG_VALUE);
    await g.page.waitForFunction(() => window.gantt!.status().filter === "code");
    await closeFilters(g.page);
    await g.nextFrame(3);
    const seen = await fit(g.page);
    expect.soft(seen.overflow, "the toolbar overflows by").toBeLessThanOrEqual(0);
    expect.soft(seen.file, "the Open button and the file line fit their group, inside the toolbar").toBe(true);
    expect.soft(seen.find, "the find box is inside the toolbar").toBe(true);
    expect.soft(seen.columns, "the Columns button is inside the toolbar").toBe(true);
    // The row count is cut at its end, so it starts at its first letter (gs#829); it keeps room for its numbers.
    const count = g.page.locator("#row-count");
    await expect.soft(count).toHaveText(new RegExp(`^${LONG_TYPE} = ${LONG_VALUE} · \\d+ of \\d+ rows$`));
    const box = await count.evaluate((el) => ({ width: el.getBoundingClientRect().width, shown: getComputedStyle(el).direction }));
    expect.soft(box.width, "the row count keeps room for its numbers").toBeGreaterThanOrEqual(90);
    expect.soft(box.shown).toBe("ltr");
  });
}

for (const width of [1280, 1440]) {
  test(`the find box's note shows whole at ${width} px`, async ({ g }) => {
    await g.page.setViewportSize({ width, height: 800 });
    await g.open({ n: 300 });
    await g.page.locator("#find-input").fill("zzzzqqqq");
    await expect(g.page.locator("#find-count")).toHaveText("no matches");
    await g.nextFrame(2);
    const note = await g.page.locator("#find-count").evaluate((el) => ({ scroll: el.scrollWidth, client: el.clientWidth }));
    expect(note.scroll, "the note is cut by").toBeLessThanOrEqual(note.client);
    expect((await fit(g.page)).overflow, "the toolbar overflows by").toBeLessThanOrEqual(0);
  });
}

for (const width of [1280, 1440]) {
  test(`the find box's note under a row filter names the shown rows in its title and shows whole at ${width} px`, async ({ g }) => {
    await g.page.setViewportSize({ width, height: 800 });
    await g.open({ n: 300 });
    await g.page.evaluate(() => window.gantt!.app.set_critical_only(true));
    await g.page.locator("#find-input").fill("zzzzqqqq");
    await expect(g.page.locator("#find-count")).toHaveText("no matches");
    await expect(g.page.locator("#find-count")).toHaveAttribute("title", "no matches in the shown rows");
    await g.nextFrame(2);
    const note = await g.page.locator("#find-count").evaluate((el) => ({ scroll: el.scrollWidth, client: el.clientWidth }));
    expect(note.scroll, "the note is cut by").toBeLessThanOrEqual(note.client);
    expect((await fit(g.page)).overflow, "the toolbar overflows by").toBeLessThanOrEqual(0);
  });
}

test("at 1280 px the row count keeps the whole of a five-digit count while the filter's name gives way", async ({ g }) => {
  await g.page.setViewportSize({ width: 1280, height: 800 });
  await g.open({ n: 300 });
  await openCoded(g, inflated(), 1);
  await openFilters(g.page);
  await g.page.selectOption("#code-type", LONG_TYPE);
  await g.page.selectOption("#code-value", LONG_VALUE);
  await g.page.waitForFunction(() => window.gantt!.status().filter === "code", undefined, { timeout: 60_000 });
  await closeFilters(g.page);
  await g.nextFrame(3);
  const count = g.page.locator("#row-count");
  await expect(count).toHaveText(/ \d{1,3},\d{3} of \d{2},\d{3} rows$/);
  const seen = await count.evaluate((el) => {
    // The numbers are the text after the last "·"; measure them in a copy that has the row count's font.
    const numbers = el.textContent!.split(" · ").pop()!;
    const probe = el.cloneNode(false) as HTMLElement;
    probe.removeAttribute("id");
    probe.textContent = `…${numbers}`;
    probe.style.cssText = "position:absolute;visibility:hidden;min-width:0;overflow:visible;direction:ltr";
    el.parentElement!.appendChild(probe);
    const needed = probe.getBoundingClientRect().width;
    probe.remove();
    return { needed, room: el.clientWidth, numbers };
  });
  expect(seen.room, `the row count shows "${seen.numbers}" whole when it has ${seen.needed}px for it`).toBeGreaterThanOrEqual(seen.needed);
  expect((await fit(g.page)).overflow, "the toolbar overflows by").toBeLessThanOrEqual(0);
});
