import { expect, test, type Gantt as Page } from "./support/gantt";
import type { Commands } from "../src/commands";
import { fromTasks } from "../src/adapters/plain";

// Host commands (gs#1085): a host page drives the chart from its own buttons. `mount` returns them on the `Gantt`; the
// developer page drops that value, so an init script catches the `gantt:ready` event `mount` sends up from the root.

declare global {
  interface Window {
    hostCommands?: Commands;
  }
}

async function open(g: Page): Promise<void> {
  await g.page.addInitScript(() => {
    document.addEventListener("gantt:ready", (e) => {
      window.hostCommands = (e as CustomEvent<Commands>).detail;
    });
  });
  await g.open({ n: 2000 });
  await g.page.waitForFunction(() => window.hostCommands !== undefined);
}

test("scrollTo({ task }) brings a row far below the first screen into view", async ({ g }) => {
  await open(g);
  expect((await g.status()).scrollY).toBe(0);
  const far = await g.page.evaluate(() => (JSON.parse(window.gantt!.app.table_rows(1500, 1501)) as [number][])[0]![0]);
  expect(await g.page.evaluate((t) => window.hostCommands!.scrollTo({ task: t }), far)).toBe(true);
  await g.nextFrame(3);
  const s = await g.status();
  expect(s.rows[0], "the first drawn row is at or above the target").toBeLessThanOrEqual(1500);
  expect(s.rows[1], "the last drawn row is at or below the target").toBeGreaterThanOrEqual(1500);
  expect(s.selected?.task, "scrollTo selects the task it brings in").toBe(far);
  const start = await g.page.evaluate((row) => window.gantt!.app.row_start(row), s.selected!.row);
  expect(start, "the selected row has a bar start").not.toBeNull();
  expect(s.t[0], "the bar starts at or after the left edge").toBeLessThanOrEqual(start!);
  expect(s.t[1], "the bar starts before the right edge").toBeGreaterThanOrEqual(start!);
});

test("scrollTo({ date }) with a day before the project starts pans the visible range to include it", async ({ g }) => {
  await open(g);
  const before = await g.status();
  const day = new Date(Date.parse(before.from) - 120 * 86_400_000).toISOString().slice(0, 10);
  expect(Date.parse(day), "the day is before the visible range, which the fit sets to the project").toBeLessThan(Date.parse(before.from));
  expect(await g.page.evaluate((d) => window.hostCommands!.scrollTo({ date: d }), day)).toBe(true);
  await g.nextFrame(3);
  const after = await g.status();
  expect(Date.parse(after.from), "the visible range now starts at or before that day").toBeLessThanOrEqual(Date.parse(day));
  expect(Date.parse(after.to), "and ends after it").toBeGreaterThan(Date.parse(day));
});

test("scrollTo({ date }) moves the time axis to that day", async ({ g }) => {
  await open(g);
  const before = await g.status();
  const day = new Date(Date.parse(before.to) + 20 * 86_400_000).toISOString().slice(0, 10);
  expect(await g.page.evaluate((d) => window.hostCommands!.scrollTo({ date: d }), day)).toBe(true);
  await g.nextFrame(3);
  const after = await g.status();
  expect(after.t[0]).not.toBe(before.t[0]);
});

test("scrollTo({ date }) with text that is not a date returns false and moves nothing", async ({ g }) => {
  await open(g);
  const before = await g.status();
  expect(await g.page.evaluate(() => window.hostCommands!.scrollTo({ date: "not a date" }))).toBe(false);
  await g.nextFrame(3);
  const after = await g.status();
  expect(after.t).toEqual(before.t);
  expect(after.scrollY).toBe(before.scrollY);
});

test("undo and redo with nothing to step return false and change nothing", async ({ g }) => {
  await open(g);
  const before = await g.status();
  expect(await g.page.evaluate(() => window.hostCommands!.undo())).toBe(false);
  expect(await g.page.evaluate(() => window.hostCommands!.redo())).toBe(false);
  await g.nextFrame(3);
  const after = await g.status();
  expect(after.revision).toBe(before.revision);
  expect(after.stamp).toBe(before.stamp);
});

test("zoomIn and zoomOut change the scale, fit shows the whole project again", async ({ g }) => {
  await open(g);
  const base = (await g.status()).pxPerMinute;
  await g.page.evaluate(() => window.hostCommands!.zoomIn());
  await g.nextFrame(3);
  const inn = (await g.status()).pxPerMinute;
  expect(inn).toBeGreaterThan(base);
  await g.page.evaluate(() => {
    window.hostCommands!.zoomOut();
    window.hostCommands!.zoomOut();
  });
  await g.nextFrame(3);
  const out = (await g.status()).pxPerMinute;
  expect(out).toBeLessThan(base);
  // Fit flies to its target over 300 ms of the session clock (`crates/app/src/fly.rs`), so three frames can land mid-flight on a
  // fast runner: wait until the view stops moving before reading it.
  const settled = async (): Promise<Awaited<ReturnType<typeof g.status>>> => {
    let before = await g.status();
    for (let k = 0; k < 200; k++) {
      await g.nextFrame(2);
      const now = await g.status();
      if (now.t[0] === before.t[0] && now.t[1] === before.t[1] && now.pxPerMinute === before.pxPerMinute) return now;
      before = now;
    }
    throw new Error("the view never settled");
  };
  expect(await g.page.evaluate(() => window.hostCommands!.fit())).toBe(true);
  const fitted = await settled();
  await g.page.click("#fit-button");
  const button = await settled();
  expect(fitted.pxPerMinute).not.toBe(out);
  expect(fitted.t, "fit shows what the Fit button shows").toEqual(button.t);
});

test("select and scrollTo take a host's key as text, and refuse a key no live task holds", async ({ g }) => {
  await open(g);
  const tasks = Array.from({ length: 30 }, (_, i) => ({ id: `t${i}`, key: `k${i}`, name: `Task ${i}`, start: "2026-03-02", duration: 2 }));
  await g.page.setViewportSize({ width: 1280, height: 320 });
  await g.page.evaluate((d) => window.gantt!.api!.load({ document: d }), fromTasks({ tasks, links: [] }).document);
  await g.nextFrame(3);
  const slot = (key: string) => g.page.evaluate((k) => (JSON.parse(window.gantt!.app.task_slots([k])) as (number | null)[])[0], key);
  expect(await g.page.evaluate(() => window.hostCommands!.scrollTo({ task: "k25" }))).toBe(true);
  await g.nextFrame(3);
  const s = await g.status();
  expect(s.selected?.task, "scrollTo selects the task the key names").toBe(await slot("k25"));
  expect(s.rows[1], "its row is in view").toBeGreaterThanOrEqual(s.selected!.row);
  expect(await g.page.evaluate(() => window.hostCommands!.select("k3"))).toBe(true);
  expect((await g.status()).selected?.task, "select takes the key's slot").toBe(await slot("k3"));
  expect(await g.page.evaluate(() => window.hostCommands!.select("nope"))).toBe(false);
  expect((await g.status()).selected?.task, "the selection is as it was").toBe(await slot("k3"));
});

test("select picks a task, and refuses one with no row", async ({ g }) => {
  await open(g);
  const task = await g.page.evaluate(() => (JSON.parse(window.gantt!.app.table_rows(3, 4)) as [number][])[0]![0]);
  expect(await g.page.evaluate((t) => window.hostCommands!.select(t), task)).toBe(true);
  await g.nextFrame(2);
  expect((await g.status()).selected?.task).toBe(task);
  const before = await g.status();
  expect(await g.page.evaluate(() => window.hostCommands!.select(4_000_000))).toBe(false);
  await g.nextFrame(3);
  const after = await g.status();
  expect(after.selected, "the selection is as it was").toEqual(before.selected);
  expect(after.scrollY).toBe(before.scrollY);
  expect(after.t).toEqual(before.t);
});

test("setFilter turns the critical-only filter on and off", async ({ g }) => {
  await open(g);
  const all = (await g.status()).rowCount;
  await g.page.evaluate(() => window.hostCommands!.setFilter("critical"));
  await g.nextFrame(2);
  const crit = await g.status();
  expect(crit.criticalOnly).toBe(true);
  expect(crit.rowCount).toBeLessThan(all);
  await g.page.evaluate(() => window.hostCommands!.setFilter("off"));
  await g.nextFrame(2);
  const back = await g.status();
  expect(back.criticalOnly).toBe(false);
  expect(back.rowCount).toBe(all);
});

test("undo and redo step through an edit", async ({ g }) => {
  await open(g);
  const task = await g.page.evaluate(() => (JSON.parse(window.gantt!.app.table_rows(2, 3)) as [number][])[0]![0]);
  const minutes = (): Promise<number> => g.page.evaluate((t) => window.gantt!.app.test_task_minutes(t), task);
  const was = await minutes();
  await g.page.evaluate((t) => window.gantt!.app.set_duration(t, "40d"), task);
  const edited = await minutes();
  expect(edited).not.toBe(was);
  expect(await g.page.evaluate(() => window.hostCommands!.undo())).toBe(true);
  expect(await minutes()).toBe(was);
  expect(await g.page.evaluate(() => window.hostCommands!.redo())).toBe(true);
  expect(await minutes()).toBe(edited);
  expect(await g.page.evaluate(() => window.hostCommands!.redo()), "nothing is left to redo").toBe(false);
});

test("assignKey({ task }, key) is no edit, is kept in the saved document and refuses a taken key by name", async ({ g }) => {
  await open(g);
  const first = await g.page.evaluate(() => window.hostCommands!.insertRow());
  const second = await g.page.evaluate(() => window.hostCommands!.insertRow());
  if (!first?.ok || !second?.ok) throw new Error("insertRow refused");
  const revision = (await g.status()).revision;
  const assign = (record: unknown, key: unknown) => g.page.evaluate(([r, k]) => window.hostCommands!.assignKey(r as { task: number }, k as string), [record, key] as const);
  expect(await assign({ task: first.task }, "k-1")).toEqual({ ok: true });
  expect((await g.status()).revision, "not an edit").toBe(revision);
  expect(await g.page.evaluate(() => window.gantt!.app.save("keys"))).toContain('"key":"k-1"');
  expect(await assign({ task: second.task }, "k-1")).toMatchObject({ ok: false, reason: "keyTaken" });
  expect(await assign({ link: 0 }, "l-1")).toEqual({ ok: true });
  expect(await assign({ task: first.task }, 5)).toMatchObject({ ok: false, reason: "emptyKey" });
  expect(await assign({ task: -1 }, "k-2")).toMatchObject({ ok: false, reason: "unknownTask" });
});

test("after destroy() every host command returns undefined, does nothing and never throws", async ({ g }) => {
  await open(g);
  const { thrown, defined } = await g.page.evaluate(() => {
    const c = window.hostCommands!;
    window.gantt!.api!.destroy();
    const calls: Record<string, () => unknown> = {
      zoomIn: () => c.zoomIn(),
      zoomOut: () => c.zoomOut(),
      fit: () => c.fit(),
      "scrollTo({ date })": () => c.scrollTo({ date: "2026-01-05" }),
      "scrollTo({ task })": () => c.scrollTo({ task: 1 }),
      select: () => c.select(1),
      'setFilter("critical")': () => c.setFilter("critical"),
      'setFilter("off")': () => c.setFilter("off"),
      undo: () => c.undo(),
      redo: () => c.redo(),
      assignKey: () => c.assignKey({ task: 1 }, "k"),
    };
    const thrown: string[] = [];
    const defined: string[] = [];
    for (const [name, call] of Object.entries(calls)) {
      try {
        if (call() !== undefined) defined.push(name);
      } catch (err) {
        thrown.push(`${name}: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
    return { thrown, defined };
  });
  expect(thrown, "no command throws after destroy()").toEqual([]);
  expect(defined, "every command returns undefined after destroy()").toEqual([]);
});
