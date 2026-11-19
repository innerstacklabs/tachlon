// The key of a row the person makes (decision 0022, rule 5): the chart makes it, or the server does and the answer names it. The app is a fake that
// holds keys as `App.assign_task_key` does; the browser cases are the scenarios of `examples/save/spec.ts`.
import type { Change, Commit } from "../src/events.ts";
import type { ChangeSet, Handlers, Save, SaveError } from "../src/gantt-types.ts";
import { keyMistake, keyRules, liveSave, SaveQueue } from "../src/save-queue.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const NAMES = { task: "id", link: "lid" };
const create = (task: number, set: Record<string, unknown> = {}, key: string | null = null): Change => ({ record: "task", op: "create", key, task, set: { name: `Task ${task}`, ...set } });
const edit = (task: number): Change => ({ record: "task", op: "update", key: null, task, set: { name: "x" }, was: { name: "y" } });
const commit = (seq: number, ...changes: Change[]): Commit => ({ seq, source: "typed", changes });
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** An app that logs `assign`, `ack n` and `rollback n`, and refuses a key another row holds, as the engine does. */
function fakeApp(): { log: string[]; held: Map<string, string>; acknowledge(seq: number): void; rollback_to(seq: number): string; assign_task_key(slot: number, key: string): string; assign_link_key(slot: number, key: string): string } {
  const log: string[] = [];
  const held = new Map<string, string>();
  const assign = (record: string, slot: number, key: string): string => {
    const taken = [...held].some(([at, k]) => k === key && at !== `${record}:${slot}` && at.startsWith(`${record}:`)); // keys are unique for each kind of row
    if (key === "" || taken) return JSON.stringify({ ok: false, reason: key === "" ? "emptyKey" : "keyTaken", message: key === "" ? "A key has at least one character." : `Another row holds the key ${key}.` });
    held.set(`${record}:${slot}`, key);
    log.push(`assign ${record}:${slot}=${key}`);
    return JSON.stringify({ ok: true });
  };
  return { log, held, acknowledge: (seq) => void log.push(`ack ${seq}`), rollback_to: (seq) => (log.push(`rollback ${seq}`), JSON.stringify({ ok: true, seq: 10 })), assign_task_key: (slot, key) => assign("task", slot, key), assign_link_key: (slot, key) => assign("link", slot, key) };
}
const rules = (app: ReturnType<typeof fakeApp>, mode: "chart" | "server", newKey?: () => string) => keyRules(app, mode, newKey, NAMES);

test("a made key replaces the slot in every reference of every queued commit", async () => {
  const app = fakeApp();
  const seen: ChangeSet[] = [];
  let n = 0;
  const queue = new SaveQueue(app, async (cs) => void seen.push(cs), () => undefined, rules(app, "chart", () => `k${++n}`));
  const link: Change = { record: "link", op: "create", key: null, link: 0, set: { from: { task: 5 }, to: { task: 6 }, type: "FS" } };
  queue.add([commit(1, create(5)), commit(2, create(6, { parent: { task: 5 }, after: { task: 5 } }), link, edit(5))]);
  await settle();
  expect(seen[0]!.changes).toMatchObject([{ key: "k1", set: { id: "k1", name: "Task 5" } }]);
  expect(seen[1]!.changes).toMatchObject([
    { key: "k2", set: { id: "k2", parent: "k1", after: "k1" } },
    { record: "link", key: "k3", set: { lid: "k3", from: "k1", to: "k2" } },
    { op: "update", key: "k1" },
  ]);
  expect(seen.flatMap((cs) => cs.changes).filter((c) => "ref" in c)).toEqual([]);
  expect(app.log).toEqual(["assign task:5=k1", "assign task:6=k2", "assign link:0=k3", "ack 1", "ack 2"]);
});

test("with keys: server a create keeps key null and a ref, a row it names is { ref }, and the answered key is assigned before the next commit is called", async () => {
  const app = fakeApp();
  const seen: ChangeSet[] = [];
  const before: string[][] = [];
  const save: Save = async (cs) => (seen.push(cs), before.push([...app.log]), cs.seq === 1 ? { keys: [{ ref: "task:5", key: "s1" }, { ref: "task:6", key: "s2" }] } : undefined);
  const queue = new SaveQueue(app, save, () => undefined, rules(app, "server"));
  queue.add([commit(1, create(5), create(6, { parent: { task: 5 } })), commit(2, edit(5)), commit(3, { ...edit(6), set: { parent: { task: 5 } } })]);
  await settle();
  expect(seen[0]!.changes).toMatchObject([{ key: null, ref: "task:5" }, { key: null, ref: "task:6", set: { parent: { ref: "task:5" } } }]);
  expect(seen[0]!.changes[0]!.set).not.toHaveProperty("id");
  expect(before[1]).toEqual(["assign task:5=s1", "assign task:6=s2", "ack 1"]);
  expect(seen[1]!.changes).toMatchObject([{ key: "s1" }]); // an edit queued behind the create names the key the server made
  expect(seen[2]!.changes).toMatchObject([{ key: "s2", set: { parent: "s1" } }]);
});

for (const [what, answer] of [["an object that holds the key under the host's name", (key: string) => ({ id: key, name: "x" })], ["a 2xx Response whose JSON body is the row", (key: string) => new Response(JSON.stringify({ id: key }), { status: 201 })], ["the key itself", (key: string) => key]] as const) {
  test(`a createTask handler may answer ${what}, and the next handler of the change set gets that key`, async () => {
    const app = fakeApp();
    const rows: Record<string, unknown>[] = [];
    const handlers: Handlers = { createTask: async (row) => (rows.push(row), answer(`s${rows.length}`)) };
    const queue = new SaveQueue(app, handlers, () => undefined, rules(app, "server"));
    queue.add([commit(1, create(5), create(6, { parent: { task: 5 } }))]);
    await settle();
    expect(rows[1]).toMatchObject({ parent: "s1" });
    expect([...app.held]).toEqual([["task:5", "s1"], ["task:6", "s2"]]);
    expect(app.log.at(-1)).toBe("ack 1");
  });
}

test("the undo of a delete creates the row under the key it had, whichever side makes keys", async () => {
  for (const mode of ["chart", "server"] as const) {
    const app = fakeApp();
    const seen: ChangeSet[] = [];
    const queue = new SaveQueue(app, async (cs) => void seen.push(cs), () => undefined, rules(app, mode, () => { throw new Error("made a key"); }));
    queue.add([{ seq: 1, source: "undo", changes: [create(5, {}, "t9")] }]);
    await settle();
    expect(seen[0]!.changes[0], mode).toMatchObject({ key: "t9", set: { id: "t9" } });
    expect("ref" in seen[0]!.changes[0]!, mode).toBe(mode === "server"); // the server may answer it
    expect(app.log, mode).toEqual(["ack 1"]);
  }
});

for (const [what, key, saved] of [["the same key", "t9", true], ["no key", undefined, true], ["a different key", "t10", false]] as const) {
  for (const form of ["function", "handlers"] as const) {
    test(`under keys: server, ${form} form, a create that carries its key answered with ${what} is ${saved ? "saved" : "put back with a message that names both keys"}`, async () => {
      const app = fakeApp();
      const errors: SaveError[] = [];
      const save: Save | Handlers = form === "function" ? async () => (key === undefined ? undefined : { keys: [{ ref: "task:5", key }] }) : { createTask: async () => (key === undefined ? {} : { id: key }) };
      const queue = new SaveQueue(app, save, (error) => errors.push(error), rules(app, "server"));
      queue.add([{ seq: 1, source: "undo", changes: [create(5, {}, "t9")] }]);
      await settle();
      if (saved) expect([errors, app.log]).toEqual([[], ["ack 1"]]);
      else expect([errors[0]?.reason, errors[0]?.message, app.log]).toEqual(["result", expect.stringMatching(/"t10".*"t9".*cannot yet take a new key.*setData/), ["rollback 1"]]);
    });
  }
}

for (const [what, answer] of [["a number", { id: 57 }], ["an empty key", { id: "" }], ["a Response whose JSON is a number key", new Response('{"id":57}')]] as const) {
  test(`under keys: server, handlers form, a restored create answered with ${what} is put back with the reason "result"`, async () => {
    const app = fakeApp();
    const errors: SaveError[] = [];
    const queue = new SaveQueue(app, { createTask: async () => answer }, (error) => errors.push(error), rules(app, "server"));
    queue.add([{ seq: 1, source: "undo", changes: [create(5, {}, "t9")] }]);
    await settle();
    expect([errors[0]?.reason, errors[0]?.message, app.log]).toEqual(["result", expect.stringMatching(/the restored task under the key.*but the chart has it under "t9"/), ["rollback 1"]]);
  });
}

test("under keys: server, handlers form, a restored create answered with another key stops the change set before the next handler runs", async () => {
  const app = fakeApp();
  const errors: SaveError[] = [];
  const called: string[] = [];
  const link: Change = { record: "link", op: "create", key: null, link: 0, set: { from: { task: 5 }, to: { task: 6 }, type: "FS" } };
  const handlers: Handlers = { createTask: async () => (called.push("createTask"), { id: "t10" }), createLink: async () => void called.push("createLink") };
  const queue = new SaveQueue(app, handlers, (error) => errors.push(error), rules(app, "server"));
  queue.add([{ seq: 1, source: "undo", changes: [create(5, {}, "t9"), link] }]);
  await settle();
  expect([called, errors[0]?.reason, errors[0]?.message, app.log]).toEqual([["createTask"], "result", expect.stringMatching(/"t10".*"t9"/), ["rollback 1"]]);
});

test("a task and a link may hold the same key, as keys are unique for each kind of row", async () => {
  const app = fakeApp();
  const link: Change = { record: "link", op: "create", key: null, link: 0, set: { from: "a", to: "b" } };
  const queue = new SaveQueue(app, async () => ({ keys: [{ ref: "task:5", key: "1" }, { ref: "link:0", key: "1" }] }), () => undefined, rules(app, "server"));
  queue.add([commit(1, create(5), link)]);
  await settle();
  expect(app.log).toEqual(["assign task:5=1", "assign link:0=1", "ack 1"]);
});

test("a put-back forgets the keys it had assigned, so a create at a freed slot is asked for again", async () => {
  const app = fakeApp();
  app.held.set("task:1", "t1");
  const seen: ChangeSet[] = [];
  const save: Save = async (cs) => (seen.push(cs), cs.seq === 1 ? { keys: [{ ref: "task:5", key: "s1" }, { ref: "task:6", key: "t1" }] } : undefined);
  const queue = new SaveQueue(app, save, () => undefined, rules(app, "server"));
  queue.add([commit(1, create(5), create(6))]); // the second key is taken: both creates go back
  await settle();
  queue.add([commit(11, create(5), create(6, { parent: { task: 5 } }))]);
  await settle();
  expect(seen[1]!.changes).toMatchObject([{ key: null, ref: "task:5" }, { key: null, ref: "task:6", set: { parent: { ref: "task:5" } } }]);
});

const BAD: [string, "chart" | "server", (cs: ChangeSet) => unknown, RegExp][] = [
  ["a key is missing", "server", () => ({ keys: [] }), /no key for the new row task:5/],
  ["the result has no keys at all", "server", () => undefined, /no key for the new row task:5/],
  ["a key is extra", "server", () => ({ keys: [{ ref: "task:5", key: "s1" }, { ref: "task:6", key: "s3" }, { ref: "task:9", key: "s2" }] }), /key for task:9, which no create/],
  ["a ref is answered twice", "server", () => ({ keys: [{ ref: "task:5", key: "s1" }, { ref: "task:6", key: "s3" }, { ref: "task:5", key: "s2" }] }), /"task:5" twice/],
  ["a key is given to two rows", "server", () => ({ keys: [{ ref: "task:5", key: "s1" }, { ref: "task:6", key: "s1" }] }), /"s1" twice/],
  ["a key is taken by another row", "server", () => ({ keys: [{ ref: "task:5", key: "t1" }, { ref: "task:6", key: "s2" }] }), /Another row holds the key t1/],
  ["a keys entry is null", "server", () => ({ keys: [null] }), /keys\[0\] of the save result is not an object with a text ref/],
  ["a keys entry has no ref", "server", () => ({ keys: [{ key: "s1" }] }), /keys\[0\] .* text ref/],
  ["keys is not a list", "server", () => ({ keys: {} }), /not a list of \{ ref, key \}/],
  ["keys is null", "server", () => ({ keys: null }), /not a list of \{ ref, key \}/],
  ["a key is not text", "server", () => ({ keys: [{ ref: "task:5", key: 7 }, { ref: "task:6", key: "s2" }] }), /at least one character/],
  ["keys come back under keys: chart", "chart", () => ({ keys: [{ ref: "task:5", key: "s1" }] }), /mount with keys: "server"/],
];
for (const [what, mode, answer, message] of BAD) {
  test(`${what}: the edit is put back with the reason "result" and a message that names it`, async () => {
    const app = fakeApp();
    app.held.set("task:1", "t1");
    const errors: SaveError[] = [];
    const queue = new SaveQueue(app, async (cs) => answer(cs) as never, (error) => errors.push(error), rules(app, mode));
    queue.add([commit(1, create(5), create(6))]);
    await settle();
    expect(errors).toMatchObject([{ reason: "result", seq: 1, undone: 1 }]);
    expect(errors[0]!.message).toMatch(message);
    expect(app.log.filter((line) => /^(ack|rollback)/.test(line))).toEqual(["rollback 1"]);
  });
}

test("a createTask handler that answers no key is a result the chart cannot use, and a key the app refuses puts a chart-made create back", async () => {
  const app = fakeApp();
  app.held.set("task:1", "t1");
  const errors: SaveError[] = [];
  const queue = new SaveQueue(app, { createTask: async () => ({}) }, (error) => errors.push(error), rules(app, "server"));
  queue.add([commit(1, create(5))]);
  await settle();
  const chart = new SaveQueue(app, async () => undefined, (error) => errors.push(error), rules(app, "chart", () => "t1"));
  chart.add([commit(2, create(6))]);
  await settle();
  expect(errors).toMatchObject([{ reason: "result", message: /resolved with no key for the new row.*as "id"/ }, { reason: "result", message: /"t1" for a new task was refused/, calls: 0 }]);
});

test("a wrapper that holds no save leaves the keys of the rows to the host, which gives them with assignKey", async () => {
  const app = fakeApp();
  let held: Save | undefined;
  const queue = new SaveQueue(app, liveSave(() => held), () => undefined, rules(app, "chart"));
  queue.add([commit(1, create(5))]);
  held = async () => undefined;
  queue.add([commit(2, create(6))]);
  await settle();
  expect(app.log).toEqual(["assign task:6=" + app.held.get("task:6"), "ack 2"]);
});

test("with save, a row with no key or the key another row of its kind holds is refused by name", () => {
  expect(keyMistake("task", [{ key: "a" }, { key: "b" }])).toBeUndefined();
  expect(keyMistake("task", undefined)).toBeUndefined();
  expect(keyMistake("task", [{ key: "a" }, {}])).toMatch(/^noKey: .*task 1 \(from 0\)/);
  expect(keyMistake("link", [{ key: "a" }, { key: "a" }])).toMatch(/^duplicateKey: two links have the key "a"/);
});
