// What a host's answer means (decision 0022, rule 3.5): `judge` reads one throw or one resolved `Response`, and the save queue acts on the verdict.
import type { Commit } from "../src/events.ts";
import type { Handlers, Save, SaveError } from "../src/gantt-types.ts";
import { type Classify, judge, statusOf } from "../src/save-status.ts";
import { SaveQueue, SaveRefused } from "../src/save-queue.ts";
import { mock, test } from "node:test";
import { expect } from "@playwright/test";

const is = async (value: unknown, thrown: boolean, classify?: Classify): Promise<string> => (await judge(value, thrown, classify)).is;
const answer = (status: number, body?: unknown): Response => new Response(body === undefined ? null : typeof body === "string" ? body : JSON.stringify(body), { status });

test("a resolved answer is judged by its status: 2xx saved, 401 stopped, 408, 429 and 5xx failed, any other a refusal", async () => {
  const table: [number[], string][] = [[[200, 201, 204], "saved"], [[401], "stopped"], [[408, 429, 500, 503, 599], "failed"], [[302, 400, 403, 404, 409, 422], "refused"]];
  for (const [statuses, verdict] of table) for (const status of statuses) expect(await is(answer(status), false), `${status} resolved`).toBe(verdict);
});

test("a thrown answer is judged by the same table, except that a 2xx is a failure", async () => {
  const table: [number[], string][] = [[[200, 204], "failed"], [[401], "stopped"], [[408, 429, 500, 503], "failed"], [[400, 403, 404, 422], "refused"]];
  for (const [statuses, verdict] of table) for (const status of statuses) {
    expect(await is(Object.assign(new Error("x"), { status }), true), `${status} on an error`).toBe(verdict);
    expect(await is(answer(status), true), `${status} on a Response`).toBe(verdict);
  }
});

test("the status is the first number from 100 to 599 in status, response.status or statusCode; 0, 99 and 600 are no status, and no status is a failure", async () => {
  for (const status of [0, 99, 600, -1, 422.5, "422", null]) {
    expect(statusOf({ status }), `status ${String(status)}`).toBeUndefined();
    expect(await is(Object.assign(new Error("x"), { status }), true), `a throw with status ${String(status)}`).toBe("failed");
  }
  expect(statusOf({ response: { status: 409 } })).toBe(409);
  expect(statusOf({ statusCode: 429 })).toBe(429);
  expect(statusOf({ status: 0, response: { status: 0 }, statusCode: 422 }), "a member out of range is passed over").toBe(422);
  expect(statusOf({ status: 503, statusCode: 422 }), "the first one in range wins").toBe(503);
  expect(await is(new Error("lost connection"), true)).toBe("failed");
});

test("classify says what a throw means: a word is that outcome, a number is a status, any other number is no status, undefined goes on to the table", async () => {
  const error = Object.assign(new Error("x"), { status: 422 });
  const says = (outcome: ReturnType<Classify>): Promise<string> => is(error, true, () => outcome);
  for (const word of ["refused", "stopped", "failed"] as const) expect(await says(word), word).toBe(word);
  expect(await says(503), "a number in range is judged by the table").toBe("failed");
  expect(await says(401)).toBe("stopped");
  expect(await says(404)).toBe("refused");
  for (const number of [0, 99, 600]) expect(await says(number), `${number}: no status, so the chart goes to the last step, though the error says 422`).toBe("failed");
  expect(await says(undefined), "the error's own status is read").toBe("refused");
  for (const stray of ["saved", "yes", "", {}, true, null] as unknown as ReturnType<Classify>[]) expect(await says(stray), `${JSON.stringify(stray)} is none of the three words`).toBe("failed");
  expect(await is(answer(422), false, () => "failed"), "classify is asked of a throw, not of a resolved Response").toBe("refused");
});

test("a thrown SaveRefused is a refusal whatever classify says, and classify is not asked", async () => {
  const classify = mock.fn<Classify>(() => "failed");
  const refused = new SaveRefused({ message: "No.", key: "t1", field: "days" });
  expect(await judge(refused, true, classify)).toEqual({ is: "refused", message: "No.", key: "t1", field: "days" });
  expect(classify.mock.callCount()).toBe(0);
});

test("a refusal's message is the SaveRefused's, the error's, or the message of the response's JSON body, else the chart's own with the status; key and field come from the body", async () => {
  expect(await judge(Object.assign(new Error("Locked by a planner"), { status: 409 }), true)).toEqual({ is: "refused", message: "Locked by a planner" });
  expect(await judge(answer(422, { message: "Too long", key: "t2", field: "days" }), false)).toEqual({ is: "refused", message: "Too long", key: "t2", field: "days" });
  expect(await judge(answer(422, { message: 7, key: 3 }), true)).toEqual({ is: "refused", message: "The server refused the change (422)" });
  expect(await judge(answer(422, "That change is not allowed"), false)).toEqual({ is: "refused", message: "The server refused the change (422)" });
  expect(await judge(new Error("x"), true, () => "refused")).toEqual({ is: "refused", message: "x" });
  expect(await judge({}, true, () => "refused"), "no status: the line says no more").toEqual({ is: "refused", message: "The server refused the change" });
  expect(await judge(new SaveRefused({ message: "" }), true), "an empty message falls back").toEqual({ is: "refused", message: "The server refused the change" });
});

// The queue, over a fake app whose rollback commit is seq 10.
const commit = (seq: number): Commit => ({ seq, source: "typed", changes: [{ record: "task", op: "update", key: `t${seq}`, task: seq, set: { name: "new" }, was: { name: "old" } }] });
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 5));
async function run(save: Save | Handlers, dim: number[][] = [], commits: Commit[] = [commit(1), commit(2)]): Promise<{ errors: SaveError[]; rolled: number[]; acked: number[]; queue: SaveQueue }> {
  const [errors, rolled, acked]: [SaveError[], number[], number[]] = [[], [], []];
  const app = { acknowledge: (seq: number) => void acked.push(seq), rollback_to: (seq: number) => (rolled.push(seq), JSON.stringify({ ok: true, seq: 10 })) };
  const queue = new SaveQueue(app, save, (e) => errors.push(e), undefined, undefined, (tasks) => dim.push([...tasks]));
  mock.method(console, "error", () => undefined);
  queue.add(commits);
  await settle();
  return { errors, rolled, acked, queue };
}

test("the queue puts back on a refusal only: a 422 put back with the response's key and field, a 401, a 503 and a throw with no status stop it", async () => {
  const refusing = await run(async () => answer(422, { message: "Too long", key: "t2", field: "days" }));
  expect(refusing.errors.map((e) => [e.reason, e.message, e.key, e.field, e.undone])).toEqual([["refused", "Too long", "t2", "days", 2]]);
  expect(refusing.rolled).toEqual([1]);
  for (const save of [async () => answer(401), async () => { throw answer(503); }, async () => { throw Object.assign(new Error("offline"), { status: 0 }); }] as Save[]) {
    const stopped = await run(save);
    expect(stopped.errors.map((e) => [e.reason, e.held]), "stopped, both commits held").toEqual([["stopped", 2]]);
    expect(stopped.rolled).toEqual([]);
  }
});

test("a refusal in the handlers form that names no key takes the key of the change whose handler was refused, and a save function's refusal names none", async () => {
  const handlers: Handlers = { updateTask: async (key) => { if (key === "t1") throw Object.assign(new Error("No"), { status: 403 }); } };
  expect((await run(handlers)).errors.map((e) => [e.key, e.message])).toEqual([["t1", "No"]]);
  const named: Handlers = { updateTask: async () => { throw new SaveRefused({ message: "No", key: "other" }); } };
  expect((await run(named)).errors.map((e) => e.key), "a key the refusal names is kept").toEqual(["other"]);
  expect((await run(async () => { throw new SaveRefused({ message: "No" }); })).errors.map((e) => e.key)).toEqual([undefined]);
  const link = { seq: 1, source: "typed", changes: [{ record: "link", op: "delete", key: "t1", link: 4, was: { from: "a", to: "b" } }] } as Commit; // a link whose key is also a task's
  const refusedLink: Handlers = { deleteLink: async () => { throw Object.assign(new Error("No"), { status: 409 }); } };
  expect((await run(refusedLink, [], [link])).errors.map((e) => [e.reason, e.key]), "a link's key names no task to select").toEqual([["refused", undefined]]);
});

test("a handlers object's classify is asked for each throw, and a handler's non-2xx Response is judged by its status", async () => {
  const classify = mock.fn<Classify>(() => "refused");
  const thrown: Handlers = { updateTask: async () => { throw new Error("Rejected by the schema"); }, classify };
  expect((await run(thrown)).errors.map((e) => [e.reason, e.message])).toEqual([["refused", "Rejected by the schema"]]);
  expect(classify.mock.callCount()).toBe(1);
  const resolved: Handlers = { updateTask: async () => answer(500), classify };
  expect((await run(resolved)).errors.map((e) => e.reason), "a 500 resolved ends the call and stops the queue").toEqual(["stopped"]);
  expect(classify.mock.callCount(), "classify is not asked about a resolved Response").toBe(1);
});

test("the tasks of every unsaved commit are told from the moment they are queued until each settles", async () => {
  const dim: number[][] = [];
  const { acked } = await run(async () => undefined, dim);
  expect(acked).toEqual([1, 2]);
  expect(dim).toEqual([[1, 2], [2], []]);
  const held: number[][] = [];
  const { queue } = await run(() => new Promise(() => undefined), held);
  expect(held, "a call that has not settled keeps both dimmed").toEqual([[1, 2]]);
  queue.stop();
  expect(held.at(-1)).toEqual([]);
});
