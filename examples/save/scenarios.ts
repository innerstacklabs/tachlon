import assert from "node:assert/strict";
import type { Page } from "@playwright/test";
import type { FakeServer, Script } from "./server.ts";

export type Form = "handlers" | "url" | "function";
/** A scenario of decision 0022, rule 11.2. `keys` pins the key mode it runs in; without it the suite runs both. `not` names forms it skips on top of rule 11.1's list. */
export interface Scenario { n: number; name: string; keys?: "chart" | "server"; not?: Form[]; script: Script; run(page: Page, server: FakeServer, keys: "chart" | "server"): Promise<void> }

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
    script: { refuse: (r) => (r.method === "PATCH" && Number(r.body?.days) > 100 ? { status: 422, body: { message: "A task cannot take more than 100 days", key: r.key, field: "days" } } : undefined) },
    async run(page, server) {
      await ready(page);
      const was = await days(page, 1);
      await edit(page, 1, "dur", "120");
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
      await until("both saves", () => writes(server).length === 2, 10_000);
      const [create, rename] = writes(server);
      assert.deepEqual([create!.method, rename!.method, rename!.body], ["POST", "PATCH", { title: "Review" }], "the rename is saved second");
      assert.equal(server.db.tasks.get(rename!.key!)?.title, "Review", "and names the row's key");
    },
  },
  {
    n: 5, name: "5 a refused create with edits behind it",
    script: { latency: (r) => (r.method === "POST" ? 1_500 : 0), refuse: (r) => (r.method === "POST" ? { status: 422, body: { message: "No new tasks today" } } : undefined) },
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
];
