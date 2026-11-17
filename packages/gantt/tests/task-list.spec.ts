import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// The Ledger task list (gs#110 PR 2): the critical diamond, float in days, the leaf name only, and a hover that
// shows on the table and the canvas together. Everything is read through window.gantt.

const SIZE = 2000;

/** Scroll `row` to the middle of the view and wait for the table to draw it. */
async function reveal(g: Gantt, row: number): Promise<void> {
  await g.page.evaluate((r) => {
    const s = window.gantt!.status();
    window.gantt!.app.set_scroll_y(Math.max(0, r * s.rowHeight - 200));
  }, row);
  await g.page.waitForFunction((r) => document.querySelector(`#table-rows .row[data-row="${r}"]`) !== null, row);
  await g.nextFrame(2);
}

/** The demo schedule's working-day lengths in minutes (8 h, 10 h, 10.5 h, 24 h) and the first one, the standard day. */
const DEMO_DAY_LENGTHS = [480, 600, 630, 1440];
const DEMO_DAY_MINUTES = 480;

/**
 * The float column's text for `minutes` of float on a `day`-minute working day: whole days as `3d`, else tenths
 * rounded half up (`2.5d`), whole days again from 100 days up, and `<0.1d` / `>-0.1d` for a nonzero float under 0.05 d.
 */
function expectedFloatText(minutes: number, day: number): string {
  if (minutes === 0) return "0d";
  const sign = minutes < 0 ? "-" : "";
  const magnitude = Math.abs(minutes);
  if (magnitude % day === 0) return `${sign}${magnitude / day}d`;
  const tenths = Math.floor((magnitude * 10 + Math.floor(day / 2)) / day);
  if (tenths === 0) return minutes < 0 ? ">-0.1d" : "<0.1d";
  if (tenths >= 1000) return `${sign}${Math.floor((magnitude + Math.floor(day / 2)) / day)}d`;
  return `${sign}${Math.floor(tenths / 10)}.${tenths % 10}d`;
}

/** What a CSS colour token resolves to on the chart root, as the browser prints it. */
async function resolved(page: Page, token: string): Promise<string> {
  return page.evaluate((name) => {
    const probe = document.createElement("div");
    probe.style.background = `var(${name})`;
    document.getElementById("app")!.append(probe);
    const colour = getComputedStyle(probe).backgroundColor;
    probe.remove();
    return colour;
  }, token);
}

test("a critical row has the diamond and a semibold red float; a plain row has neither", async ({ g }) => {
  await g.open({ n: SIZE });
  const critical = await g.page.evaluate(() => window.gantt!.app.test_first_critical_row());
  expect(critical, "the demo has a critical row").not.toBeUndefined();
  await reveal(g, critical!);
  const crit = await resolved(g.page, "--gantt-crit");
  const critText = await resolved(g.page, "--gantt-crit-text");

  const read = (row: number) =>
    g.page.evaluate((r) => {
      const el = document.querySelector<HTMLElement>(`#table-rows .row[data-row="${r}"]`)!;
      const diamond = getComputedStyle(el, "::before");
      const float = getComputedStyle(el.querySelector(".float")!);
      return {
        critical: el.classList.contains("critical"),
        diamond: diamond.content !== "none" && diamond.content !== "normal",
        diamondColour: diamond.backgroundColor,
        diamondSize: [diamond.width, diamond.height],
        floatColour: float.color,
        floatWeight: Number(float.fontWeight),
      };
    }, row);

  const hot = await read(critical!);
  expect(hot.critical).toBe(true);
  expect(hot.diamond, "the critical row draws a diamond").toBe(true);
  expect(hot.diamondColour).toBe(crit);
  expect(hot.diamondSize).toEqual(["6px", "6px"]);
  expect(hot.floatColour).toBe(critText);
  expect(hot.floatWeight).toBeGreaterThanOrEqual(600);

  const plain = await g.page.evaluate(() => {
    const row = [...document.querySelectorAll<HTMLElement>("#table-rows .row")].find(
      (el) => !el.classList.contains("critical") && el.dataset.kind === "0",
    );
    return row ? Number(row.dataset.row) : null;
  });
  expect(plain, "a plain leaf row is on screen").not.toBeNull();
  const cold = await read(plain!);
  expect(cold.diamond, "a plain row has no diamond").toBe(false);
  expect(cold.floatColour).not.toBe(critText);
});

test("float is shown in working days and agrees with the hover card's split", async ({ g }) => {
  await g.open({ n: SIZE });
  const rows = await g.page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("#table-rows .row")]
      .filter((el) => el.dataset.kind === "0")
      .map((el) => ({
        task: Number(el.dataset.task),
        text: el.querySelector(".float")!.textContent ?? "",
        fromApp: window.gantt!.app.task_float(Number(el.dataset.task)),
        details: window.gantt!.app.task_details(Number(el.dataset.task)) as {
          floatDays: number;
          floatHours: number;
          floatRemMinutes: number;
          floatMinutes: number;
        },
      })),
  );
  expect(rows.length, "leaf rows are on screen").toBeGreaterThan(5);
  for (const row of rows) {
    const d = row.details;
    expect(row.text, `task ${row.task}: the cell is the app's days text`).toBe(row.fromApp);
    // The demo generator uses four calendars (an 8 h, a 10 h, a 10.5 h and a 24 h day); the app does not export a
    // task's calendar, so the day lengths that agree with the hover card's split of this float are the candidates.
    const days = DEMO_DAY_LENGTHS.filter(
      (day) =>
        d.floatDays === Math.trunc(d.floatMinutes / day) &&
        d.floatHours === Math.trunc((d.floatMinutes % day) / 60) &&
        d.floatRemMinutes === (d.floatMinutes % day) % 60,
    );
    expect(days.length, `task ${row.task}: the card's split of ${d.floatMinutes} min fits a demo calendar`).toBeGreaterThan(0);
    expect(
      days.map((day) => expectedFloatText(d.floatMinutes, day)),
      `task ${row.task}: the cell shows ${d.floatMinutes} min as days on one of those calendars`,
    ).toContain(row.text);
  }
  // The rule itself, on known minutes (signed values and a non-8 hour day included). The app's Rust tests
  // (edit.rs, format_float_days) cover the same cases against the real function; this pins the spec's own copy.
  expect(expectedFloatText(12 * DEMO_DAY_MINUTES, DEMO_DAY_MINUTES)).toBe("12d");
  expect(expectedFloatText(DEMO_DAY_MINUTES + DEMO_DAY_MINUTES / 2, DEMO_DAY_MINUTES)).toBe("1.5d");
  expect(expectedFloatText(-(2 * DEMO_DAY_MINUTES + DEMO_DAY_MINUTES / 2), DEMO_DAY_MINUTES)).toBe("-2.5d");
  expect(expectedFloatText(1, DEMO_DAY_MINUTES)).toBe("<0.1d");
  expect(expectedFloatText(-1, DEMO_DAY_MINUTES)).toBe(">-0.1d");
  expect(expectedFloatText(3 * 1440, 1440)).toBe("3d");
  expect(expectedFloatText(480, 1440)).toBe("0.3d");
  expect(expectedFloatText(100 * DEMO_DAY_MINUTES + 100, DEMO_DAY_MINUTES)).toBe("100d");
});

test("every task's float text is in working days, not only the rows on screen", async ({ g }) => {
  await g.open({ n: SIZE });
  // Every row through the app (App.table_rows over the whole order), so short-float tasks far below scroll 0 count.
  const rows = await g.page.evaluate(() => {
    const app = window.gantt!.app;
    const all = JSON.parse(app.table_rows(0, app.test_row_count())) as [number, string, string, number | null, boolean, number][];
    return all
      .filter((r) => r[5] === 0)
      .map(([task, , , floatMinutes]) => ({
        task,
        floatMinutes,
        text: app.task_float(task),
        details: app.task_details(task) as { floatDays: number; floatHours: number; floatRemMinutes: number; floatMinutes: number },
      }));
  });
  expect(rows.length, "every leaf row is read").toBeGreaterThan(SIZE / 2);
  const short = rows.filter((r) => Math.abs(r.details.floatMinutes) < 100 * DEMO_DAY_MINUTES);
  expect(short.length, "the schedule has tasks with under 100 days of float").toBeGreaterThan(0);
  for (const row of rows) {
    const d = row.details;
    expect(row.floatMinutes, `task ${row.task}: the table row and the card agree on minutes`).toBe(d.floatMinutes);
    const days = DEMO_DAY_LENGTHS.filter(
      (day) =>
        d.floatDays === Math.trunc(d.floatMinutes / day) &&
        d.floatHours === Math.trunc((d.floatMinutes % day) / 60) &&
        d.floatRemMinutes === (d.floatMinutes % day) % 60,
    );
    expect(days.length, `task ${row.task}: the card's split of ${d.floatMinutes} min fits a demo calendar`).toBeGreaterThan(0);
    expect(
      days.map((day) => expectedFloatText(d.floatMinutes, day)),
      `task ${row.task}: the float text shows ${d.floatMinutes} min as days on one of those calendars`,
    ).toContain(row.text);
  }
});

test("a row shows the leaf name, and the outline path is its tooltip", async ({ g }) => {
  await g.open({ n: SIZE });
  const rows = await g.page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("#table-rows .row")].map((el) => {
      const task = Number(el.dataset.task);
      const name = el.querySelector<HTMLElement>(".name")!;
      return {
        task,
        cell: name.textContent ?? "",
        label: window.gantt!.app.task_label(task),
        path: window.gantt!.app.task_name(task),
        title: name.title,
      };
    }),
  );
  expect(rows.length).toBeGreaterThan(5);
  for (const row of rows) {
    expect(row.cell, `task ${row.task}: the cell is the leaf name`).toBe(row.label);
    expect(row.title, `task ${row.task}: the tooltip is the outline path`).toBe(row.path);
  }
  expect(
    rows.some((row) => row.path !== row.label),
    "at least one row is nested, so the leaf name differs from its path",
  ).toBe(true);
});

test("hover is mirrored between the table and the canvas", async ({ g }) => {
  await g.open({ n: SIZE });
  const hov = await resolved(g.page, "--gantt-hov");
  const marked = () =>
    g.page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-hover='true']")].map((el) => Number(el.dataset.row)));
  const fill = (row: number) =>
    g.page.evaluate((r) => getComputedStyle(document.querySelector(`#table-rows .row[data-row="${r}"]`)!).backgroundColor, row);
  const box = await g.box();
  const rowH = (await g.status()).rowHeight;
  const axis = (await g.status()).axisHeight;

  // Table to canvas: a pointer on a table row makes the canvas hover that row.
  const target = await g.page.evaluate(() => {
    const el = document.querySelector<HTMLElement>("#table-rows .row[data-row='4']")!;
    const r = el.getBoundingClientRect();
    return { x: r.left + 40, y: r.top + r.height / 2 };
  });
  await g.page.mouse.move(target.x, target.y);
  await g.page.waitForFunction(() => window.gantt!.status().hoverRow === 4);
  await g.nextFrame(2);
  expect((await g.status()).hoverRow, "the canvas hovers the table's row").toBe(4);
  expect(await marked(), "the table marks that row, and only it").toEqual([4]);
  expect(await fill(4), "the table row uses --gantt-hov").toBe(hov);

  // Canvas to table: a pointer on a canvas row makes the table mark that row.
  const y = box.y + axis + 9 * rowH + rowH / 2 - (await g.status()).scrollY;
  await g.page.mouse.move(box.x + box.width - 40, y);
  await g.page.waitForFunction(() => window.gantt!.status().hoverRow === 9);
  await g.nextFrame(2);
  expect(await marked(), "the table follows the canvas pointer").toEqual([9]);
  expect(await fill(9), "the canvas-driven table row uses --gantt-hov").toBe(hov);

  // Leaving both clears the mark.
  await g.page.mouse.move(box.x + box.width - 40, 2);
  await g.page.waitForFunction(() => window.gantt!.status().hoverRow === null);
  await g.nextFrame(2);
  expect(await marked(), "no row is marked once the pointer is away").toEqual([]);
});

const markedRows = (page: Page) =>
  page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-hover='true']")].map((el) => Number(el.dataset.row)));

test("a stationary table pointer follows the rows as they scroll past it", async ({ g }) => {
  await g.open({ n: SIZE });
  const rowH = (await g.status()).rowHeight;
  const target = await g.page.evaluate(() => {
    const body = document.getElementById("table-body")!.getBoundingClientRect();
    const r = document.querySelector<HTMLElement>("#table-rows .row[data-row='4']")!.getBoundingClientRect();
    return { x: r.left + 40, y: r.top + r.height / 2, offset: r.top + r.height / 2 - body.top };
  });
  await g.page.mouse.move(target.x, target.y);
  await g.page.waitForFunction(() => window.gantt!.status().hoverRow === 4);

  // Scroll the table under the pointer without moving it.
  await g.page.mouse.wheel(0, 3 * rowH);
  const expected = (scrollY: number) => Math.floor((scrollY + target.offset) / rowH);
  await g.page.waitForFunction(
    ([offset, h]) => {
      const s = window.gantt!.status();
      return s.scrollY >= 2 * h && s.hoverRow === Math.floor((s.scrollY + offset) / h);
    },
    [target.offset, rowH],
  );
  await g.nextFrame(3);
  const s = await g.status();
  expect(s.scrollY, "the view scrolled").toBeGreaterThanOrEqual(2 * rowH);
  expect(s.hoverRow, "the canvas hovers the row now under the pointer").toBe(expected(s.scrollY));
  expect(s.hoverRow, "and not the row the pointer first met").not.toBe(4);
  expect(await markedRows(g.page), "the table marks that row, and only it").toEqual([s.hoverRow]);
});

test("hover marks follow the canvas pointer while a cell is being edited", async ({ g }) => {
  await g.open({ n: SIZE });
  const box = await g.box();
  const { rowHeight, axisHeight } = await g.status();
  await g.page.locator("#table-rows .row[data-row='2'] .name").dblclick();
  await g.page.waitForSelector("#table-rows input.cell-edit");

  // Editing suspends rebuilding the rows, not the marks.
  const y = (row: number) => box.y + axisHeight + row * rowHeight + rowHeight / 2;
  await g.page.mouse.move(box.x + box.width - 40, y(6));
  await g.page.waitForFunction(() => window.gantt!.status().hoverRow === 6);
  await g.nextFrame(2);
  expect(await markedRows(g.page), "the table moves its mark to the canvas row").toEqual([6]);
  expect(await g.page.locator("#table-rows input.cell-edit").count(), "the editor is still open").toBe(1);

  await g.page.mouse.move(box.x + box.width - 40, 2);
  await g.page.waitForFunction(() => window.gantt!.status().hoverRow === null);
  await g.nextFrame(2);
  expect(await markedRows(g.page), "leaving the canvas clears the table's mark").toEqual([]);
  expect(await g.page.locator("#table-rows input.cell-edit").count(), "the editor is still open").toBe(1);
});
