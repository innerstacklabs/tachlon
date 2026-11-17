import { readFileSync } from "node:fs";
import { expect, test, type Gantt } from "./support/gantt";
import { EDITED, editDuration } from "./support/moved-note";
import type { Status } from "../src/api";
import type { Filter } from "../src/set-filter";
import type { ViewState } from "../src/view-state";

// gs#1194: a host page reads the chart's view state, hears when it changes and drives every filter, through `Gantt` alone
// (`window.gantt.api`, the object `mount` returned). The setup gives the filters something to work on: the ledger with an
// activity code type (loaded through `Gantt.load`), a baseline (the hook's `set_baseline`: the public baseline call is gs#1195)
// and an edit that moves other tasks (typed in the task list, as a person does), so Slipped, Float lost, Joined, Left and
// Show moved can all be turned on. Everything after the setup goes through `Gantt`.

const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");
const BASELINED = readFileSync(new URL("./fixtures/baselined.xml", import.meta.url), "utf8");
const PHASES = ["Civil", "MEP", "Site", "Fit-out", "Test", "Handover"];
// As filters.spec.ts: the ledger's TASKACTV rows already carry type 7000; the definition tables name it Phase.
const CODED = LEDGER.replace(
  /%E\s*$/,
  "%T\tACTVTYPE\n%F\tactv_code_type_id\tactv_code_type\n%R\t7000\tPhase\n" +
    "%T\tACTVCODE\n%F\tactv_code_id\tactv_code_type_id\tshort_name\n" +
    PHASES.map((phase, i) => `%R\t${7100 + i}\t7000\t${phase}\n`).join("") +
    "%E\n",
);

declare global {
  interface Window {
    __changes?: ViewState[];
  }
}

/** Open the demo, load the coded ledger through `Gantt.load`, and record every `change` on `window.__changes`. */
async function host(g: Gantt): Promise<void> {
  await g.open({ n: 50 });
  const loaded = await g.page.evaluate(async (text) => {
    const file = new File([new TextEncoder().encode(text)], "coded.xer", { type: "application/octet-stream" });
    return window.gantt!.api!.load({ file });
  }, CODED);
  expect(loaded).toEqual({ ok: true });
  await g.nextFrame(4);
  await g.page.evaluate(() => {
    window.__changes = [];
    window.gantt!.api!.on("change", (state) => window.__changes!.push(state));
  });
}

/** Give the baseline and move-report filters what they need: a baseline, then an edit that slips other tasks. */
async function baselineAndMove(g: Gantt): Promise<void> {
  await g.page.evaluate(() => window.gantt!.app.set_baseline());
  await g.nextFrame(3);
  await editDuration(g.page, "10d");
  await g.page.waitForFunction(() => window.gantt!.api!.state().moved !== null);
  await g.nextFrame(4);
}

const state = (g: Gantt): Promise<ViewState> => g.page.evaluate(() => window.gantt!.api!.state());
const changes = (g: Gantt): Promise<ViewState[]> => g.page.evaluate(() => window.__changes!);
const setFilter = (g: Gantt, filter: Filter): Promise<boolean | undefined> =>
  g.page.evaluate((f) => window.gantt!.api!.commands.setFilter(f), filter);

test("a host sets every filter through Gantt, reads it back with state(), and hears one change per change", async ({ g }) => {
  await host(g);
  await baselineAndMove(g);
  const start = await state(g);
  expect(start.filter).toBe("off");
  expect(start.taskCount).toBeGreaterThan(0);
  expect(start.rowCount).toBeGreaterThan(0);
  expect(start.backend).toBe(await g.page.evaluate(() => window.gantt!.api!.backend()));

  // Each step: the filter it sets, and what state() reads back at once, before any frame.
  const steps: [Filter, Partial<ViewState>][] = [
    ["critical", { filter: "critical" }],
    ["longestPath", { filter: "longestPath" }],
    ["nearCritical", { filter: "nearCritical" }],
    ["slipped", { filter: "slipped" }],
    ["floatLost", { filter: "floatLost" }],
    ["joinedCritical", { filter: "joinedCritical" }],
    ["leftCritical", { filter: "leftCritical" }],
    ["moved", { filter: "moved" }],
    ["startConstraint", { filter: "startConstraint" }],
    [{ status: "not-started" }, { filter: "status", statusFilter: "not-started" }],
    [{ status: "complete" }, { filter: "status", statusFilter: "complete" }],
    [{ code: { type: "Phase", value: "Civil" } }, { filter: "code", codeFilter: { type: "Phase", value: "Civil" } }],
    [{ code: { type: "Phase", value: "MEP" } }, { filter: "code", codeFilter: { type: "Phase", value: "MEP" } }],
    [{ lookahead: { start: "2024-01-08", weeks: 2 } }, { filter: "code", lookahead: { on: true, start: "2024-01-08", weeks: 2 } }],
    [{ lookahead: null }, { filter: "code" }],
    ["off", { filter: "off", statusFilter: null, codeFilter: null, resourceFilter: null }],
  ];
  for (const [filter, expected] of steps) {
    const label = JSON.stringify(filter);
    const before = (await changes(g)).length;
    expect(await setFilter(g, filter), `${label} is taken`).toBe(true);
    const now = await state(g);
    expect(now, `${label} reads back at once`).toMatchObject(expected);
    await g.nextFrame(4);
    const seen = await changes(g);
    expect(seen.length - before, `${label} fires one change`).toBe(1);
    expect(seen.at(-1), `${label}: the change carries the state`).toEqual(await state(g));
  }
  const off = await state(g);
  expect(off.lookahead.on).toBe(false);
  expect(off.rowCount, "every row is back").toBe(start.rowCount);

  // clearFilters turns the row filter and the lookahead off together: one change.
  await setFilter(g, "critical");
  await setFilter(g, { lookahead: { start: "2024-01-08", weeks: 3 } });
  await g.nextFrame(4);
  const before = (await changes(g)).length;
  expect(await g.page.evaluate(() => window.gantt!.api!.commands.clearFilters())).toBe(true);
  expect(await state(g)).toMatchObject({ filter: "off", lookahead: { on: false } });
  await g.nextFrame(4);
  expect((await changes(g)).length - before, "clearFilters fires one change").toBe(1);
});

test("change fires at most once a frame, and never on an idle chart or a fading move note", async ({ g }) => {
  await host(g);
  // Two filters set in the same task land in one frame: one change, carrying both.
  await g.page.evaluate(() => {
    const commands = window.gantt!.api!.commands;
    commands.setFilter("critical");
    commands.setFilter({ lookahead: { start: "2024-01-08", weeks: 2 } });
  });
  await g.nextFrame(4);
  const both = await changes(g);
  expect(both).toHaveLength(1);
  expect(both[0]).toMatchObject({ filter: "critical", lookahead: { on: true, weeks: 2 } });

  // A filter set to what it already is changes nothing and fires nothing.
  expect(await setFilter(g, "critical")).toBe(true);
  await g.nextFrame(30);
  expect(await changes(g), "an idle chart fires no change").toHaveLength(1);

  // The move note holds for 4 s, then fades frame by frame (`moved.alpha`); the state carries the report, not the fade. The hook's
  // status is read only to know when the fade has ended.
  await g.page.evaluate(() => window.gantt!.api!.commands.clearFilters());
  await g.nextFrame(4);
  await editDuration(g.page, "10d");
  await g.page.waitForFunction(() => window.gantt!.api!.state().moved !== null);
  // Off the table and the note, so nothing holds the note or changes the hover.
  await g.page.mouse.move(1, 1);
  await g.nextFrame(4);
  const settled = (await changes(g)).length;
  await g.page.waitForFunction(() => window.gantt!.status().moved?.alpha === 0, undefined, { timeout: 30_000 });
  await g.nextFrame(4);
  expect((await changes(g)).length, "no change while the move note faded").toBe(settled);
  expect((await state(g)).moved, "the report stays after its note faded").not.toBeNull();
});

test("selecting a task is a change, and state() reads the selection", async ({ g }) => {
  await host(g);
  const task = await g.page.evaluate(() => (JSON.parse(window.gantt!.app.table_rows(2, 3)) as [number][])[0]![0]);
  expect(await g.page.evaluate((t) => window.gantt!.api!.commands.select(t), task)).toBe(true);
  expect((await state(g)).selection?.task).toBe(task);
  await g.nextFrame(4);
  const seen = await changes(g);
  expect(seen).toHaveLength(1);
  expect(seen[0]!.selection?.task).toBe(task);
});

test("a refused filter returns false, changes nothing and fires nothing", async ({ g }) => {
  await host(g);
  const refused: Filter[] = [
    "slipped", // no baseline yet
    "joinedCritical",
    "moved", // no move report yet
    { status: "started" as never },
    { code: { type: "Phase", value: "Nowhere" } },
    { lookahead: { start: "not a date", weeks: 2 } },
    { lookahead: { start: "2024-01-08", weeks: 0 } },
    // Past the 520 weeks the app takes, and past 2^32, which would wrap in the module's u32.
    { lookahead: { start: "2024-01-08", weeks: 521 } },
    { lookahead: { start: "2024-01-08", weeks: 2 ** 32 + 2 } },
    // Wrong shapes give false, never a throw.
    { code: null } as never,
    { lookahead: undefined } as never,
    { status: 42 } as never,
    "toString" as never,
  ];
  for (const filter of refused) expect(await setFilter(g, filter), JSON.stringify(filter)).toBe(false);
  expect(await state(g)).toMatchObject({ filter: "off", lookahead: { on: false } });
  await g.nextFrame(6);
  expect(await changes(g)).toHaveLength(0);
});

test("filterInputs() reads the activity codes, whether the file has activity IDs, and whether the baseline carries float", async ({ g }) => {
  await host(g);
  const before = await g.page.evaluate(() => window.gantt!.api!.filterInputs());
  expect(before.hasCodes).toBe(true);
  expect(before.baselineHasFloat).toBe(false);
  expect(before.activityCodes.find(([type]) => type === "Phase")?.[1]).toEqual(expect.arrayContaining(PHASES.slice(0, 2)));
  await g.page.evaluate(() => window.gantt!.app.set_baseline());
  await g.nextFrame(3);
  expect((await g.page.evaluate(() => window.gantt!.api!.filterInputs())).baselineHasFloat).toBe(true);
});

test("a baseline set over a file's, with the same rows and no data date, is a change, so a host reads baselineHasFloat again", async ({ g }) => {
  // Both tasks carry a file baseline (no float) and the file has no data date: setBaseline keeps `{ rows: 2, dataDate: null }`.
  const xml = BASELINED.replace("</Duration><DurationFormat>7</DurationFormat></Task>", "</Duration><DurationFormat>7</DurationFormat><Baseline><Number>0</Number><Start>2026-01-05T08:00:00</Start><Finish>2026-01-05T16:00:00</Finish></Baseline></Task>");
  await g.open({ n: 5 });
  expect(await g.page.evaluate(async (text) => window.gantt!.api!.load({ file: new File([text], "baselined.xml") }), xml)).toEqual({ ok: true });
  await g.nextFrame(4);
  expect((await state(g)).baseline).toEqual({ rows: 2, dataDate: null, dayText: null, slipped: 1, added: 0 });
  expect((await g.page.evaluate(() => window.gantt!.api!.filterInputs())).baselineHasFloat).toBe(false);
  await g.page.evaluate(() => {
    window.__changes = [];
    window.gantt!.api!.on("change", (s) => window.__changes!.push(s));
  });
  await g.nextFrame(4);
  expect(await changes(g), "the chart is idle").toEqual([]);
  expect(await g.page.evaluate(() => window.gantt!.api!.commands.setBaseline())).toEqual({ ok: true });
  await g.nextFrame(4);
  const heard = await changes(g);
  expect(heard.length, "one change for the new baseline").toBe(1);
  expect(heard[0]!.baseline).toEqual({ rows: 2, dataDate: null, dayText: null, slipped: 0, added: 0 });
  expect((await g.page.evaluate(() => window.gantt!.api!.filterInputs())).baselineHasFloat).toBe(true);
});

test("Status.level is a ZoomLevel, so a level string the axis never writes is a type error", () => {
  // @ts-expect-error "fortnight" is not a ZoomLevel; with `level: string` this line compiled and `pnpm typecheck` failed here.
  const wrong: Status["level"] = "fortnight";
  expect(wrong).toBe("fortnight");
});

test("each other field of the state is one change: an edit that moves tasks, a collapse, an insert, a zoom across a level, a load", async ({ g }) => {
  await host(g);
  const start = await state(g);
  expect(start.selection, "nothing is selected, so no step below also moves the selection").toBeNull();
  /** Run `act`, let frames pass, and return the one change it fired. */
  const one = async (label: string, act: () => Promise<unknown>): Promise<ViewState> => {
    const before = (await changes(g)).length;
    await act();
    await g.nextFrame(4);
    const seen = await changes(g);
    expect(seen.length - before, `${label} fires one change`).toBe(1);
    expect(seen.at(-1), `${label}: the change carries the state`).toEqual(await state(g));
    return seen.at(-1)!;
  };
  // The edits and the collapse go through the hook: the public row and outline calls are gs#1195's. Each step moves only the
  // field it is about (and the insert, the counts it changes), so dropping that field's comparison from `same` turns it red.

  // An edit that moves other tasks: only the report (`moved`).
  const edited = await one("an edit that moves tasks", () => g.page.evaluate((t) => window.gantt!.app.set_duration(t, "10d"), EDITED));
  expect(edited.moved?.others).toBeGreaterThan(0);
  // The edit also marks the file unsaved (tl#1289), which host-open-state.spec.ts reads.
  expect({ ...edited, moved: null, unsaved: false }).toEqual({ ...start, moved: null });

  // Collapse to the first level: only the row count.
  const collapsed = await one("a collapse", () => g.page.evaluate(() => window.gantt!.app.collapse_to_level(1)));
  expect(collapsed.rowCount).toBeLessThan(start.rowCount);
  expect({ ...collapsed, rowCount: 0 }).toEqual({ ...edited, rowCount: 0 });

  // An insert (the last top-level row, then the selection cleared in the same task): one more task and one more row, and the
  // edit's report cleared, as any other change clears it.
  const inserted = await one("an insert", () =>
    g.page.evaluate(() => {
      window.gantt!.app.insert_task();
      window.gantt!.app.clear_selection();
    }),
  );
  expect(inserted).toMatchObject({ taskCount: start.taskCount + 1, rowCount: collapsed.rowCount + 1, moved: null, selection: null });

  // Zoom in step by step until the axis changes its unit; the steps that do not cross a level fire nothing.
  const zoomed = await one("a zoom across a level", async () => {
    for (let k = 0; k < 12 && (await state(g)).zoomLevel === start.zoomLevel; k++) {
      await g.page.evaluate(() => window.gantt!.api!.commands.zoomIn());
      await g.nextFrame(2);
    }
  });
  expect(zoomed.zoomLevel).not.toBe(start.zoomLevel);
  expect({ ...zoomed, zoomLevel: "day" }).toEqual({ ...inserted, zoomLevel: "day" });

  // A load replaces the schedule (a new epoch): one change, with the file's own counts back, and one open.
  await g.page.evaluate(() => {
    const seen: unknown[] = [];
    window.gantt!.api!.on("open", (detail) => seen.push(detail));
    (window as unknown as { __opens: unknown[] }).__opens = seen;
  });
  const loaded = await one("a load", () =>
    g.page.evaluate(async (text) => {
      const file = new File([new TextEncoder().encode(text)], "coded.xer", { type: "application/octet-stream" });
      return window.gantt!.api!.load({ file });
    }, CODED),
  );
  expect(loaded).toMatchObject({ taskCount: start.taskCount, rowCount: start.rowCount, moved: null, selection: null });
  expect(await g.page.evaluate(() => (window as unknown as { __opens: unknown[] }).__opens.length), "and one open").toBe(1);
});

test("a state() read between frames does not make the next change repeat the last one", async ({ g }) => {
  await host(g);
  // On, read, off, read, all before a frame: the frame sees the state it last sent and fires nothing.
  await g.page.evaluate(() => {
    const api = window.gantt!.api!;
    api.commands.setFilter("critical");
    api.state();
    api.commands.setFilter("off");
    api.state();
  });
  await g.nextFrame(6);
  expect(await changes(g)).toHaveLength(0);
});
