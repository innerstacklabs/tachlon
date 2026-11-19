// The host's save queue (decision 0022, rule 3.3): the host's `save` function gets each commit as a change set, one call at a time, in `seq` order, and a
// `SaveRefused` puts that commit and the ones behind it back. The app is a fake that answers as `App.rollback_to` does; the browser case is a later row.
import type { Commit, GanttEvents } from "../src/events.ts";
import type { ChangeSet, Save, SaveError } from "../src/gantt-types.ts";
import { liveSave, SaveQueue, SaveRefused } from "../src/save-queue.ts";
import { mock, test } from "node:test";
import { expect } from "@playwright/test";

const commit = (seq: number, over: Partial<Commit> = {}): Commit => ({
  seq,
  source: "typed",
  changes: [{ record: "task", op: "update", key: `k${seq}`, task: seq, set: { name: `new ${seq}` }, was: { name: "old" } }],
  ...over,
});
const OK = JSON.stringify({ ok: true, seq: 10 });
const GONE = JSON.stringify({ ok: false, reason: "rollbackGone", message: "gone", snapshot: true, seq: 3 });
const BUSY = JSON.stringify({ ok: false, reason: "busy", message: "busy", snapshot: true, seq: 3 });

/** An app that logs `ack n` and `rollback n`, and answers each `rollback_to` with the next of `answers`, else `OK`. */
function fakeApp(answers: string[] = []): { log: string[]; acknowledge(seq: number): void; rollback_to(seq: number): string } {
  const log: string[] = [];
  return { log, acknowledge: (seq) => void log.push(`ack ${seq}`), rollback_to: (seq) => (log.push(`rollback ${seq}`), answers.shift() ?? OK) };
}
/** Let every promise that can settle settle. */
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));
/** A save that records the `seq` of each call and holds it until `release()`; `most` is the most calls running at once. */
function held(): { save: (cs: ChangeSet) => Promise<void>; seqs: number[]; release(): void; most: () => number } {
  const seqs: number[] = [];
  const open: (() => void)[] = [];
  let most = 0;
  const save = (cs: ChangeSet): Promise<void> => {
    seqs.push(cs.seq);
    return new Promise((resolve) => {
      open.push(resolve);
      most = Math.max(most, open.length);
    });
  };
  return { save, seqs, release: () => void open.shift()?.(), most: () => most };
}

test("three commits reach save in seq order, one call in flight at a time, each acknowledged after its save", async () => {
  const app = fakeApp();
  const host = held();
  const queue = new SaveQueue(app, host.save, () => undefined);
  queue.add([commit(1), commit(2)]);
  queue.add([commit(3)]);
  await settle();
  expect(host.seqs).toEqual([1]);
  for (const sent of [[1, 2], [1, 2, 3]]) {
    host.release();
    await settle();
    expect(host.seqs).toEqual(sent);
  }
  host.release();
  await settle();
  expect(host.most()).toBe(1);
  expect(app.log).toEqual(["ack 1", "ack 2", "ack 3"]);
});

test("a SaveRefused on the second commit puts back the second and the third, and fires one error", async () => {
  const app = fakeApp();
  const seen: ChangeSet[] = [];
  const errors: SaveError[] = [];
  const queue = new SaveQueue(app, async (cs) => {
    seen.push(cs);
    if (cs.seq === 2) throw new SaveRefused({ message: "The task is locked.", key: "k2", field: "name" });
  }, (error) => errors.push(error));
  queue.add([commit(1), commit(2), commit(3)]);
  await settle();
  expect(seen.map((cs) => cs.seq)).toEqual([1, 2]);
  expect(app.log).toEqual(["ack 1", "rollback 2"]);
  expect(errors).toEqual([{ source: "save", message: "The task is locked.", fatal: false, reason: "refused", calls: 1, key: "k2", field: "name", seq: 2, undone: 2, reload: false }]);
  // The rollback commit is seq 10: a commit before it that is read late was taken back too, and the first one after it is saved.
  queue.add([commit(4), commit(10, { source: "rollback", changes: [] }), commit(11)]);
  await settle();
  expect(seen.map((cs) => cs.seq)).toEqual([1, 2, 11]);
});

test("a commit the app no longer holds sets reload, puts nothing back and empties the queue", async () => {
  const app = fakeApp([GONE]);
  const seen: number[] = [];
  const errors: SaveError[] = [];
  const queue = new SaveQueue(app, async (cs) => {
    seen.push(cs.seq);
    if (cs.seq === 1) throw new SaveRefused({ message: "No." });
  }, (error) => errors.push(error));
  queue.add([commit(1), commit(2)]);
  await settle();
  expect(errors).toMatchObject([{ reason: "refused", seq: 1, undone: 0, reload: true }]);
  expect("key" in errors[0]!).toBe(false);
  queue.add([commit(4)]);
  await settle();
  expect(seen).toEqual([1, 4]);
});

test("a rollback commit and a commit with no change for the host are not sent", async () => {
  const app = fakeApp();
  const seen: number[] = [];
  const queue = new SaveQueue(app, async (cs) => void seen.push(cs.seq), () => undefined);
  queue.add([commit(1, { source: "rollback" }), commit(2, { changes: [] }), commit(3)]);
  await settle();
  expect(seen).toEqual([3]);
  expect(app.log).toEqual(["ack 3"]);
});

test("a put-back that finds the app busy is tried again until it answers, and the error comes after", async () => {
  const app = fakeApp([BUSY, BUSY, OK]);
  const errors: SaveError[] = [];
  const queue = new SaveQueue(app, async () => { throw new SaveRefused({ message: "No." }); }, (error) => errors.push(error));
  queue.add([commit(1), commit(2)]);
  await settle();
  expect(errors).toEqual([]);
  await new Promise((resolve) => setTimeout(resolve, 200));
  expect(app.log).toEqual(["rollback 1", "rollback 1", "rollback 1"]);
  expect(errors).toMatchObject([{ seq: 1, undone: 2, reload: false }]);
});

test("one change set keeps its id and each change's once on every call, and carries no session slot", async () => {
  mock.method(console, "error", () => undefined);
  const sets: ChangeSet[] = [];
  let failing = true;
  const queue = new SaveQueue(fakeApp(), async (cs) => {
    sets.push(cs);
    if (failing) throw new Error("the network is down");
  }, () => undefined);
  const two = commit(1, { changes: [...commit(1).changes, { record: "link", op: "delete", key: "l1", link: 7, was: { from: "a", to: "b" } }] });
  queue.add([two, commit(2)]);
  await settle();
  failing = false;
  queue.resume();
  await settle();
  const [first, again, next] = sets as [ChangeSet, ChangeSet, ChangeSet];
  expect(again).toBe(first);
  expect(first.id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  expect(first.changes.map((c) => c.once)).toEqual([`${first.id}:0`, `${first.id}:1`]);
  expect(first.changes.some((c) => "task" in c || "link" in c)).toBe(false);
  expect(next.id).not.toBe(first.id);
  mock.restoreAll();
});

test("any other throw puts nothing back and stops the queue", async () => {
  mock.method(console, "error", () => undefined);
  const app = fakeApp();
  const seen: number[] = [];
  const errors: SaveError[] = [];
  const queue = new SaveQueue(app, async (cs) => {
    seen.push(cs.seq);
    throw new TypeError("fetch failed");
  }, (error) => errors.push(error));
  queue.add([commit(1), commit(2)]);
  await settle();
  queue.add([commit(3)]);
  await settle();
  expect(seen).toEqual([1]);
  expect(app.log).toEqual([]);
  expect(errors).toEqual([]);
  mock.restoreAll();
});

test("a call that is running when the queue stops finishes, and its result is ignored", async () => {
  const app = fakeApp();
  const host = held();
  const queue = new SaveQueue(app, host.save, () => undefined);
  queue.add([commit(1), commit(2)]);
  await settle();
  queue.stop();
  host.release();
  await settle();
  expect(host.seqs).toEqual([1]);
  expect(app.log).toEqual([]);
});

test("a live save with none writes no change set and acknowledges nothing; once there is one, a synchronous save that returns nothing has saved", async () => {
  const app = fakeApp();
  const uuid = mock.method(globalThis.crypto, "randomUUID");
  const seen: number[] = [];
  let current: Save | undefined;
  const queue = new SaveQueue(app, liveSave(() => current), () => undefined);
  queue.add([commit(1)]);
  await settle();
  expect(uuid.mock.callCount(), "no change set is written").toBe(0);
  expect(app.log).toEqual([]);
  current = ((cs: ChangeSet) => void seen.push(cs.seq)) as never;
  queue.add([commit(2)]);
  await settle();
  expect(seen).toEqual([2]);
  expect(app.log).toEqual(["ack 2"]);
  mock.restoreAll();
});

test("a page with no crypto.randomUUID still gets a v4 change-set id, and the save is called", async () => {
  const app = fakeApp();
  const seen: ChangeSet[] = [];
  const real = crypto.randomUUID;
  Object.defineProperty(crypto, "randomUUID", { value: undefined, configurable: true });
  try {
    const queue = new SaveQueue(app, async (cs) => void seen.push(cs), () => undefined);
    queue.add([commit(1)]);
    await settle();
  } finally {
    Object.defineProperty(crypto, "randomUUID", { value: real, configurable: true });
  }
  expect(seen.map((cs) => cs.id)).toEqual([expect.stringMatching(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/)]);
  expect(app.log).toEqual(["ack 1"]);
});

test("an error whose source is save is a SaveError, so a host reads reload and undone with no cast", () => {
  const read = (e: GanttEvents["error"]) => (e.source === "save" ? [e.reload, e.undone] : []);
  expect(read({ source: "save", message: "m", fatal: false, reason: "refused", calls: 1, seq: 2, undone: 3, reload: true })).toEqual([true, 3]);
});

test("a host commit is not saved, but a person's Undo and Redo of it are, each with was and set", async () => {
  const app = fakeApp();
  const seen: ChangeSet[] = [];
  const queue = new SaveQueue(app, async (cs) => void seen.push(cs), () => undefined);
  queue.add([commit(1, { source: "host" }), commit(2, { source: "undo" }), commit(3, { source: "redo" })]);
  await settle();
  expect(seen.map((cs) => [cs.seq, cs.source, cs.changes[0]!.was, cs.changes[0]!.set])).toEqual([[2, "undo", { name: "old" }, { name: "new 2" }], [3, "redo", { name: "old" }, { name: "new 3" }]]);
  expect(app.log).toEqual(["ack 2", "ack 3"]);
});
