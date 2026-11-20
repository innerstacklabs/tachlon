import assert from "node:assert/strict";
import type { Page } from "@playwright/test";
import type { FakeServer, Script, Seen } from "./server.ts";

export type Form = "handlers" | "url" | "function";
/** A scenario of decision 0022, rule 11.2. `keys` pins the key mode it runs in; without it the suite runs both. `not` names forms it skips on top of rule 11.1's list. `props` is a setting the scenario names, written as the props it adds to the chart (`retry: false`): the page changes in that and nothing else. */
export interface Scenario { n: number; name: string; keys?: "chart" | "server"; not?: Form[]; props?: string; script: Script; run(page: Page, server: FakeServer, keys: "chart" | "server", form: Form): Promise<void> }
/** What an `error` event carries (rule 3.5); the suite records every one the chart fires in `window.errors`. */
interface SaveError { source: string; message: string; reason?: string; calls?: number; undone?: number }

const until = async (what: string, ok: () => boolean | Promise<boolean>, ms: number): Promise<void> => {
  for (const end = Date.now() + ms; !(await ok()); await new Promise((resolve) => setTimeout(resolve, 50))) if (Date.now() > end) assert.fail(`timed out waiting for ${what}`);
};
const row = (page: Page, n: number) => page.locator(`#table-rows .row[data-row="${n}"]`);
const text = async (page: Page, n: number, cell: "name" | "dur" | "start"): Promise<string> => (await row(page, n).locator(`.${cell}`).allTextContents())[0] ?? "";
const days = async (page: Page, n: number): Promise<number> => Number.parseFloat(await text(page, n, "dur"));
const ready = (page: Page) => row(page, 2).waitFor({ timeout: 30_000 });
const edit = async (page: Page, n: number, cell: "name" | "dur", value: string): Promise<void> => {
  await row(page, n).locator(`.${cell}`).dblclick();
  await page.locator("#table-rows input.cell-edit").fill(value);
  await page.keyboard.press("Enter");
};
const insert = async (page: Page, name: string): Promise<void> => { // a row below the last one, named as it is typed
  await row(page, 2).click({ position: { x: 4, y: 4 } });
  await page.keyboard.press("Insert");
  await page.keyboard.type(name);
  await page.keyboard.press("Enter");
};
const writes = (server: FakeServer) => server.seen.filter((r) => r.method !== "GET");
const rows = (page: Page) => page.locator("#table-rows .row").count();
const alert = async (page: Page): Promise<string> => (await page.locator("#edit-error-text").allTextContents()).join(" ");
const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const reads = (server: FakeServer) => server.seen.filter((r) => r.method === "GET" && r.path === "/api/schedule").length; // the mount's read, and the read after a put-back: the host's, or in the URL form the chart's
const errors = (page: Page): Promise<SaveError[]> => page.evaluate(() => Reflect.get(window, "errors") ?? []);
const press = (page: Page, button: string | RegExp) => page.getByRole("button", { name: button }).click();
const undo = async (page: Page) => { await row(page, 0).click({ position: { x: 4, y: 4 } }); await page.keyboard.press("ControlOrMeta+z"); };
const HOST = new Set(["id", "title", "plannedStart", "days", "parentId"]); // the member names the server's rows use
const task = (i: number) => ({ id: `t${i}`, title: `Task ${i}`, plannedStart: "2026-11-02", days: 2 });
const newKey = (body?: Record<string, unknown>) => String(body?.id ?? "s1"); // the key a create's row gets: the chart's, or the server's first

// A row is inserted while the server holds the answer, and the host's fresh data with that row (or none) lands first.
async function oneRow(page: Page, server: FakeServer): Promise<void> {
  await ready(page);
  await insert(page, "Review");
  await until("the row saved", () => server.db.tasks.size === 4, 8_000);
  await pause(1_500);
  assert.equal(await rows(page), 4, "the row appears once");
  assert.ok(server.db.tasks.has(newKey(writes(server)[0]?.body)), "under the key the chart made or the key the result gives");
}

// The second task has two links, so deleting it saves three requests and undoing that saves three more (the task, then its links). Returns how many writes the delete took.
async function deleteBuild(page: Page, server: FakeServer): Promise<number> {
  await ready(page);
  await row(page, 1).click({ position: { x: 4, y: 4 } });
  await page.keyboard.press("Delete");
  await until("the delete saved", () => !server.db.tasks.has("t2") && writes(server).length === 3, 8_000);
  return 3;
}
// The server drops every write while three edits are made; the chart calls the first at the default waits (1, 3 and 9 s), then stops and asks. Returns the durations before the edits.
const AWAY: Script = { drop: (r) => (r.method === "GET" ? undefined : "request") };
async function away(page: Page, server: FakeServer): Promise<number[]> {
  await ready(page);
  const was = await Promise.all([0, 1, 2].map((n) => days(page, n)));
  for (const [n, value] of [[0, "7"], [1, "8"], [2, "9"]] as const) await edit(page, n, "dur", value);
  await until("the alert line to say it is trying again", async () => (await alert(page)).includes("Not saved yet. Trying again."), 5_000);
  await until("the chart to stop and ask", async () => (await alert(page)).includes("Not saved: "), 25_000); // "Not saved yet." comes from the first failure
  assert.deepEqual(writes(server).map((r) => r.key), ["t1", "t1", "t1", "t1"], "the first is called four times, the others not at all");
  assert.deepEqual(await Promise.all([0, 1, 2].map((n) => days(page, n))), [7, 8, 9], "all three stay on screen");
  assert.deepEqual(await errors(page), [], "nothing is put back");
  return was;
}
// The PATCH is held, and so is the read the host makes after a put-back: the example page starts the chart again with the rows it reads, so what the put-back shows stays on screen long enough to be read.
const HELD = (r: Seen): number => (r.method === "PATCH" ? 800 : r.n > 1 && r.method === "GET" ? 1_500 : 0);
// A create whose response is lost: the server stores it and the connection closes.
const LOST: Script = { drop: (r) => (r.method === "POST" && !r.repeat ? "response" : undefined) };

export const SCENARIOS: Scenario[] = [
  {
    n: 1, name: "1 slow saves", script: { latency: 800 },
    async run(page, server) {
      await ready(page);
      for (const [n, value] of [[0, "7"], [1, "8"], [2, "9"]] as const) await edit(page, n, "dur", value);
      await until("three saves", () => writes(server).length >= 3, 8_000);
      assert.deepEqual(writes(server).map((r) => [r.key, r.body?.days]), [["t1", 7], ["t2", 8], ["t3", 9]], "three calls, in commit order");
      assert.equal(server.maxInFlight, 1, "never two at once");
    },
  },
  {
    n: 2, name: "2 a field refusal",
    script: { latency: HELD, refuse: (r) => (r.method === "PATCH" && Number(r.body?.days) > 100 ? { status: 422, body: { message: "A task cannot take more than 100 days", key: r.key, field: "days" } } : undefined) },
    async run(page, server) {
      await ready(page);
      const was = await days(page, 1);
      await edit(page, 1, "dur", "120");
      await row(page, 0).click({ position: { x: 4, y: 4 } }); // another row is selected while the PATCH is held, so a selected row 1 afterwards is the chart's doing
      await until("the refusal", () => server.seen.length > 1, 8_000);
      assert.deepEqual(writes(server).map((r) => [r.key, r.body]), [["t2", { days: 120 }]], "the server got its own name for the field");
      await until("the cell goes back", async () => (await days(page, 1)) === was, 8_000);
      assert.equal(await row(page, 1).getAttribute("data-selected"), "true", "the row is selected");
      assert.deepEqual(await page.locator("#table-rows .cell-error").allTextContents(), ["A task cannot take more than 100 days"], "the message shows at the duration cell");
    },
  },
  { n: 3, name: "3 a new row's key", script: { latency: 800 }, run: oneRow },
  {
    n: 3, name: "3 a new row's key, host data first", not: ["url"], // the host's data reaches the page from outside, which a `save` string does not do (rule 11.1)
    script: { latency: (r) => (r.method === "POST" ? 800 : 0), push: (r) => (r.method === "POST" ? { after: 100, tasks: [{ ...task(4), id: newKey(r.body), title: "Review" }] } : undefined) },
    run: oneRow,
  },
  {
    n: 4, name: "4 an edit before the create is saved", script: { latency: (r) => (r.method === "POST" ? 1_500 : 0) },
    async run(page, server) {
      await ready(page);
      await insert(page, "Review");
      await until("both saves applied", () => writes(server).length === 2 && [...server.db.tasks.values()].some((t) => t.title === "Review"), 10_000); // the server logs a request when it arrives and applies it after its latency, so a request count alone can be read before the rename lands
      const [create, rename] = writes(server);
      assert.deepEqual([create!.method, rename!.method, rename!.body], ["POST", "PATCH", { title: "Review" }], "the rename is saved second");
      assert.equal(server.db.tasks.get(rename!.key!)?.title, "Review", "and names the row's key");
    },
  },
  {
    n: 5, name: "5 a refused create with edits behind it",
    script: { latency: (r) => (r.method === "POST" ? 1_500 : HELD(r)), refuse: (r) => (r.method === "POST" ? { status: 422, body: { message: "No new tasks today" } } : undefined) },
    async run(page, server) {
      await ready(page);
      await insert(page, "Review");
      await until("the message", async () => (await alert(page)).includes("No new tasks today"), 10_000);
      assert.match(await alert(page), /\b(2|two)\b/, "one message, and it says two changes were undone");
      assert.equal(await rows(page), 3, "the row is gone");
      assert.equal(writes(server).length, 1, "the rename never went to the server");
    },
  },
  {
    n: 6, name: "6 a server recalculation", // function form only: the change-set endpoint needs a function (rule 11.1)
    script: { changed: (_r, db) => (db.tasks.get("t1")?.snet === undefined ? [{ id: "t1", snet: "2026-12-07" }, { id: "t3", snet: "2026-12-21" }] : undefined) },
    async run(page) {
      await ready(page);
      const [was, first, last] = [await days(page, 1), await text(page, 0, "start"), await text(page, 2, "start")];
      await edit(page, 1, "dur", "9");
      await until("the two rows the person did not touch", async () => (await text(page, 0, "start")) !== first && (await text(page, 2, "start")) !== last, 8_000);
      await page.keyboard.press("ControlOrMeta+z");
      await until("the edit taken back", async () => (await days(page, 1)) === was, 8_000);
      assert.notEqual(await text(page, 0, "start"), first, "undo takes back the person's edit only, not the server's changes");
    },
  },
  {
    n: 7, name: "7 an outside change during a save",
    script: {
      latency: (r) => (r.method === "PATCH" ? 2_000 : 0),
      outside: (r, db) => void (r.method === "PATCH" && Object.assign(db.tasks.get("t2")!, { title: "Renamed elsewhere", days: 9 })),
      push: (r) => (r.method === "PATCH" ? { after: 300, tasks: [{ id: "t2", title: "Renamed elsewhere", days: 9 }] } : undefined),
    },
    async run(page) {
      await ready(page);
      await edit(page, 1, "dur", "5");
      await until("the field the person did not touch", async () => (await text(page, 1, "name")) === "Renamed elsewhere", 1_500);
      assert.equal(await days(page, 1), 5, "the field the person touched waits");
      await until("the server's value", async () => (await days(page, 1)) === 9, 6_000);
    },
  },
  {
    n: 8, name: "8 a late result", // 2.5 s stand in for the record's two minutes
    script: {
      outside: (r, db) => { if (r.n === 1) for (const i of [4, 5]) db.tasks.set(`t${i}`, task(i)); },
      latency: (r) => (r.key === "t5" ? 5_000 : 0),
      push: (r) => (r.method === "PATCH" && r.key === "t1" ? { after: 2_500, tasks: [1, 2, 3, 4, 5].map((i) => ({ id: `t${i}`, days: 8 })) } : undefined),
    },
    async run(page) {
      await row(page, 4).waitFor({ timeout: 30_000 });
      await edit(page, 0, "dur", "7");
      await edit(page, 4, "dur", "3");
      await until("four rows updated", async () => (await Promise.all([0, 1, 2, 3].map((n) => days(page, n)))).every((d) => d === 8), 5_000);
      assert.equal(await days(page, 4), 3, "the fifth keeps the edit while its save is in flight");
    },
  },
  {
    n: 9, name: "9 an old read", script: { latency: (r) => (r.method === "GET" && r.n > 1 ? 3_000 : 0), stale: (r) => r.method === "GET" },
    async run(page, server) {
      await ready(page);
      const began = Date.now();
      await page.locator("#refresh").click();
      await edit(page, 0, "dur", "7");
      await row(page, 2).click({ position: { x: 4, y: 4 } });
      await page.keyboard.press("Delete");
      await until("the save and the delete", () => server.db.tasks.get("t1")!.days === 7 && !server.db.tasks.has("t3"), 2_500);
      await pause(began + 3_800 - Date.now());
      assert.equal(await days(page, 0), 7, "the saved value stays");
      assert.equal(await rows(page), 2, "the deleted row does not come back");
    },
  },
  {
    n: 10, name: "10 a lost response", script: LOST,
    async run(page, server) {
      await ready(page);
      await insert(page, "Review");
      await until("the alert line to say it is trying again", async () => (await alert(page)).includes("Not saved yet. Trying again."), 900); // between the two calls
      await until("the create called again, then the rename queued behind it", () => writes(server).length === 3, 10_000);
      await pause(1_500);
      const [first, again] = writes(server);
      assert.deepEqual([again!.once, again!.body], [first!.once, first!.body], "the same call: same key, same body");
      assert.deepEqual(writes(server).map((r) => r.method), ["POST", "POST", "PATCH"], "the create twice, then the rename once, and no more"); // insert() makes two commits (scenario 4)
      assert.equal(server.db.tasks.size, 4, "the server holds the row once");
      assert.equal(await rows(page), 4, "the row appears once");
      assert.deepEqual([await errors(page), await alert(page)], [[], ""], "nothing is put back");
    },
  },
  {
    n: 11, name: "11 a failure part of the way", // the second of the three requests of the undo is refused
    script: {},
    async run(page, server) {
      const sent = await deleteBuild(page, server);
      server.script.refuse = (r) => (r.method !== "GET" && writes(server).length === sent + 2 ? { status: 422, body: { message: "No new links today" } } : undefined);
      await undo(page);
      await until("the put-back and the read after it", async () => reads(server) === 2 && (await errors(page)).length === 1, 8_000);
      assert.deepEqual((await errors(page)).map((e) => [e.reason, e.undone]), [["refused", 1]], "the commit is put back");
      assert.equal(writes(server).length, sent + 2, "the third request is never sent");
      assert.deepEqual([server.db.tasks.size, server.db.links.size], [3, 0], "the server kept the first request");
      await until("the row the server kept", async () => (await rows(page)) === 3, 5_000);
    },
  },
  {
    n: 11, name: "11 a failure part of the way, a dropped request", script: {},
    async run(page, server, _keys, form) {
      const sent = await deleteBuild(page, server);
      server.script.drop = (r) => (r.method !== "GET" && writes(server).length === sent + 2 ? "request" : undefined);
      await undo(page);
      await until("the undo saved whole", () => server.db.links.size === 2, 10_000);
      await until("the alert line to clear", async () => (await alert(page)) === "", 5_000);
      const creates = writes(server).slice(sent).filter((r) => r.path === "/api/tasks").length;
      assert.equal(creates, form === "function" ? 2 : 1, "the handlers and the URL form resume at the second request; the function form sends the first again");
      assert.equal([...server.db.tasks.values()].filter((t) => t.title === "Build").length, 1, "and the server applied the first once");
      assert.deepEqual([await errors(page), await alert(page), reads(server)], [[], "", 1], "nothing is put back or read again");
    },
  },
  {
    n: 12, name: "12 undo across the server", script: { outside: (r, db) => void (r.n === 1 && (db.tasks.get("t2")!.note = "Call the architect")) }, // `note` is a column the chart does not know
    async run(page, server) {
      const sent = await deleteBuild(page, server);
      server.script.refuse = (r) => (r.method === "POST" ? { status: 422, body: { message: "No new tasks today" } } : undefined);
      await undo(page);
      await until("the refusal", async () => (await alert(page)).includes("No new tasks today"), 8_000);
      assert.equal(await rows(page), 2, "the task is gone again");
      assert.equal(writes(server).length, sent + 1, "the undo stopped at its first request");
      server.script.refuse = undefined;
      await undo(page); // the step is back on the undo list
      await until("the task and its links", () => server.db.links.size === 2 && [...server.db.tasks.values()].some((t) => t.title === "Build"), 8_000);
      assert.deepEqual(writes(server).slice(sent + 1).map((r) => `${r.method} ${r.path}`), ["POST /api/tasks", "POST /api/links", "POST /api/links"], "the task, then the links");
      assert.equal([...server.db.tasks.values()].find((t) => t.title === "Build")!.note, "Call the architect", "the column the chart does not know is intact");
      assert.equal(await rows(page), 3);
    },
  },
  { n: 13, name: "13 the server is away", script: AWAY,
    async run(page, server) {
      await away(page, server);
      server.script.drop = undefined; // the server comes back
      await press(page, "Try again");
      await until("the three saved", () => [7, 8, 9].every((d, i) => server.db.tasks.get(`t${i + 1}`)!.days === d), 8_000);
      assert.deepEqual(writes(server).slice(4).map((r) => r.key), ["t1", "t2", "t3"], "once each, in commit order");
      assert.deepEqual(await errors(page), []);
    },
  },
  { n: 14, name: "14 discard", script: AWAY,
    async run(page, server) {
      const was = await away(page, server);
      await press(page, "Discard 3 changes");
      await until("the read after the put-back", () => reads(server) === 2, 8_000); // the host's, or in the URL form the chart's
      assert.deepEqual((await errors(page)).map((e) => [e.reason, e.calls, e.undone]), [["failed", 4, 3]], "error fires once");
      await until("the three put back", async () => (await Promise.all([0, 1, 2].map((n) => days(page, n)))).every((d, i) => d === was[i]), 8_000);
    },
  },
  {
    n: 15, name: "15 repeats off", props: "retry: false", // the latency keeps the failure from landing while insert() is still typing: the rename queues behind the create, and one put-back takes both (scenario 5)
    script: { latency: (r) => (r.method === "POST" ? 1_500 : 0), drop: (r) => (r.method === "POST" ? "response" : undefined) },
    async run(page, server) {
      await ready(page);
      await insert(page, "Review");
      await until("the put-back and the read after it", () => reads(server) === 2, 8_000); // the host's, or in the URL form the chart's
      await pause(1_500); // longer than the first of the default waits
      assert.equal(writes(server).length, 1, "save ran once");
      assert.deepEqual((await errors(page)).map((e) => e.reason), ["failed"]);
      assert.equal(await rows(page), 4, "the row the server kept, once");
    },
  },
  {
    n: 16, name: "16 the host's own names", script: { outside: (r, db) => void (r.n === 1 && (db.tasks.get("t1")!.name = "Own name")) },
    async run(page, server, keys) {
      await ready(page);
      await edit(page, 0, "name", "Renamed");
      await insert(page, "Review");
      await until("three saves", () => writes(server).length === 3, 8_000);
      const bodies = writes(server).map((r) => r.body ?? {});
      for (const member of bodies.flatMap((b) => Object.keys(b))) assert.ok(HOST.has(member), `${member} is not the host's name`);
      if (keys === "chart") assert.equal(typeof bodies[1]!.id, "string", "a create carries the new key as id");
      assert.deepEqual([server.db.tasks.get("t1")!.title, server.db.tasks.get("t1")!.name], ["Renamed", "Own name"], "a row with title and a name of its own keeps both");
      assert.equal(await text(page, 0, "name"), "Renamed");
    },
  },
  // Scenario 17 asserts what rule 3.12 warns of, not a fix: a server that ignores the key breaks the repeat of scenario 10's create.
  {
    n: 17, name: "17 a server that ignores the key, keys chart", keys: "chart", script: { ...LOST, honor: false },
    async run(page, server) {
      await ready(page);
      await insert(page, "Review");
      await until("the put-back and the read after it", async () => reads(server) === 2 && (await errors(page)).length === 1, 10_000);
      assert.match((await errors(page))[0]!.message, /A row with that key exists/, "the repeated create is refused");
      assert.equal(server.db.tasks.size, 4);
      await until("one row", async () => (await rows(page)) === 4, 5_000);
    },
  },
  {
    n: 17, name: "17 a server that ignores the key, keys server", keys: "server", not: ["url"], script: { ...LOST, honor: false }, // with `idempotent`, as the copy of rule 11.1 has it; the URL form never sees host data arrive from outside
    async run(page, server) {
      await ready(page);
      await insert(page, "Review");
      await until("two rows stored", () => server.db.tasks.size === 5, 10_000);
      await page.locator("#refresh").click();
      await until("two rows shown", async () => (await rows(page)) === 5, 5_000);
      assert.deepEqual(await errors(page), [], "nothing is put back");
    },
  },
  {
    n: 21, name: "21 a 500 that applied nothing", script: { fail: (r) => (r.method === "PATCH" && !r.repeat ? 500 : undefined) },
    async run(page, server) {
      await ready(page);
      await edit(page, 1, "dur", "7");
      await until("the repeat applied", () => server.db.tasks.get("t2")!.days === 7, 10_000);
      await pause(1_500);
      const [first, again] = writes(server);
      assert.deepEqual([writes(server).length, again!.once], [2, first!.once], "one repeat, with the same key");
      assert.equal(await days(page, 1), 7, "saved");
      assert.deepEqual([await errors(page), await alert(page), reads(server)], [[], "", 1], "nothing put back, no error, src not read");
    },
  },
  {
    n: 22, name: "22 an expired login", script: { fail: (r) => (r.method === "PATCH" ? 401 : undefined) },
    async run(page, server) {
      await ready(page);
      await edit(page, 1, "dur", "7");
      await until("the alert line", async () => /sign in/i.test(await alert(page)), 5_000);
      await pause(1_500); // longer than the first of the default waits
      assert.equal(writes(server).length, 1, "no call follows");
      assert.deepEqual([await page.getByRole("button", { name: "Try again" }).count(), await page.getByRole("button", { name: /^Discard 1 changes?$/ }).count()], [1, 1], "both buttons");
      assert.deepEqual(await errors(page), [], "no error fires");
      server.script.fail = undefined; // the script accepts the session again
      await press(page, "Try again");
      await until("the commit saved", () => server.db.tasks.get("t2")!.days === 7, 8_000);
      assert.equal(writes(server).length, 2, "saved by one more call");
      server.script.refuse = (r) => (r.method === "PATCH" ? { status: 403, body: { message: "You may not change this schedule" } } : undefined);
      await edit(page, 1, "dur", "8");
      await until("the message", async () => (await alert(page)).includes("You may not change this schedule"), 8_000);
      assert.equal(writes(server).length, 3, "a 403 is put back at once");
      assert.deepEqual([await days(page, 1), (await errors(page)).map((e) => e.reason)], [7, ["refused"]]);
    },
  },
  {
    n: 23, name: "23 a refusal that carries a status", // the body is not JSON; the handlers form's client throws an error with `status: 422` and its status line as the message
    script: { latency: HELD, refuse: (r) => (r.method === "PATCH" ? { status: 422, body: "That change is not allowed" } : undefined) },
    async run(page, server, _keys, form) {
      await ready(page);
      const was = await days(page, 1);
      await edit(page, 1, "dur", "7");
      await row(page, 0).click({ position: { x: 4, y: 4 } }); // another row is selected while the save is in flight, so a selected row 1 afterwards is the chart's doing
      await until("the put-back", async () => (await errors(page)).length === 1 && (await days(page, 1)) === was, 8_000);
      const shown = await alert(page);
      if (form === "handlers") assert.ok(shown.includes("Unprocessable Entity"), "the error's message shows");
      else assert.ok(shown.includes("The server refused the change") && shown.includes("422"), "the chart's own message, with the status");
      if (form !== "url") assert.deepEqual(await Promise.all([0, 1].map((n) => row(page, n).getAttribute("data-selected"))), form === "handlers" ? [null, "true"] : ["true", null], form === "handlers" ? "the refused change's row is selected, though the error names no key" : "the selection stays: the chart cannot tell which request was refused"); // the record names no row for the URL form
      await pause(1_500);
      assert.equal(writes(server).length, 1, "no repeat");
      assert.deepEqual((await errors(page)).map((e) => e.reason), ["refused"]);
    },
  },
  {
    n: 24, name: "24 a 503 part of the way", script: {}, // deleting the second task is three requests (its two links, then the task); the second is answered 503
    async run(page, server) {
      await ready(page);
      server.script.fail = (r) => (r.method !== "GET" && writes(server).length === 2 ? 503 : undefined); // only the second request: the repeat is the third
      await row(page, 1).click({ position: { x: 4, y: 4 } });
      await page.keyboard.press("Delete");
      await until("the commit saved", () => server.db.links.size === 0 && !server.db.tasks.has("t2"), 10_000);
      await pause(1_500);
      const [first, second, again, last] = writes(server);
      assert.equal(writes(server).length, 4, "the first request once, the second twice, the third once");
      assert.deepEqual([again!.method, again!.path, again!.once], [second!.method, second!.path, second!.once], "the repeat resumes at the second change, with its key");
      assert.ok(first!.path !== second!.path && last!.path === "/api/tasks/t2", "the links, then the task");
      assert.deepEqual([await errors(page), await alert(page)], [[], ""], "nothing is put back");
    },
  },
  {
    n: 25, name: "25 a create the server cannot repeat", keys: "server", props: "idempotent: false", // the page's `idempotent` is the one this scenario leaves off
    script: { honor: false, latency: (r) => (r.method === "POST" ? 1_500 : 0) }, // the latency keeps the failure from landing while insert() is still typing (scenario 5)
    async run(page, server) {
      await ready(page);
      server.script.drop = (r) => (r.method === "POST" ? "response" : undefined);
      await insert(page, "Review");
      await until("the put-back and the read after it", async () => reads(server) === 2 && (await errors(page)).length === 1, 8_000);
      await pause(1_500);
      assert.deepEqual([writes(server).length, (await errors(page)).map((e) => e.reason)], [1, ["failed"]], "put back at once, no repeat");
      assert.deepEqual([server.db.tasks.size, await rows(page)], [4, 4], "the row the server made, once");
      server.script.drop = (r) => (r.method === "PATCH" && !r.repeat ? "response" : undefined);
      await edit(page, 1, "dur", "7");
      await until("the update repeated", () => writes(server).length === 3, 10_000);
      await until("the alert line to clear", async () => (await alert(page)) === "", 5_000);
      const [, first, again] = writes(server);
      assert.deepEqual([again!.method, again!.path, again!.once], [first!.method, first!.path, first!.once], "the same call");
      assert.deepEqual([server.db.tasks.get("t2")!.days, await days(page, 1)], [7, 7], "saved");
      server.script.drop = (r) => (r.method === "DELETE" && !r.repeat ? "response" : undefined);
      await row(page, 3).click({ position: { x: 4, y: 4 } });
      await page.keyboard.press("Delete");
      await until("the delete repeated", () => writes(server).length === 5, 10_000); // the repeat is answered 404 and counts as saved
      await pause(1_500);
      assert.deepEqual(writes(server).slice(3).map((r) => `${r.method} ${r.path}`), ["DELETE /api/tasks/s1", "DELETE /api/tasks/s1"], "once more, and no more");
      assert.deepEqual([await rows(page), (await errors(page)).length, await alert(page)], [3, 1, ""], "saved: nothing else is put back");
    },
  },
  {
    n: 26, name: "26 a missing handler", props: "save: (({ createLink, ...rest }) => rest)(save)", // the page's handlers without `createLink`
    script: {},
    async run(page, server) {
      const warned: string[] = [];
      page.on("console", (m) => void (m.type() === "warning" && warned.push(m.text())));
      page.on("framenavigated", () => void (warned.length = 0)); // only the mount after the reload counts
      await page.reload(); // the warning is logged at mount, so mount again with the listener on
      await ready(page);
      assert.equal(warned.filter((w) => w.includes("createLink")).length, 1, "one warning, and it names the handler");
      const hook = (read: string, r?: number) => page.evaluate(`(() => { const g = window.gantt, r = ${r}; return ${read}; })()`);
      const [from, to] = await Promise.all([0, 2].map((n) => hook("g.app.x_of(g.app.row_start(r))", n) as Promise<number>));
      const [a, b, box] = [await row(page, 0).boundingBox(), await row(page, 2).boundingBox(), await page.locator("#timeline").boundingBox()];
      const down = { x: box!.x + from + 4, y: a!.y + a!.height / 2 };
      await page.mouse.move(down.x, down.y);
      await until("the pointer over the first bar", async () => (await hook("g.status().hover")) !== null, 3_000); // the press below is on a bar
      await page.keyboard.down("Alt");
      try {
        await page.mouse.down();
        await page.mouse.move(box!.x + to + 4, b!.y + b!.height / 2, { steps: 8 });
        assert.equal(await hook("g.app.linking()"), false, "no link gesture starts");
        await page.mouse.move(down.x, down.y, { steps: 8 }); // back where it began, so a press that became a bar drag moves nothing
      } finally {
        await page.mouse.up();
        await page.keyboard.up("Alt");
      }
      await pause(1_500);
      const base = writes(server).length;
      await row(page, 1).click({ position: { x: 4, y: 4 } });
      await page.keyboard.press("Delete");
      await until("the delete saved", () => !server.db.tasks.has("t2") && writes(server).length === base + 3, 8_000);
      await undo(page);
      await pause(1_500);
      assert.deepEqual([await rows(page), writes(server).length - base, server.db.tasks.size, server.db.links.size], [2, 3, 2, 0], "nothing changes and save is not called");
      assert.ok(!server.seen.some((r) => r.method === "POST" && r.path === "/api/links"), "no link was made");
    },
  },
];
