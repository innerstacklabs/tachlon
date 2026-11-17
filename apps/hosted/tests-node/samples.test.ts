// The hosted page's sample file and sample picks, no page (moved from tests/sample-baseline.spec.ts and
// tests/sample-race.spec.ts).
import { test } from "node:test";
import { expect } from "@playwright/test";
import { ROWS, toMspdi } from "../samples/office-fitout.ts";
import { type Sample, choose, fetchSample } from "../src/samples.ts";

test("the sample file holds one Baseline 0 for each row, the phases too", () => {
  const xml = toMspdi();
  const count = ROWS.length;
  expect(count, "the sample has rows").toBeGreaterThan(20);
  expect(xml.match(/<Baseline><Number>0<\/Number>/g)?.length ?? 0, "one baseline 0 to a row").toBe(count);
  expect(xml.match(/<Baseline>/g)?.length ?? 0, "no other baseline number").toBe(count);
});

// The next cases run `fetchSample` and `choose` in Node with a stand-in `fetch`, so a call that does not go through `choose` (the way
// `main.ts` calls `fetchSample`) is covered too.
function fakeSample(id: string): Sample {
  return { id, label: id, format: "xer", file: `${id}.xer`, activities: 1, rows: 1, bytes: 1000, sha256: null };
}

/** Replace `fetch` with one that answers each file when its gate opens, and fails with an abort when its signal does. */
function stubFetch(): { open: (id: string) => void; restore: () => void } {
  const gates = new Map<string, () => void>();
  const original = globalThis.fetch;
  globalThis.fetch = ((input: string, init?: { signal?: AbortSignal }) =>
    new Promise<Response>((resolve, reject) => {
      const id = String(input).split("/").pop()!.replace(/\.xer$/, "");
      gates.set(id, () => resolve(new Response(new Uint8Array([1, 2, 3]))));
      init?.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
    })) as typeof fetch;
  return { open: (id) => gates.get(id)?.(), restore: () => void (globalThis.fetch = original) };
}

test("a fetchSample started without choose is cancelled by a later pick, and never returns its file", async () => {
  const net = stubFetch();
  try {
    const older = fetchSample(fakeSample("older"));
    const settled = older.then(
      () => "returned a file",
      (err: unknown) => (err instanceof DOMException ? err.name : String(err)),
    );
    const opened: string[] = [];
    const status = { textContent: "" } as HTMLElement;
    const choosing = choose(fakeSample("newer"), { openFile: async (sample) => void opened.push((await fetchSample(sample)).name), openBuiltIn: () => undefined, openOwnFile: () => undefined }, status);
    net.open("older");
    net.open("newer");
    await choosing;
    expect(await settled).toBe("AbortError");
    expect(opened).toEqual(["newer.xer"]);
  } finally {
    net.restore();
  }
});

test("a fetchSample after an aborted one starts with a live signal", async () => {
  const net = stubFetch();
  try {
    const first = fetchSample(fakeSample("a")).catch((err: unknown) => err);
    const second = fetchSample(fakeSample("b"));
    net.open("a");
    net.open("b");
    expect(await first).toBeInstanceOf(DOMException);
    expect((await second).name).toBe("b.xer");
    const third = fetchSample(fakeSample("c"));
    net.open("c");
    expect((await third).name).toBe("c.xer");
  } finally {
    net.restore();
  }
});

test("an older pick whose open finishes after a newer pick began does not write the address or clear the status", async () => {
  const replaced: string[] = [];
  const saved = (globalThis as { window?: unknown }).window;
  (globalThis as { window?: unknown }).window = {
    location: { href: "http://x.test/" },
    history: { state: null, replaceState: (_s: unknown, _t: string, url: URL) => void replaced.push(url.searchParams.get("sample") ?? "") },
  };
  try {
    let finishOlder: () => void = () => undefined;
    const status = { textContent: "" } as HTMLElement;
    const none = { openBuiltIn: () => undefined, openOwnFile: () => undefined };
    const older = choose(fakeSample("older"), { ...none, openFile: () => new Promise<void>((resolve) => (finishOlder = resolve)) }, status);
    const newer = choose(fakeSample("newer"), { ...none, openFile: () => new Promise<void>(() => undefined) }, status);
    finishOlder();
    await older;
    expect(replaced).toEqual([]);
    expect(status.textContent).toContain("Fetching newer");
    void newer;
  } finally {
    (globalThis as { window?: unknown }).window = saved;
  }
});
