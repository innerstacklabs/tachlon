// The drag table `demos/bench/profile.ts --drag` prints (docs/PROFILING.md): each side's median p50 and p95 of the frame interval and the
// drag response, and with two sides whether the second side's median p95 lies inside the first side's run-to-run spread; and what `dragOnce` says when the page never answers.
import assert from "node:assert/strict";
import { test } from "node:test";
import { DRAG_WAIT_MS, dragOnce, READ_MS } from "../../../demos/bench/profile.ts";
import { type DragRun, dragTable } from "../../../demos/bench/profile-format.ts";

const run = (p95: number, response = p95): DragRun => ({ head: { interval: [16.7, p95], response: [16.5, response] }, critical: { interval: [16.7, 18], response: [16.5, 18] } });

test("the drag table gives each side's medians and the spread, and calls a p95 outside the other side's spread outside", () => {
  const table = dragTable([run(18.4), run(18.6), run(18.5)], [run(18.5), run(25, 18.5)], ["main", "head"]);
  assert.match(table, /\| head-task drag, frame interval \| 3\/2 \| 16\.7 \| 18\.5 \(18\.4 to 18\.6\) \| 16\.7 \| 21\.8 \(18\.5 to 25\) \| outside \|/);
  assert.match(table, /\| head-task drag, response \| 3\/2 \| 16\.5 \| 18\.5 \(18\.4 to 18\.6\) \| 16\.5 \| 18\.5 \(18\.5 to 18\.5\) \| within \|/);
  assert.match(table, /\| critical-path drag, response \| 3\/2 \| 16\.5 \| 18 \(18 to 18\) \| 16\.5 \| 18 \(18 to 18\) \| within \|/);
});

test("one set of drag runs gives one set of columns and no verdict", () => {
  const table = dragTable([run(18.4), run(18.6), run(18.5)], undefined, ["median"]);
  assert.match(table, /^\| drag \(ms\) \| runs \| median p50 \| median p95 \(spread\) \|\n\|---\|---\|---\|---\|\n/);
  assert.match(table, /\| head-task drag, frame interval \| 3 \| 16\.7 \| 18\.5 \(18\.4 to 18\.6\) \|\n/);
  assert.doesNotMatch(table, /within|outside/);
});

/** A session whose page's drag wait times out after reporting a page error, a console error and a failed request; `evaluate` is the test's. */
function stalledSession(evaluate: (fn: () => string) => Promise<string>, timeouts: number[] = []) {
  const handlers = new Map<string, (x: any) => void>();
  const page = {
    on: (name: string, f: (x: any) => void) => handlers.set(name, f),
    off: (name: string) => handlers.delete(name),
    goto: async () => undefined,
    evaluate,
    waitForFunction: async (_fn: unknown, _arg: unknown, options: { timeout: number }) => {
      timeouts.push(options.timeout);
      handlers.get("pageerror")!(new Error("engine panicked"));
      handlers.get("console")!({ type: () => "error", text: () => "wasm trap" });
      handlers.get("requestfailed")!({ url: () => "http://mini/measure/data.bin", failure: () => ({ errorText: "net::ERR_ABORTED" }) });
      throw new Error(`Timeout ${options.timeout}ms exceeded.\n  - waiting for function`);
    },
  };
  return { handlers, session: { baseUrl: "http://mini/", browser: { contexts: () => [{ pages: () => [page] }] } } as any };
}

const seen = ["page error: engine panicked", "console error: wasm trap", "request failed: http://mini/measure/data.bin net::ERR_ABORTED"];

test("the drag page's address carries groups only when the flag is set, and is otherwise the plain address", async () => {
  const addresses: string[] = [];
  const result = { drag: { p50: 1, p95: 2 }, dragResponseMs: { p50: 1, p95: 2 } };
  const page = { on: () => undefined, off: () => undefined, goto: async (url: string) => addresses.push(url), waitForFunction: async () => ({ jsonValue: async () => result }) };
  const session = { baseUrl: "http://mini/", browser: { contexts: () => [{ pages: () => [page] }] } } as any;
  await dragOnce(session, "head");
  await dragOnce(session, "head", 0, 12);
  assert.deepEqual(addresses, ["http://mini/measure/index.html?measure=1&dragMs=4000&repeats=3&editRepeats=0&dragTarget=head", "http://mini/measure/index.html?measure=1&dragMs=4000&repeats=3&editRepeats=0&groups=12&dragTarget=head"]);
});

test("a drag page that gives no result ends dragOnce at DRAG_WAIT_MS with its address, message line, errors and failed requests", async () => {
  assert.ok(DRAG_WAIT_MS <= 120_000);
  // The page as it is mid-run: the status line is hidden and empty, the progress is in the overlay.
  const page = { getElementById: (id: string) => (id === "overlay" ? { textContent: "measure head drag 2/3" } : null), querySelector: () => ({ textContent: "" }) };
  const timeouts: number[] = [];
  const { handlers, session } = stalledSession(async (fn) => {
    const was = (globalThis as any).document;
    (globalThis as any).document = page;
    try {
      return fn();
    } finally {
      (globalThis as any).document = was;
    }
  }, timeouts);
  await assert.rejects(dragOnce(session, "head"), (e: Error) => {
    for (const part of ["the head drag run gave no result", `Timeout ${DRAG_WAIT_MS}ms exceeded.`, "page http://mini/measure/index.html?", "dragTarget=head", 'message line "measure head drag 2/3"', ...seen]) assert.ok(e.message.includes(part), `${part} missing from: ${e.message}`);
    return true;
  });
  assert.deepEqual([timeouts, handlers.size], [[DRAG_WAIT_MS], 0]);
});

test("a drag page whose message line never answers still ends dragOnce, with the line unavailable", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const { session } = stalledSession(() => new Promise<string>(() => undefined));
  const ended = assert.rejects(dragOnce(session, "head"), (e: Error) => {
    for (const part of ['message line "(unavailable)"', ...seen]) assert.ok(e.message.includes(part), `${part} missing from: ${e.message}`);
    return true;
  });
  await new Promise((done) => setImmediate(done));
  t.mock.timers.tick(READ_MS);
  await ended;
});
