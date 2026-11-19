// `ViewState.unsavedCommits` (decision 0022, rule 3.9) is the real save queue's own count. The app is a fake that answers as `App.rollback_to` does;
// a host that holds a restart until the count reaches 0 is released on every way a commit ends.
import type { Commit } from "../src/events.ts";
import type { ChangeSet, SaveError } from "../src/gantt-types.ts";
import { liveSave, SaveQueue, SaveRefused } from "../src/save-queue.ts";
import { mock, test } from "node:test";
import { expect } from "@playwright/test";

const commit = (seq: number, over: Partial<Commit> = {}): Commit => ({
  seq,
  source: "typed",
  changes: [{ record: "task", op: "update", key: `k${seq}`, task: seq, set: { name: `new ${seq}` }, was: { name: "old" } }],
  ...over,
});
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));
/** A queue over a fake app (a rollback commit is seq 10) with the counts it told. `hold()` is a host that waits for 0 before it restarts the chart: `released()` says it may. */
function setup(save: (cs: ChangeSet) => Promise<void>): { queue: SaveQueue; told: number[]; hold(): void; released(): boolean } {
  const told: number[] = [];
  let holding = false;
  let released = false;
  const app = { acknowledge: () => undefined, rollback_to: () => JSON.stringify({ ok: true, seq: 10 }) };
  const queue = new SaveQueue(app, save, () => undefined, undefined, (n) => {
    told.push(n);
    if (holding && n === 0) released = true;
  });
  return {
    queue,
    told,
    hold: () => {
      holding = true;
      released = queue.unsavedCommits === 0;
    },
    released: () => released,
  };
}
const refuseFirst = async (cs: ChangeSet): Promise<void> => {
  if (cs.seq === 1) throw new SaveRefused({ message: "No." });
};
const hang = (): Promise<void> => new Promise(() => undefined);

test("the count falls as each commit is saved, and is told at each change", async () => {
  const { queue, told } = setup(async () => undefined);
  queue.add([commit(1), commit(2)]);
  expect(queue.unsavedCommits).toBe(2);
  await settle();
  expect(queue.unsavedCommits).toBe(0);
  expect(told).toEqual([2, 1, 0]);
});

test("a refusal puts back the commit and every commit behind it: the count is 0", async () => {
  const { queue, told } = setup(refuseFirst);
  queue.add([commit(1), commit(2), commit(3)]);
  expect(queue.unsavedCommits).toBe(3);
  await settle();
  expect(queue.unsavedCommits).toBe(0);
  expect(told).toEqual([3, 0]);
});

test("a commit read after the put-back with a seq below its floor is dropped, not counted", async () => {
  const { queue, told } = setup(refuseFirst);
  queue.add([commit(1)]);
  await settle();
  queue.add([commit(4), commit(10, { source: "rollback", changes: [] })]);
  expect(queue.unsavedCommits).toBe(0);
  expect(told).toEqual([1, 0]);
  queue.add([commit(11)]);
  expect(queue.unsavedCommits).toBe(1);
});

test("stop drops the commits still waiting: the count is 0 and told, with a call running", async () => {
  const { queue, told } = setup(hang);
  queue.add([commit(1), commit(2)]);
  await settle();
  expect(queue.unsavedCommits).toBe(2);
  queue.stop();
  expect(queue.unsavedCommits).toBe(0);
  expect(told).toEqual([2, 0]);
});

test("a wrapper with no save yet drops each commit it is given: the count is 0 and told", async () => {
  const told: number[] = [];
  const queue = new SaveQueue({ acknowledge: () => undefined, rollback_to: () => "" }, liveSave(() => undefined), () => undefined, undefined, (n) => void told.push(n));
  queue.add([commit(1)]);
  await settle();
  expect(queue.unsavedCommits).toBe(0);
  expect(told).toEqual([1, 0]);
});

test("a save that throws a plain Error stops the queue: one error event, the held commits stay counted, no further call", async () => {
  const errors: SaveError[] = [];
  let calls = 0;
  const app = { acknowledge: () => undefined, rollback_to: () => "" };
  const queue = new SaveQueue(app, async () => { calls++; throw new Error("save failed: 503"); }, (e) => errors.push(e));
  const quiet = mock.method(console, "error", () => undefined);
  queue.add([commit(1)]);
  await settle();
  queue.add([commit(2), commit(3)]);
  await settle();
  expect(errors.map((e) => [e.reason, e.message, e.held, e.undone])).toEqual([["stopped", "save failed: 503", 1, 0]]);
  for (const [thrown, message] of [[new Response(null, { status: 422 }), "the save answered with status 422"], [{ code: 7 }, "a save threw, so the chart stopped saving"]] as const) { new SaveQueue(app, async () => { throw thrown; }, (e) => errors.push(e)).add([commit(1)]); await settle(); expect(errors.at(-1)?.message).toBe(message); }
  quiet.mock.restore();
  expect([queue.unsavedCommits, calls]).toEqual([3, 1]);
});

test("a call that never settles holds the count", async () => {
  const { queue, told } = setup(hang);
  queue.add([commit(1), commit(2)]);
  await settle();
  await settle();
  expect(queue.unsavedCommits).toBe(2);
  expect(told).toEqual([2]);
});

for (const [way, save, act] of [
  ["saved", async () => undefined, (): void => undefined],
  ["put back by a refusal", refuseFirst, (): void => undefined],
  ["dropped below the floor after a put-back", refuseFirst, (): void => undefined],
  ["dropped by stop", hang, (queue: SaveQueue): void => queue.stop()],
] as const) {
  test(`a host that holds a restart until the count is 0 is released when a commit is ${way}`, async () => {
    const { queue, hold, released } = setup(save);
    const late = way.startsWith("dropped below");
    queue.add([commit(1), commit(2)]);
    if (!late) hold();
    expect(released()).toBe(false);
    act(queue);
    await settle();
    if (late) (queue.add([commit(4)]), hold()); // made before the put-back, read after it: never counted
    expect(released()).toBe(true);
    expect(queue.unsavedCommits).toBe(0);
  });
}

test("a host that holds a restart is not released while a call never settles", async () => {
  const { queue, hold, released } = setup(hang);
  queue.add([commit(1)]);
  hold();
  await settle();
  expect(released()).toBe(false);
});
