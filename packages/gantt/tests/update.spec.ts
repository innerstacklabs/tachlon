import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { Gantt } from "../src/index";
import { chromePath } from "./support/knobs";

// gantt#2087: a host hands the chart new values for many tasks by key, in place: the selection, the scroll and the undo list stay as they were.
declare global {
  interface Window { upChart: Gantt; edits: unknown[]; mountUp(options: Record<string, unknown>): Promise<void>; render(props: object): void; starts: number; answers: unknown[] }
}
// gantt#2087: the same host as a React page: `render(props)` renders the component with the props it was given so far, and `records` reaches the chart as `update`.
const REACT_MODULE = `import { createElement as h } from "react"; import { createRoot } from "react-dom/client"; import { Gantt } from "/src/react.tsx";
  const root = createRoot(document.body.appendChild(document.createElement("div"))), props = { forceWebgl: true, style: { width: 1000, height: 500 }, onEdit: (change) => window.edits.push(change), onReady: (chart) => ((window.upChart = chart), (window.starts += 1)), onUpdate: (answer) => window.answers.push(answer) };
  Object.assign(window, { edits: [], answers: [], starts: 0, render: (more) => root.render(h(Gantt, Object.assign(props, more))), ready: true });`;
const reactHost = { name: "react-host", resolveId: (id: string) => (id === "/tests/react-update-host.ts" ? id : null), load: (id: string) => (id === "/tests/react-update-host.ts" ? REACT_MODULE : null) };
const REACT_HOST = `<!doctype html><html><body style="margin: 0"><script type="module">import "/tests/react-update-host.ts";</script></body></html>`;
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.edits = [];
  window.mountUp = async (options) => {
    window.upChart = await mount(document.getElementById("app"), { forceWebgl: true, ...options });
    window.upChart.on("edit", (change) => window.edits.push(change));
  };
  window.ready = true;
</script></body></html>`;
let server: ViteDevServer;
let base = "";
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", plugins: [reactHost], server: { host: "127.0.0.1", port: 0, strictPort: false } });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});
test.afterAll(async () => server.close());
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"] } });

const tasks = (n: number, extra: object = {}) => Array.from({ length: n }, (_, i) => ({ id: `t${i}`, key: `k${i}`, name: `Task ${i}`, start: "2026-03-02", duration: 2, ...extra }));
async function mountHost(page: Page, options: Record<string, unknown>): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
  await page.evaluate((o) => window.mountUp(o), options);
}
const frames = (page: Page) => page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
const saved = (page: Page) => page.evaluate(() => (window.upChart.toDocument() as { tasks: { key?: string; name: string; duration: number }[] }).tasks);
const minutes = async (page: Page, keys: string[]) => { const all = await saved(page); return keys.map((key) => all.find((t) => t.key === key)!.duration); };
const keys = (from: number, to: number) => Array.from({ length: to - from }, (_, i) => `k${from + i}`);
const scrollTop = (page: Page) => page.evaluate(() => document.querySelector("#table-body")!.scrollTop);

test("update sets 20 tasks in place: selection, scroll and undo hold, no edit fires, the table shows the new durations", async ({ page }) => {
  await mountHost(page, { tasks: tasks(40) });
  await page.locator('#table-rows .row[data-row="2"] .name').dblclick();
  await page.locator("#table-rows input.cell-edit").fill("Renamed");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.edits.length > 0);
  await page.evaluate(() => { window.edits.length = 0; });
  await expect.poll(async () => {
    await page.evaluate(() => { document.querySelector("#table-body")!.scrollTop = 25 * document.querySelector("#table-rows .row")!.getBoundingClientRect().height; });
    await frames(page);
    return page.locator('#table-rows .row[data-row="30"]').count();
  }).toBe(1);
  await page.locator('#table-rows .row[data-row="30"]').click({ position: { x: 4, y: 4 } });
  await frames(page);
  const before = { selected: await page.evaluate(() => window.upChart.state().selection?.task), top: await scrollTop(page) };
  expect(before.selected, "a task is selected").toBeDefined();
  expect(before.top, "the list is scrolled").toBeGreaterThan(0);
  const answer = await page.evaluate((ks) => window.upChart.update(ks.map((key, i) => ({ key, duration: 5, ...(i === 0 ? { snet: "2026-03-09" } : {}) }))), keys(0, 20));
  await frames(page);
  expect(answer).toEqual({ ok: true, updated: 20 });
  expect(await minutes(page, keys(0, 20)), "the 20 new durations (5 days of 480 minutes)").toEqual(Array(20).fill(2400));
  expect(await minutes(page, keys(20, 40)), "the other tasks keep theirs").toEqual(Array(20).fill(960));
  expect({ selected: await page.evaluate(() => window.upChart.state().selection?.task), top: await scrollTop(page) }, "selection and scroll are as read before the call").toEqual(before);
  expect(await page.evaluate(() => window.edits), "an update is not an edit").toEqual([]);
  await page.evaluate(() => { document.querySelector("#table-body")!.scrollTop = 0; });
  await frames(page);
  expect(await page.locator("#table-rows .row .dur").evaluateAll((cells) => cells.slice(0, 10).map((c) => c.textContent)), "the table shows the new durations").toEqual(Array(10).fill("5d"));
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  expect((await saved(page))[2]!.name, "undo undoes the rename").toBe("Task 2");
  expect(await minutes(page, keys(0, 20)), "and leaves the 20 durations").toEqual(Array(20).fill(2400));
});

test("a refused update changes no cell: an unknown key answers with its record and key, a value that is not an array is badRecords", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  const before = JSON.stringify(await saved(page));
  const unknown = await page.evaluate(() => window.upChart.update([{ key: "k1", duration: 9 }, { key: "nope", duration: 1 }]));
  expect(unknown).toMatchObject({ ok: false, reason: "unknownKey", record: 1, key: "nope" });
  expect(await page.evaluate(() => window.upChart.update("k1" as never))).toMatchObject({ ok: false, reason: "badRecords" });
  expect(JSON.stringify(await saved(page)), "nothing changed, k1 included").toBe(before);
});

test("a call while a cell is being edited answers busy and changes nothing", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  const before = JSON.stringify(await saved(page));
  await page.locator('#table-rows .row[data-row="1"] .dur').dblclick();
  await expect(page.locator("#table-rows input.cell-edit")).toBeVisible();
  expect(await page.evaluate(() => window.upChart.update([{ key: "k1", duration: 9 }]))).toMatchObject({ ok: false, reason: "busy" });
  expect(JSON.stringify(await saved(page))).toBe(before);
});

test("a duration is counted in the unit the chart mounted with: hours, minutes, and days of the task's own calendar", async ({ page }) => {
  const set = (options: Record<string, unknown>, duration: number, key = "k0") => mountHost(page, options).then(() => page.evaluate(([k, d]) => window.upChart.update([{ key: k as string, duration: d as number }]), [key, duration])).then(() => minutes(page, [key]));
  expect(await set({ tasks: tasks(2), durationUnit: "hours" }, 12)).toEqual([720]);
  expect(await set({ tasks: tasks(2), durationUnit: "minutes" }, 89.6)).toEqual([90]);
  const ten = { key: "ten", workDays: [1, 2, 3, 4, 5], hoursPerDay: 10 };
  const calendars = [{ key: "std", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 }, ten];
  expect(await set({ tasks: [{ ...tasks(1)[0]!, calendar: "ten" }], calendars }, 2)).toEqual([1200]);
  expect(await set({ tasks: tasks(1), calendars }, 2), "a task on the first calendar counts its 8-hour day").toEqual([960]);
  const document = await page.evaluate(() => window.upChart.toDocument());
  expect(await set({ document, durationUnit: "day" }, 3), "a unit that is not one of the three counts days, as the reader does").toEqual([1440]);
});

test("a null start clears the floor a record gave: the type takes it and the chart leaves the date", async ({ page }) => {
  await mountHost(page, { tasks: tasks(3) });
  const start = () => page.evaluate(() => window.upChart.snapshot().tasks[0]!.start);
  expect(await page.evaluate(() => window.upChart.update([{ key: "k0", start: "2026-03-09" }]))).toEqual({ ok: true, updated: 1 });
  const floored = await start();
  expect(floored, "the record gave k0 a later start").toMatch(/^2026-03-09/);
  expect(await page.evaluate(() => window.upChart.update([{ key: "k0", start: null }]))).toEqual({ ok: true, updated: 1 });
  expect(await start(), "start: null takes the floor away").not.toBe(floored);
});

test("a constraint is the one date limit: it replaces the other bounds, null clears them, a record with two spellings is refused, and a host edit tells the edit event in the same form", async ({ page }) => {
  await mountHost(page, { tasks: tasks(2).map((t, i) => (i === 0 ? { ...t, constraint: { type: "snlt", date: "2026-03-20" } } : t)) });
  const limits = () => page.evaluate(() => { const { snet, snlt, fnet, fnlt } = window.upChart.snapshot().tasks[0] as Record<string, unknown>; return { snet, snlt, fnet, fnlt }; });
  expect(await limits(), "a constraint in the rows at mount is the limit its type names").toMatchObject({ snlt: expect.stringMatching(/^2026-03-20/) });
  expect(await page.evaluate(() => (window.upChart.toDocument() as { tasks: { snet: number | null }[] }).tasks[0]!.snet), "the start of a row with a constraint is no floor").toBeNull();
  expect(await page.evaluate(() => window.upChart.update([{ key: "k0", constraint: { type: "fnlt", date: "2026-03-27" } }]))).toEqual({ ok: true, updated: 1 });
  expect(await limits(), "the new limit replaces the old one").toMatchObject({ fnlt: "2026-03-27T17:00", snlt: undefined });
  expect(await page.evaluate(() => window.upChart.update([{ key: "k0", constraint: null }]))).toEqual({ ok: true, updated: 1 });
  expect(await limits(), "null clears every bound").toEqual({ snet: undefined, snlt: undefined, fnet: undefined, fnlt: undefined });
  expect(await page.evaluate(() => window.upChart.update([{ key: "k0", constraint: { type: "snet", date: "2026-03-09" }, snlt: "2026-03-20" }]))).toMatchObject({ ok: false, reason: "badRecords", message: expect.stringContaining('both "constraint" and "snlt"') });
  expect(await page.evaluate(() => window.upChart.hostEdit([{ key: "k1", constraint: { type: "fnet", date: "2026-03-30" } }]))).toEqual({ ok: true, updated: 1 });
  await expect.poll(() => page.evaluate(() => window.edits.length)).toBe(1);
  const [edit] = await page.evaluate(() => window.edits as { commits: { changes: { key: string; set: unknown; was: unknown }[] }[] }[]);
  expect(edit!.commits[0]!.changes, "the edit event carries the one member in both set and was").toMatchObject([{ key: "k1", set: { constraint: { type: "fnet", date: "2026-03-30T17:00" } }, was: { constraint: null } }]);
});

test("rows with a constraint, an object and null, are no floor at mount and setData with the same rows changes nothing; the separate form still floors a start and writes six members", async ({ page }) => {
  const rows = () => tasks(3).map((t, i) => (i === 0 ? { ...t, constraint: { type: "fnet", date: "2026-03-20" } } : i === 1 ? { ...t, constraint: null } : t));
  await mountHost(page, { tasks: rows() });
  const floors = () => page.evaluate(() => (window.upChart.toDocument() as { tasks: { snet: number | null }[] }).tasks.map((t) => t.snet));
  expect(await floors(), "the two rows with a constraint have no floor from their start; the third keeps its start as a floor").toEqual([null, null, expect.any(Number)]);
  expect(await page.evaluate((r) => window.upChart.setData({ tasks: r } as never), rows()), "the same rows again").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate((r) => window.upChart.setData({ tasks: r.map((t, i) => (i === 0 ? { ...t, constraint: null } : t)) } as never), rows())).toEqual({ ok: true, updated: 1 });
  expect(await page.evaluate(() => window.edits.length), "setData is not an edit").toBe(0);

  await mountHost(page, { tasks: tasks(2).map((t, i) => (i === 0 ? { ...t, snlt: "2026-03-20" } : t)), constraintForm: "separate" });
  expect(await floors(), "separate spelling: the start is a floor, as before").toEqual([expect.any(Number), expect.any(Number)]);
  expect(await page.evaluate(() => window.upChart.hostEdit([{ key: "k1", snlt: "2026-03-30" }]))).toEqual({ ok: true, updated: 1 });
  await expect.poll(() => page.evaluate(() => window.edits.length)).toBe(1);
  const [edit] = await page.evaluate(() => window.edits as { commits: { changes: { set: Record<string, unknown> }[] }[] }[]);
  expect(Object.keys(edit!.commits[0]!.changes[0]!.set), "under separate a limit is its own member, not constraint").toEqual(["snlt"]);
});

// to#144: a host that saves and rereads its rows gives back the pin the chart holds, spelled as its database writes it; `update` refuses a pin only when it differs from the one held, so `setData` goes on with the rest.
const pinned = (pins: Record<number, object>, name = "Task 1") => tasks(4).map((t, i) => ({ ...t, ...(i === 1 ? { name } : {}), ...pins[i] }));
const names = (page: Page) => page.evaluate(() => window.upChart.snapshot().tasks.map((t) => t.name));

test("setData after a reread of the pin the chart holds sets the rest: mso and mfo, each spelled another way, and a pin that differs is still refused", async ({ page }) => {
  await mountHost(page, { tasks: pinned({ 0: { mso: "2026-03-09" }, 2: { mfo: "2026-03-10" } }), constraintForm: "separate" });
  const reread = (pins: Record<number, object>, name?: string) => page.evaluate((r) => window.upChart.setData({ tasks: r } as never), pinned(pins, name));
  expect(await reread({ 0: { mso: "2026-03-09T08:00" }, 2: { mfo: "2026-03-10T17:00" } }, "Renamed")).toEqual({ ok: true, updated: 3 });
  expect((await names(page))[1], "the name is set").toBe("Renamed");
  expect(await reread({ 0: { mso: "2026-03-09T08:00" }, 2: { mfo: "2026-03-10T17:00" } }, "Renamed"), "the same rows again").toEqual({ ok: true, updated: 0 });
  expect(await reread({ 0: { mso: "2026-03-10" }, 2: { mfo: "2026-03-10T17:00" } }, "Again")).toMatchObject({ ok: false, reason: "notUpdatable", member: "mso", key: "k0" });
  expect((await names(page))[1], "a refused call sets nothing").toBe("Renamed");
  expect(await page.evaluate(() => window.edits.length), "setData is not an edit").toBe(0);
});

test("setData with mso and mfo null against rows that left them out sets the rest", async ({ page }) => {
  await mountHost(page, { tasks: pinned({}), constraintForm: "separate" });
  const nulls = Object.fromEntries([0, 1, 2, 3].map((i) => [i, { mso: null, mfo: null }]));
  expect(await page.evaluate((r) => window.upChart.setData({ tasks: r } as never), pinned(nulls, "Renamed"))).toEqual({ ok: true, updated: 4 });
  expect((await names(page))[1]).toBe("Renamed");
});

test("setData after a reread of a typed constraint the chart holds sets the rest: mso, mfo, so and fo", async ({ page }) => {
  const at = (type: string, date: string) => ({ constraint: { type, date } });
  await mountHost(page, { tasks: pinned({ 0: at("mso", "2026-03-09"), 1: at("so", "2026-03-09"), 2: at("mfo", "2026-03-10"), 3: at("fo", "2026-03-10") }) });
  const rows = pinned({ 0: at("mso", "2026-03-09T08:00"), 1: at("so", "2026-03-09T08:00"), 2: at("mfo", "2026-03-10T17:00"), 3: at("fo", "2026-03-10T17:00") }, "Renamed");
  expect(await page.evaluate((r) => window.upChart.setData({ tasks: r } as never), rows)).toEqual({ ok: true, updated: 4 });
  expect((await names(page))[1], "the name is set").toBe("Renamed");
  expect(await page.evaluate((r) => window.upChart.setData({ tasks: r.map((t, i) => (i === 2 ? { ...t, constraint: { type: "mfo", date: "2026-03-11" } } : t)) } as never), rows)).toMatchObject({ ok: false, reason: "notUpdatable", member: "mfo", key: "k2" });
});

const shown = (page: Page) => page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-task] > span")].map((c) => ({ text: c.textContent, marked: c.dataset.changed === "true", motion: getComputedStyle(c).animationName })));

test("an update highlights the cells in view whose shown value changed and no others, and the mark goes after its timer", async ({ page }) => {
  await mountHost(page, { tasks: tasks(40) });
  const before = await shown(page);
  await page.evaluate((ks) => window.upChart.update(ks.map((key) => ({ key, duration: 5 }))), keys(0, 20));
  await expect(page.locator("#table-rows [data-changed]").first()).toBeAttached();
  const after = await shown(page);
  expect(after.map((c) => c.marked), "a cell is marked when its text differs from the one shown before").toEqual(after.map((c, i) => c.text !== before[i]!.text));
  expect(after.filter((c) => c.marked).length, "some changed").toBeGreaterThan(0);
  expect(after.filter((c) => !c.marked).length, "some did not").toBeGreaterThan(0);
  expect(after.find((c) => c.marked)!.motion, "with motion the mark fades").toBe("gantt-changed");
  await expect(page.locator("#table-rows [data-changed]")).toHaveCount(0, { timeout: 4000 });
});

test("an update that sends the values already shown marks nothing, and with reduced motion the mark has no animation", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await mountHost(page, { tasks: tasks(40) });
  await page.evaluate((ks) => window.upChart.update(ks.map((key) => ({ key, duration: 2 }))), keys(0, 20));
  await frames(page);
  expect((await shown(page)).filter((c) => c.marked), "the same values change no cell").toEqual([]);
  await page.evaluate((ks) => window.upChart.update(ks.map((key) => ({ key, duration: 5 }))), keys(0, 20));
  await expect(page.locator("#table-rows [data-changed]").first()).toBeAttached();
  const marked = (await shown(page)).filter((c) => c.marked).map((c) => c.motion);
  expect([marked.length > 0, marked.every((a) => a === "none")], "marked, with no animation").toEqual([true, true]);
});

test("a pending key the update wrote is no longer pending, and a refused update marks and clears nothing", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  const pending = () => page.locator("#table-rows .row[data-pending]").evaluateAll((rows) => rows.map((r) => (r as HTMLElement).dataset.row));
  await page.evaluate(() => window.upChart.setPending(["k1", "k2"]));
  await page.evaluate(() => window.upChart.update([{ key: "k1", duration: 9 }, { key: "nope", duration: 1 }]));
  await frames(page);
  expect(await pending(), "a refused update leaves both pending").toEqual(["1", "2"]);
  expect(await shown(page).then((cells) => cells.filter((c) => c.marked)), "and marks nothing").toEqual([]);
  await page.evaluate(() => window.upChart.update([{ key: "k1", duration: 9 }]));
  await frames(page);
  expect(await pending(), "only the task written leaves").toEqual(["2"]);
});

test("an update followed by a load before the next frame highlights nothing in the new schedule", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  await page.evaluate(async (ks) => {
    const document = window.upChart.toDocument() as { tasks: { duration: number }[] };
    window.upChart.update(ks.map((key) => ({ key, duration: 5 })));
    await window.upChart.load({ document: { ...document, tasks: document.tasks.map((t) => ({ ...t, duration: 9 })) } });
  }, keys(0, 10));
  await frames(page);
  expect(await page.locator("#table-rows [data-changed]").count(), "the new schedule's cells are not marked").toBe(0);
});

test("a host's own finish dates go through App.update_tasks and update: the one that matches is not named, the one a day late is, and a call with none has no differs", async ({ page }) => {
  await mountHost(page, { tasks: tasks(3) });
  const records = [{ key: "k0", hostStart: "2026-03-02", hostFinish: "2026-03-03" }, { key: "k1", hostFinish: "2026-03-04" }];
  const named = [{ record: 1, key: "k1", member: "finish", host: "2026-03-04", chart: "2026-03-03" }];
  const app = await page.evaluate((r) => JSON.parse(window.gantt!.app.update_tasks(JSON.stringify(r))) as unknown, records);
  expect(app, "the app names the late finish only").toEqual({ ok: true, updated: 2, differs: named });
  expect(await page.evaluate((r) => window.upChart.update(r), records), "update hands the same list to the host").toEqual({ ok: true, updated: 2, differs: named });
  expect(await page.evaluate(() => window.upChart.update([{ key: "k0", hostFinish: "2026-03-03" }])), "a host date that matches leaves no differs member").toStrictEqual({ ok: true, updated: 1 });
});

test("under dateForm day, update and setData read a date alone on the task's calendar and refuse a date with a time as badValue, naming dateForm", async ({ page }) => {
  await mountHost(page, { tasks: tasks(2), dateForm: "day" });
  const limits = () => page.evaluate(() => { const { start, fnlt } = window.upChart.snapshot().tasks[0] as Record<string, unknown>; return { start, fnlt }; });
  const refusal = { ok: false, reason: "badValue", record: 0, member: "fnlt", key: "k0", message: expect.stringContaining('dateForm is "day"') };
  const untouched = { start: "2026-03-02T08:00", fnlt: undefined };
  expect(await page.evaluate(() => window.upChart.update([{ key: "k0", fnlt: "2026-03-27T17:00" }])), "update refuses the minute form").toMatchObject(refusal);
  const rows = tasks(2).map((task, i) => (i === 0 ? { ...task, fnlt: "2026-03-27T17:00" } : task));
  expect(await page.evaluate((r) => window.upChart.setData({ tasks: r }), rows), "setData refuses it too").toMatchObject(refusal);
  expect(await limits(), "a refused call changes nothing").toEqual(untouched);
  const day = [{ key: "k0", snet: "2026-03-09", fnlt: "2026-03-27" }];
  expect(await page.evaluate((r) => window.upChart.update(r), day)).toEqual({ ok: true, updated: 1 });
  const read = { start: "2026-03-09T08:00", fnlt: "2026-03-27T17:00" };
  expect(await limits(), "a start is the first working minute of its day, a finish the end of the last").toEqual(read);
  expect(await page.evaluate((r) => window.upChart.update(r), day), "the dates written back").toEqual({ ok: true, updated: 1 });
  expect(await limits(), "leave the task where it was").toEqual(read);
});

test("setData sets only the 20 changed durations: selection, scroll and the undo list hold, old rows do not undo a rename, and the same rows again do nothing", async ({ page }) => {
  await mountHost(page, { tasks: tasks(40) });
  await page.locator('#table-rows .row[data-row="2"] .name').dblclick();
  await page.locator("#table-rows input.cell-edit").fill("Renamed");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.edits.length > 0);
  await page.evaluate(() => { window.edits.length = 0; });
  await expect.poll(async () => {
    await page.evaluate(() => { document.querySelector("#table-body")!.scrollTop = 25 * document.querySelector("#table-rows .row")!.getBoundingClientRect().height; });
    await frames(page);
    return page.locator('#table-rows .row[data-row="30"]').count();
  }).toBe(1);
  await page.locator('#table-rows .row[data-row="30"]').click({ position: { x: 4, y: 4 } });
  await frames(page);
  const before = { selected: await page.evaluate(() => window.upChart.state().selection?.task), top: await scrollTop(page) };
  const next = tasks(40).map((task, i) => (i < 20 ? { ...task, duration: 5 } : task));
  expect(await page.evaluate((rows) => window.upChart.setData({ tasks: rows }), next)).toEqual({ ok: true, updated: 20 });
  await frames(page);
  expect(await minutes(page, keys(0, 40)), "20 new durations, the others as they were").toEqual([...Array(20).fill(2400), ...Array(20).fill(960)]);
  expect((await saved(page))[2]!.name, "the rows still say Task 2, which is no change: the rename stays").toBe("Renamed");
  expect({ selected: await page.evaluate(() => window.upChart.state().selection?.task), top: await scrollTop(page) }, "selection and scroll are as read before the call").toEqual(before);
  expect(await page.evaluate(() => window.edits), "setData is not an edit").toEqual([]);
  expect(await page.evaluate((rows) => window.upChart.setData({ tasks: rows }), next), "the same rows again").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate((rows) => window.upChart.setData({ tasks: rows.slice(1) }), next)).toMatchObject({ ok: false, reason: "rowsChanged" });
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  expect((await saved(page))[2]!.name, "the undo list is the person's: the one undo is the rename").toBe("Task 2");
  expect(await minutes(page, keys(0, 20)), "and it leaves the 20 durations").toEqual(Array(20).fill(2400));
});

test("hostEdit then a typed rename: two Undo keystrokes take the rename back, then the durations, and the host commit is not a save", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  expect(await page.evaluate(() => window.upChart.hostEdit([{ key: "k0", duration: 5 }, { key: "k1", duration: 5 }]))).toEqual({ ok: true, updated: 2 });
  await page.locator(`#table-rows .row[data-row="2"] .name`).dblclick();
  await page.locator("#table-rows input.cell-edit").fill("Renamed");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.edits.length > 1);
  expect(await page.evaluate(() => (window.edits as { commits: { source: string }[] }[]).flatMap((e) => e.commits.map((c) => c.source))), "a host commit, then a typed one").toEqual(["host", "typed"]);
  await page.locator("#app").focus();
  await page.keyboard.press("Control+z");
  await frames(page);
  expect([(await saved(page))[2]!.name, ...(await minutes(page, ["k0", "k1"]))], "the first Undo takes back the rename only").toEqual(["Task 2", 2400, 2400]);
  await page.keyboard.press("Control+z");
  await frames(page);
  expect(await minutes(page, ["k0", "k1"]), "the second takes back the host durations").toEqual([960, 960]);
});

test("pushHostEntry then an Undo keystroke fires an edit whose commit carries the token, and clearHistory empties both lists", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  expect(await page.evaluate(() => [window.upChart.pushHostEntry("assign-7"), window.upChart.pushHostEntry(""), window.upChart.pushHostEntry("assign-8")])).toMatchObject([{ ok: true }, { ok: false, reason: "badToken" }, { ok: true }]);
  await page.locator("#app").focus();
  await page.keyboard.press("Control+z");
  await page.waitForFunction(() => (window.edits as { commits: unknown[] }[]).flatMap((e) => e.commits).length > 2);
  const commits = await page.evaluate(() => (window.edits as { commits: { source: string; host?: string }[] }[]).flatMap((e) => e.commits));
  expect(commits.map((c) => [c.source, c.host]), "both pushes, then the Undo that reached the second").toEqual([["host", "assign-7"], ["host", "assign-8"], ["undo", "assign-8"]]);
  await page.evaluate(() => window.upChart.clearHistory());
  expect(await page.evaluate(() => [window.upChart.commands.undo(), window.upChart.commands.redo()]), "the undo entry and the redo entry went").toEqual([false, false]);
});

test("hostEdit refuses a record that sets progress or changes a task with actual dates, names the task and says to call update, which sets it", async ({ page }) => {
  await mountHost(page, { tasks: tasks(3), durationUnit: "minutes", capabilities: { durationStep: "days" } });
  const progress = { key: "k0", progress: 50 }, start = { key: "k0", snet: "2026-03-09" };
  expect((await page.evaluate((r) => window.upChart.hostEdit([r]), progress) as { reason: string }).reason).toBe("hasActuals");
  expect(await page.evaluate((r) => window.upChart.update([r]), progress)).toEqual({ ok: true, updated: 1 });
  expect((await page.evaluate((r) => window.upChart.hostEdit([r]), start) as { message: string }).message).toBe("Nothing changed: task k0 has actual dates, or the record sets them, so hostEdit cannot take it; call update, which sets it without an undo step.");
  expect(await page.evaluate((r) => window.upChart.update([r]), start)).toEqual({ ok: true, updated: 1 });
  expect((await page.evaluate(() => { window.upChart.update([{ key: "k2", duration: 90 }]); return window.upChart.hostEdit([{ key: "k2", duration: 960 }]); }) as { message: string }).message).toBe("Nothing changed: task k2 cannot be put back by an Undo under the host's capability profile (durations in whole days), so hostEdit cannot take it; call update, which sets it without an undo step.");
});

test("a React host hands 20 tasks to the component as records while one is selected and scrolled to: the chart starts once, selection, scroll and undo hold, and the changed cells are marked", async ({ page }) => {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: REACT_HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
  await page.evaluate((rows) => window.render({ tasks: rows }), tasks(40));
  await page.waitForFunction(() => window.starts === 1, undefined, { timeout: 120_000 });
  await page.locator('#table-rows .row[data-row="2"] .name').dblclick();
  await page.locator("#table-rows input.cell-edit").fill("Renamed");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.edits.length > 0);
  await expect.poll(async () => {
    await page.evaluate(() => { document.querySelector("#table-body")!.scrollTop = 25 * document.querySelector("#table-rows .row")!.getBoundingClientRect().height; });
    await frames(page);
    return page.locator('#table-rows .row[data-row="30"]').count();
  }).toBe(1);
  await page.locator('#table-rows .row[data-row="30"]').click({ position: { x: 4, y: 4 } });
  await frames(page);
  const before = { selected: await page.evaluate(() => window.upChart.state().selection?.task), top: await scrollTop(page) };
  expect([before.selected !== undefined, before.top > 0], "a task is selected and the list is scrolled").toEqual([true, true]);
  await page.evaluate((ks) => window.render({ records: ks.map((key) => ({ key, duration: 5 })) }), keys(20, 40));
  await page.waitForFunction(() => window.answers.length > 0);
  expect(await page.evaluate(() => ({ answers: window.answers, starts: window.starts, canvases: document.querySelectorAll("canvas").length })), "onUpdate got the answer, and there was no second start").toEqual({ answers: [{ ok: true, updated: 20 }], starts: 1, canvases: 1 });
  await expect(page.locator("#table-rows [data-changed]").first(), "a changed cell is marked").toBeAttached();
  expect(await minutes(page, keys(20, 40)), "the 20 new durations").toEqual(Array(20).fill(2400));
  expect(await minutes(page, keys(0, 20)), "the others keep theirs").toEqual(Array(20).fill(960));
  expect({ selected: await page.evaluate(() => window.upChart.state().selection?.task), top: await scrollTop(page) }, "selection and scroll are as they were").toEqual(before);
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  expect((await saved(page))[2]!.name, "undo undoes the rename").toBe("Task 2");
  expect(await minutes(page, keys(20, 40)), "and leaves the 20 durations").toEqual(Array(20).fill(2400));
});
