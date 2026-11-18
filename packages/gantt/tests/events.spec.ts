import { expect, test, type Gantt } from "./support/gantt";
import { fileURLToPath } from "node:url";
import { fromTasks } from "../src/adapters/plain";
import { AXIS, ROW } from "./support/pixels";

// gs#1086: a host reacts to the chart with `on(name, handler)`. Events come from the loop's `Status` read, so
// each fires once per change, never per frame. The hook (`window.gantt.on`) is the entry's own `on`.

interface Seen {
  name: string;
  detail: unknown;
}

/** Record every event of every name on `window.__seen`, in order. */
async function record(g: Gantt): Promise<void> {
  await g.page.evaluate(() => {
    const w = window as unknown as { __seen: Seen[] };
    w.__seen = [];
    for (const name of ["select", "edit", "move", "open", "load", "error"] as const) {
      window.gantt!.on(name, (detail: unknown) => w.__seen.push({ name, detail }));
    }
  });
}

/** Two tasks with a host key each, loaded as plain data. The events are recorded after the frames that see the load, since an edit in the frame that sees a load fires no `edit`. */
async function loadKeyed(g: Gantt): Promise<void> {
  await g.open({ n: 20, params: { backend: "webgl" } });
  const tasks = [{ id: "a", key: "k-dig", name: "Dig", start: "2026-03-02", duration: 3 }, { id: "b", key: "k-pour", name: "Pour", start: "2026-03-02", duration: 2 }];
  await g.page.evaluate((document) => window.gantt!.api!.load({ document }), fromTasks({ tasks, links: [] }).document);
  await g.nextFrame(3);
  await record(g);
}

const seen = (g: Gantt, name: string): Promise<Seen[]> =>
  g.page.evaluate((n) => (window as unknown as { __seen: Seen[] }).__seen.filter((e) => e.name === n), name);

/** Run `act` in the page, then wait until the page has heard `n` edit events in all. */
async function editAndWait(g: Gantt, act: () => unknown, n: number): Promise<void> {
  await g.page.evaluate(act);
  await expect.poll(async () => (await g.status(), (await seen(g, "edit")).length)).toBe(n);
}

test.describe("on() events", () => {
  test("a row click fires select once with its id", async ({ g }) => {
    await g.open({ n: 200 });
    await record(g);
    const first = await g.page.evaluate(() => {
      const app = window.gantt!.app;
      return (JSON.parse(app.table_rows(0, 3)) as [number][])[1]![0];
    });
    await g.page.locator('#table-rows .row[data-row="1"]').click({ position: { x: 4, y: 4 } });
    await g.nextFrame(6);
    expect(await seen(g, "select")).toEqual([{ name: "select", detail: { task: first, key: null } }]);
    // Idle frames fire nothing more.
    await g.nextFrame(10);
    expect(await seen(g, "select")).toHaveLength(1);
  });

  test("load fires once, also for a handler added after it", async ({ g }) => {
    await g.open({ n: 200 });
    await record(g);
    await g.nextFrame(4);
    expect(await seen(g, "load")).toHaveLength(1);
  });

  test("an edit fires edit once with the new revision", async ({ g }) => {
    await g.open({ n: 200 });
    await record(g);
    const before = (await g.status()).revision;
    await g.page.locator('#table-rows .row[data-row="1"]').click({ position: { x: 4, y: 4 } });
    await g.page.keyboard.press("Insert");
    await g.nextFrame(6);
    const edits = await seen(g, "edit");
    expect(edits).toHaveLength(1);
    expect((edits[0]!.detail as { revision: number }).revision).toBeGreaterThan(before);
  });

  test("a bar drag fires move once, on release", async ({ g }) => {
    await g.open({ n: 200, params: { backend: "webgl" } });
    // As hover-drag.spec: the day level, then `c` puts the first critical bar a fifth of the way across.
    const box = await g.box();
    for (let k = 0; k < 200 && (await g.status()).level !== "day"; k++) {
      await g.page.mouse.move(box.x + box.width / 2, box.y + AXIS + 10 * ROW);
      await g.page.keyboard.down("Control");
      await g.page.mouse.wheel(0, -120);
      await g.page.keyboard.up("Control");
      await g.nextFrame();
    }
    await g.page.keyboard.press("c");
    await g.nextFrame(3);
    await record(g);
    const x = box.x + box.width * 0.2 + 4;
    const y = box.y + AXIS + ROW / 2;
    await g.page.mouse.move(x, y);
    await g.page.mouse.down();
    await g.page.mouse.move(x + 4, y);
    for (let k = 1; k <= 8; k++) await g.page.mouse.move(x + 15 * k, y);
    await g.nextFrame(3);
    expect(await seen(g, "move"), "nothing while the bar is held").toHaveLength(0);
    const held = await seen(g, "edit");
    expect(held.length, "the held bar moves the revision").toBeGreaterThan(0);
    expect(held.map((e) => (e.detail as { commits: unknown[] }).commits), "no commit before the release").toEqual(held.map(() => []));
    await g.page.mouse.up();
    await g.nextFrame(6);
    const moves = await seen(g, "move");
    expect(moves).toHaveLength(1);
    const { task, key, start } = moves[0]!.detail as { task: number; key: null; start: string };
    const row = (await g.page.evaluate(() => JSON.parse(window.gantt!.app.table_rows(0, 400)) as [number, string][])).find((r) => r[0] === task);
    expect([key, start]).toEqual([null, row![1].replace(" ", "T")]);
    expect(start).toMatch(/^\d{4}-\d\d-\d\dT\d\d:\d\d$/);
    expect((await seen(g, "edit")).flatMap((e) => (e.detail as { commits: { source: string }[] }).commits).map((c) => c.source), "the release makes the drag's one commit").toEqual(["drag"]);
  });

  test("a good file fires open once, with the epoch it made", async ({ g }) => {
    await g.open({ n: 200 });
    await record(g);
    const before = (await g.status()).epoch;
    await g.page.setInputFiles("#open-input", fileURLToPath(new URL("fixtures/links.xml", import.meta.url)));
    await expect.poll(() => seen(g, "open")).toEqual([{ name: "open", detail: { epoch: before + 1 } }]);
    await g.nextFrame(8);
    expect(await seen(g, "open")).toHaveLength(1);
    expect(await seen(g, "error")).toHaveLength(0);
  });

  test("the function on() returns removes the handler", async ({ g }) => {
    await g.open({ n: 200 });
    await g.page.evaluate(() => {
      const w = window as unknown as { __n: number };
      w.__n = 0;
      const off = window.gantt!.on("edit", () => (w.__n += 1));
      off();
    });
    await g.page.locator('#table-rows .row[data-row="1"]').click({ position: { x: 4, y: 4 } });
    await g.page.keyboard.press("Insert");
    await g.nextFrame(6);
    expect(await g.page.evaluate(() => (window as unknown as { __n: number }).__n)).toBe(0);
  });

  test("a throwing handler is logged and the chart keeps running, also a late load handler", async ({ g }) => {
    await g.open({ n: 200 });
    await g.page.evaluate(() => {
      window.gantt!.on("load", () => {
        throw new Error("late load boom");
      });
      window.gantt!.on("select", () => {
        throw new Error("select boom");
      });
    });
    await g.page.locator('#table-rows .row[data-row="1"]').click({ position: { x: 4, y: 4 } });
    const frames = (await g.status()).frames;
    await g.nextFrame(6);
    expect((await g.status()).frames).toBeGreaterThan(frames);
    const logged = g.logs.filter((l) => l.startsWith("error: an event handler threw"));
    expect(logged).toHaveLength(2);
    // The throws are the test's own: take them out of the page-error check the fixture makes.
    g.logs.length = 0;
  });

  test("undo and redo each fire edit", async ({ g }) => {
    await g.open({ n: 200 });
    await record(g);
    await g.page.locator('#table-rows .row[data-row="1"]').click({ position: { x: 4, y: 4 } });
    await g.page.keyboard.press("Insert");
    await g.nextFrame(4);
    await g.page.keyboard.press("Control+z");
    await g.nextFrame(4);
    await g.page.keyboard.press("Control+Shift+z");
    await g.nextFrame(4);
    const revisions = (await seen(g, "edit")).map((e) => (e.detail as { revision: number }).revision);
    expect(revisions).toHaveLength(3);
    // The revision counts changes, so an undo and a redo each move it on; each fires its own event.
    expect(revisions[1]).toBeGreaterThan(revisions[0]!);
    expect(revisions[2]).toBeGreaterThan(revisions[1]!);
  });

  test("a failed frame fires error with source frame and a message", async ({ g }) => {
    await g.open({ n: 200, params: { backend: "webgl" } });
    await record(g);
    await g.page.evaluate(() => window.gantt!.failNextFrame(false));
    await g.nextFrame(4);
    const errors = await seen(g, "error");
    expect(errors).toHaveLength(1);
    const detail = errors[0]!.detail as { source: string; message: string };
    expect(detail.source).toBe("frame");
    expect(detail.message).not.toBe("");
    g.logs.length = 0; // the guard logs the failed frame on purpose
  });

  test("an unreadable file fires error once and no open", async ({ g }) => {
    await g.open({ n: 200 });
    await record(g);
    await g.page.evaluate(() => {
      const input = document.getElementById("open-input") as HTMLInputElement;
      const dt = new DataTransfer();
      dt.items.add(new File(["not a schedule"], "bad.xer"));
      input.files = dt.files;
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect.poll(() => seen(g, "error")).toHaveLength(1);
    const detail = (await seen(g, "error"))[0]!.detail as { source: string; message: string };
    expect(detail.source).toBe("open");
    expect(detail.message).toBe(await g.page.locator("#open-error-text").innerText());
    expect(await seen(g, "open")).toHaveLength(0);
  });
});

test.describe("the edit event carries commits, select and move name the key", () => {
  test("three typed edits give three edit events whose new values match a fresh snapshot", async ({ g }) => {
    await loadKeyed(g);
    await editAndWait(g, () => window.gantt!.app.rename_task(0, "Dig deep"), 1);
    await editAndWait(g, () => window.gantt!.app.set_duration(0, "4d"), 2);
    await editAndWait(g, () => window.gantt!.app.set_start_text(0, "2026-03-04"), 3);
    const edits = (await seen(g, "edit")).map((e) => e.detail as { commits: { source: string; changes: { key: string; set: Record<string, unknown> }[] }[]; seq: number; snapshot: boolean });
    expect(edits.map((e) => [e.commits.length, e.commits[0]!.source, e.seq, e.snapshot])).toEqual([[1, "typed", 1, false], [1, "typed", 2, false], [1, "typed", 3, false]]);
    const [name, duration, snet] = edits.map((e) => e.commits[0]!.changes[0]!);
    const after = (await g.page.evaluate(() => window.gantt!.api!.snapshot().tasks))[0]!;
    expect([name!.key, name!.set]).toEqual(["k-dig", { name: after.name }]);
    expect(duration!.set).toEqual({ duration: after.duration });
    expect(Object.keys(snet!.set)).toEqual(["snet"]);
    expect(String(snet!.set.snet).slice(0, 10)).toBe(after.start!.slice(0, 10));
  });

  test("an undo's event carries one undo commit with old and new swapped", async ({ g }) => {
    await loadKeyed(g);
    await editAndWait(g, () => window.gantt!.app.rename_task(0, "Dig deep"), 1);
    await editAndWait(g, () => window.gantt!.app.undo(), 2);
    const [first, second] = (await seen(g, "edit")).flatMap((e) => (e.detail as { commits: { source: string; changes: { set: unknown; was: unknown }[] }[] }).commits);
    expect([first!.source, first!.changes[0]!.was, first!.changes[0]!.set]).toEqual(["typed", { name: "Dig" }, { name: "Dig deep" }]);
    expect([second!.source, second!.changes[0]!.was, second!.changes[0]!.set]).toEqual(["undo", { name: "Dig deep" }, { name: "Dig" }]);
  });

  test("a delete's event carries one change whose was is the whole record it removed", async ({ g }) => {
    await loadKeyed(g);
    await editAndWait(g, () => (window.gantt!.app.select_task(0), window.gantt!.app.delete_selected()), 1);
    const [edit] = await seen(g, "edit");
    const [change, ...more] = (edit!.detail as { commits: { changes: { op: string; key: string; set?: unknown; was: { name: string } }[] }[] }).commits[0]!.changes;
    expect(more).toEqual([]);
    expect([change!.op, change!.key, change!.was.name, "set" in change!]).toEqual(["delete", "k-dig", "Dig", false]);
  });

  test("a keyed task fires select with its key", async ({ g }) => {
    await loadKeyed(g);
    await g.page.evaluate(() => window.gantt!.api!.commands.select(1));
    await g.status();
    await expect.poll(async () => (await g.status(), (await seen(g, "select")).map((e) => e.detail))).toEqual([{ task: 1, key: "k-pour" }]);
  });

  test("1,001 renames between two frames give one edit with snapshot true, and the next edit drains as usual", async ({ g }) => {
    await loadKeyed(g);
    await editAndWait(g, () => { for (let i = 0; i < 1001; i++) window.gantt!.app.rename_task(0, `Dig ${i}`); }, 1);
    expect((await seen(g, "edit")).map((e) => e.detail)).toEqual([{ revision: expect.any(Number), commits: [], snapshot: true, seq: 1001 }]);
    await editAndWait(g, () => window.gantt!.app.rename_task(0, "Last"), 2);
    const next = (await seen(g, "edit"))[1]!.detail as { commits: { seq: number }[]; snapshot: boolean };
    expect([next.commits.map((c) => c.seq), next.snapshot]).toEqual([[1002], false]);
  });
});
