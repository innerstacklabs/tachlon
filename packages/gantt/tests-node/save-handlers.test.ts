// The handlers form of `save` (decision 0022, rule 3.1): an object of one handler per kind of change, called in order through the host's save queue.
// The app is a fake that answers as `App.rollback_to` does; the browser case is scenario 1 of `examples/save/spec.ts`.
import type { Commit } from "../src/events.ts";
import type { ChangeSet, Handlers } from "../src/gantt-types.ts";
import { callSave } from "../src/save-handlers.ts";
import { liveSave, SaveQueue, SaveRefused } from "../src/save-queue.ts";
import { mock, test } from "node:test";
import { expect } from "@playwright/test";

type Change = ChangeSet["changes"][number];
const set = (changes: Change[], seq = 1): ChangeSet => ({ id: `id${seq}`, seq, source: "typed", changes: changes.map((c, n) => ({ ...c, once: `id${seq}:${n}` })) });
const update = (key: string, name: string): Change => ({ record: "task", op: "update", key, set: { name }, was: { name: "old" }, once: "" });
const commit = (seq: number, changes: Commit["changes"]): Commit => ({ seq, source: "typed", changes });
const OK = JSON.stringify({ ok: true, seq: 10 });
const fakeApp = () => {
  const log: string[] = [];
  return { log, acknowledge: (seq: number) => void log.push(`ack ${seq}`), rollback_to: (seq: number) => (log.push(`rollback ${seq}`), OK) };
};
/** Let every promise that can settle settle; `setImmediate` keeps working while a test holds `setTimeout`. */
const settle = async (): Promise<void> => {
  for (let i = 0; i < 5; i++) await new Promise((resolve) => setImmediate(resolve));
};
/** Handlers that log each call's name and arguments (the context left out) and hold it until `release()`. */
function held(): { handlers: Handlers; calls: unknown[][]; contexts: Parameters<NonNullable<Handlers["deleteLink"]>>[1][]; release(): void } {
  const calls: unknown[][] = [];
  const contexts: Parameters<NonNullable<Handlers["deleteLink"]>>[1][] = [];
  const open: (() => void)[] = [];
  const hold = (name: string) => (...args: unknown[]): Promise<void> => {
    contexts.push(args.pop() as never);
    calls.push([name, ...args]);
    return new Promise((resolve) => open.push(resolve));
  };
  const handlers: Handlers = {
    createTask: hold("createTask"), updateTask: hold("updateTask"), deleteTask: hold("deleteTask"),
    createLink: hold("createLink"), updateLink: hold("updateLink"), deleteLink: hold("deleteLink"), updateSchedule: hold("updateSchedule"),
  };
  return { handlers, calls, contexts, release: () => void open.shift()?.() };
}

test("each change calls its own handler, in order, each after the one before has resolved, with the arguments of its kind", async () => {
  const host = held();
  const changeSet = set([
    { record: "task", op: "create", key: null, set: { id: "n1", title: "New" }, once: "" },
    { record: "task", op: "update", key: "t2", set: { days: 5 }, was: { days: 3 }, once: "" },
    { record: "link", op: "create", key: null, set: { id: "l9", source: "n1", target: "t2" }, once: "" },
    { record: "link", op: "update", key: "l1", set: { lag: 1 }, was: { lag: 0 }, once: "" },
    { record: "link", op: "delete", key: "l2", was: { source: "a", target: "b" }, once: "" },
    { record: "task", op: "delete", key: "t3", was: { title: "Old" }, once: "" },
    { record: "schedule", op: "update", key: null, set: { dataDate: "2026-11-02" }, was: { dataDate: null }, once: "" },
  ]);
  let done = false;
  void callSave(host.handlers, changeSet).then(() => (done = true));
  for (let n = 1; n <= 7; n++) {
    await settle();
    expect(host.calls.length).toBe(n);
    host.release();
  }
  await settle();
  expect(done).toBe(true);
  expect(host.calls).toEqual([
    ["createTask", { id: "n1", title: "New" }],
    ["updateTask", "t2", { days: 5 }],
    ["createLink", { id: "l9", source: "n1", target: "t2" }],
    ["updateLink", "l1", { lag: 1 }],
    ["deleteLink", "l2"],
    ["deleteTask", "t3"],
    ["updateSchedule", { dataDate: "2026-11-02" }],
  ]);
  expect(host.contexts.map((c) => c.once)).toEqual([0, 1, 2, 3, 4, 5, 6].map((n) => `id1:${n}`));
  expect(host.contexts.map((c) => c.was)).toEqual([undefined, { days: 3 }, undefined, { lag: 0 }, { source: "a", target: "b" }, { title: "Old" }, { dataDate: null }]);
  expect(host.contexts.every((c) => c.changeSet === changeSet && c.signal instanceof AbortSignal)).toBe(true);
});

test("the signal aborts 10 seconds after the handler is called, and the chart still waits for the handler", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  try {
    const host = held();
    let done = false;
    void callSave(host.handlers, set([update("t1", "a")])).then(() => (done = true));
    await settle();
    const { signal } = host.contexts[0]!;
    mock.timers.tick(9_999);
    expect(signal.aborted).toBe(false);
    mock.timers.tick(1);
    expect(signal.aborted).toBe(true);
    await settle();
    expect(done).toBe(false);
    host.release();
    await settle();
    expect(done).toBe(true);
  } finally {
    mock.timers.reset();
  }
});

test("a handler that rejects ends the call with its error, and the handlers after it do not run", async () => {
  const calls: string[] = [];
  const boom = new Error("no");
  const handlers: Handlers = {
    updateTask: async (key) => void calls.push(key),
    deleteLink: async () => { calls.push("deleteLink"); throw boom; },
    deleteTask: async () => void calls.push("deleteTask"),
  };
  const changeSet = set([update("t1", "a"), { record: "link", op: "delete", key: "l1", once: "" }, { record: "task", op: "delete", key: "t2", once: "" }]);
  await expect(callSave(handlers, changeSet)).rejects.toBe(boom);
  expect(calls).toEqual(["t1", "deleteLink"]);
});

test("a handler that resolves with a Response that is not a 2xx ends the call with that Response, and a 2xx goes on", async () => {
  const calls: string[] = [];
  const refused = new Response("{}", { status: 422 });
  const handlers: Handlers = {
    updateTask: async (key) => (calls.push(key), key === "t2" ? refused : new Response(null, { status: 204 })),
  };
  await expect(callSave(handlers, set([update("t1", "a"), update("t2", "b"), update("t3", "c")]))).rejects.toBe(refused);
  expect(calls).toEqual(["t1", "t2"]);
});

test("each call uses the object the chart holds at that moment", async () => {
  const app = fakeApp();
  const seen: string[] = [];
  const first: Handlers = { updateTask: async (key) => void seen.push(`first ${key}`) };
  const second: Handlers = { updateTask: async (key) => void seen.push(`second ${key}`) };
  let current = first;
  const queue = new SaveQueue(app, liveSave(() => current), () => undefined);
  queue.add([commit(1, [{ record: "task", op: "update", key: "t1", task: 0, set: { name: "a" } }])]);
  await settle();
  current = second;
  queue.add([commit(2, [{ record: "task", op: "update", key: "t2", task: 1, set: { name: "b" } }])]);
  await settle();
  expect(seen).toEqual(["first t1", "second t2"]);
  expect(app.log).toEqual(["ack 1", "ack 2"]);
});

test("a handler's SaveRefused puts that commit and the ones behind it back, as one from a save function does", async () => {
  const app = fakeApp();
  const errors: unknown[] = [];
  const handlers: Handlers = { updateTask: async (key) => { if (key === "t2") throw new SaveRefused({ message: "The task is locked.", key }); } };
  const queue = new SaveQueue(app, handlers, (error) => errors.push(error));
  const task = (n: number): Commit => commit(n, [{ record: "task", op: "update", key: `t${n}`, task: n, set: { name: "x" } }]);
  queue.add([task(1), task(2), task(3)]);
  await settle();
  expect(app.log).toEqual(["ack 1", "rollback 2"]);
  expect(errors).toMatchObject([{ reason: "refused", message: "The task is locked.", key: "t2", seq: 2, undone: 2 }]);
});

test("a call that failed and is made again with another object resumes at the change whose handler did not resolve", async () => {
  mock.method(console, "error", () => undefined);
  try {
    const app = fakeApp();
    const seen: string[] = [];
    const first: Handlers = {
      updateTask: async (key) => { seen.push(`first ${key}`); if (key === "t2") throw new Error("the network is down"); },
    };
    const second: Handlers = { updateTask: async (key) => void seen.push(`second ${key}`) };
    let current = first;
    const queue = new SaveQueue(app, liveSave(() => current), () => undefined);
    queue.add([commit(1, [1, 2, 3].map((n) => ({ record: "task" as const, op: "update" as const, key: `t${n}`, task: n, set: { name: "x" } })))]);
    await settle();
    expect(seen).toEqual(["first t1", "first t2"]);
    current = second;
    queue.resume();
    await settle();
    expect(seen).toEqual(["first t1", "first t2", "second t2", "second t3"]);
    expect(app.log).toEqual(["ack 1"]);
  } finally {
    mock.restoreAll();
  }
});

test("a handler that resolves with an object that has an error, errors or userErrors member warns once for that handler", async () => {
  const warn = mock.method(console, "warn", () => undefined);
  try {
    const handlers: Handlers = {
      updateTask: async () => ({ userErrors: [{ message: "No." }] }),
      deleteTask: async () => ({ errors: ["No."] }),
      updateLink: async () => ({ ok: true }),
      createTask: async () => ({ error: "No." }),
    };
    const create: Change = { record: "task", op: "create", key: null, set: {}, once: "" };
    const drop: Change = { record: "task", op: "delete", key: "t1", once: "" };
    const link: Change = { record: "link", op: "update", key: "l1", set: {}, once: "" };
    await callSave(handlers, set([update("t1", "a"), update("t2", "b"), drop, drop, link, link]));
    expect(warn.mock.calls.map((c) => String(c.arguments[0]).split(" ")[1])).toEqual(["updateTask", "deleteTask"]);
    await callSave(handlers, set([update("t3", "c"), create, create], 2));
    expect(warn.mock.calls.map((c) => String(c.arguments[0]).split(" ")[1])).toEqual(["updateTask", "deleteTask", "createTask"]);
    expect(String(warn.mock.calls[0]!.arguments[0])).toContain('"userErrors"');
  } finally {
    mock.restoreAll();
  }
});

test("a queue stopped while the first change's handler is pending starts no other handler, and acknowledges nothing", async () => {
  const app = fakeApp();
  const host = held();
  const queue = new SaveQueue(app, host.handlers, () => undefined);
  queue.add([commit(1, [
    { record: "task", op: "update", key: "t1", task: 0, set: { name: "a" } },
    { record: "task", op: "update", key: "t2", task: 1, set: { name: "b" } },
  ])]);
  await settle();
  expect(host.calls.length).toBe(1);
  queue.stop();
  host.release();
  await settle();
  expect(host.calls.length).toBe(1);
  expect(app.log).toEqual([]);
});
