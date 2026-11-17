// The driver's one rule for which bar to drag (`grab.ts`), the same for every side, its scroll
// phases wheeling every side until its probe reads the target (`phases.ts`, #1292), and the slow link a run
// can open the page over (`network.ts`), and the browser launch retried on ETXTBSY (`scripts/lib/launch-retrying.ts`).

import assert from "node:assert/strict";
import { createServer } from "node:http";
import { test } from "node:test";
import { launchRetrying } from "../../scripts/lib/launch-retrying.ts";
import { parseArgs } from "./drive.ts";
import { stopRecorder } from "./frames.ts";
import { GRAB_PX, grabTarget, guardZoom, nextStep, pickBar, QUIET_MS } from "./grab.ts";
import { applyNetwork, brotliProxy, NETWORK_PROFILES, warmUp } from "./network.ts";
import { SCROLL_STEPS, SCROLL_X_VIEWS, SCROLL_Y_VIEWS, scrollX, scrollY, sweepTarget, wheelCap, type DrivenPage } from "./phases.ts";

const view = { x: 600, y: 80, width: 600, height: 700 };
const bar = (x: number, y: number, width: number, id: string) => ({ x, y, width, height: 12, id });

test("the bar is the first fully in view, in row order, wherever the side listed it", () => {
  const bars = [bar(700, 300, 40, "low"), bar(590, 100, 40, "clipped left"), bar(900, 120, 40, "first"), bar(800, 120, 40, "same row, left")];
  assert.equal(pickBar(bars, view)?.id, "same row, left");
  assert.deepEqual(nextStep(bars, view), { kind: "drag", bar: bars[3] });
  assert.equal(pickBar([bar(700, 775, 40, "below the bottom")], view), null);
});

test("a bar narrower than the grab width is zoomed in on, not skipped for a wider one further down", () => {
  // tree4 on our side: the first screen fits the portfolio, so its 2-day leaves are 3 to 5 px wide.
  const narrow = bar(613, 126, 3, "leaf");
  assert.deepEqual(nextStep([narrow, bar(700, 600, 40, "wide")], view), { kind: "zoom", bar: narrow });
  assert.equal(nextStep([bar(613, 126, GRAB_PX, "just wide enough")], view).kind, "drag");
});

test("with no bar in view the wheel brings the first one reported into it, and with none reported there is nothing to drag", () => {
  assert.deepEqual(nextStep([bar(1500, 100, 40, "right"), bar(700, 2000, 40, "below")], view), { kind: "scroll", dx: 880, dy: 0 });
  assert.deepEqual(nextStep([bar(700, 2000, 40, "below")], view), { kind: "scroll", dx: 0, dy: 1900 });
  assert.deepEqual(nextStep([bar(200, 20, 40, "left and above")], view), { kind: "scroll", dx: -420, dy: -80 });
  assert.deepEqual(nextStep([], view), { kind: "none" });
});

/** A fake page whose one bar is `width` px wide (2 days) and widens by `grow` on each ctrl+wheel; what the page was sent. */
function zoomPage(width: number, grow: number) {
  const sent: string[] = [];
  let ctrl = false;
  let w = width;
  const g = globalThis as any;
  const listeners = new Set<unknown>();
  g.window = { addEventListener: (_: string, f: unknown) => listeners.add(f), removeEventListener: (_: string, f: unknown) => listeners.delete(f) };
  const side = {
    name: "fake",
    ready: () => true,
    scrollTarget: () => ({ ...view, top: 0, left: 0, maxTop: 0 }),
    leafBars: () => ({ bars: [{ x: 700 - w / 2, y: 120, width: w, height: 12, id: "4", datasetId: "1" }], pxPerDay: w / 2 }),
    dragState: () => "",
  };
  const page = {
    evaluate: async (fn: () => unknown) => (fn.name === "nextFrame" ? undefined : fn()),
    keyboard: { down: async () => void (ctrl = true), up: async () => void (ctrl = false) },
    mouse: {
      move: async () => undefined,
      wheel: async (_dx: number, dy: number) => {
        sent.push(`${ctrl ? "ctrl+" : ""}wheel ${dy}`);
        if (ctrl) w += grow;
      },
    },
  };
  return { page, side, sent, listeners };
}

test("a narrow bar is zoomed in on until it is wide enough; the steps, the task and pxPerDay are returned, and the page-zoom guard is gone after", async () => {
  const { page, side, sent, listeners } = zoomPage(3, 1);
  const t0 = Date.now();
  const g = await grabTarget(page, side as any, new AbortController().signal);
  assert.deepEqual(sent, ["ctrl+wheel -100", "ctrl+wheel -100", "ctrl+wheel -100"]);
  assert.deepEqual([g.zoomSteps, g.scrollSteps, g.target?.datasetId, g.target?.id, g.target?.pxPerDay, g.why], [3, 0, "1", "4", 3, undefined]);
  assert.ok(Date.now() - t0 >= QUIET_MS, "the bars were watched for QUIET_MS after the last step before the pick");
  assert.equal(listeners.size, 0, "the guard against the browser's page zoom is taken away");
});

test("a side that does not zoom on ctrl+wheel ends the search with why, and the browser's page zoom is stopped meanwhile", async () => {
  const { page, side, sent } = zoomPage(3, 0);
  const g = await grabTarget(page, side as any, new AbortController().signal);
  assert.equal(g.target, null);
  assert.equal(g.why, "ctrl+wheel did not zoom the time axis in: the side does not zoom on it");
  assert.equal(sent.length, 1);
  // The guard: a ctrl+wheel the side left alone is not the browser's to zoom the page with; a plain wheel is untouched.
  guardZoom();
  const f = (globalThis as any).window.benchZoomGuard;
  const ev = (ctrlKey: boolean) => ({ ctrlKey, prevented: false, preventDefault() { this.prevented = true; } });
  const [zoom, scroll] = [ev(true), ev(false)];
  f(zoom);
  f(scroll);
  assert.deepEqual([zoom.prevented, scroll.prevented], [true, false]);
});

/** A page whose view moves `px` per wheel event, whatever its delta (as DHTMLX does with `wheelDelta`), between 0 and `maxTop` down and 0 and `maxLeft` along. */
function steppedPage(px: number | "delta", maxTop: number, maxLeft = 0): DrivenPage {
  let top = 0;
  let left = 0;
  const box = () => ({ x: 0, y: 0, width: 100, height: 100, top, left, maxTop, maxLeft });
  const move = (at: number, d: number, max: number) => Math.min(max, Math.max(0, at + Math.sign(d) * (px === "delta" ? Math.abs(d) : px)));
  return {
    evaluate: async (fn: unknown) => (fn === stopRecorder ? [] : fn === steppedSide.scrollTarget ? box() : undefined),
    mouse: {
      move: async () => undefined,
      wheel: async (dx: number, dy: number) => {
        if (dy !== 0) top = move(top, dy, maxTop);
        if (dx !== 0) left = move(left, dx, maxLeft);
      },
    },
  } as unknown as DrivenPage;
}
const steppedSide = { name: "stepped", ready: () => true, scrollTarget: () => ({}) as any, leafBars: () => ({ bars: [], pxPerDay: 1 }), dragState: () => "" };

test("a side that moves a fixed step per wheel event is wheeled until it reads the target, and back, and the events are counted", async () => {
  // 120 events of 90 px go 10,800 px; this sweep (50 heights of 800 px) needs 443 each way, to within its 200 px tolerance.
  const r = (await scrollY(steppedPage(90, 100_000), steppedSide, 60, 800, new AbortController().signal)) as any;
  assert.equal(r.verified, true, r.why);
  assert.deepEqual([r.stepPx, r.wheelEvents, r.turn, r.back], [(SCROLL_Y_VIEWS * 800) / SCROLL_STEPS, { out: 443, back: 443 }, 39_870, 0]);
});

test("a scroll covers the same distance on a tall schedule and a long one: SCROLL_Y_VIEWS viewport heights, not the schedule's height", async () => {
  const run = async (maxTop: number) => (await scrollY(steppedPage(90, maxTop), steppedSide, 60, 800, new AbortController().signal)) as any;
  const [tall, short] = [await run(2_000_000), await run(60_000)];
  assert.deepEqual([tall.target, short.target], [SCROLL_Y_VIEWS * 800, SCROLL_Y_VIEWS * 800]);
  assert.deepEqual([tall.wheelEvents, tall.turn, tall.verified], [short.wheelEvents, short.turn, short.verified], "the same events and the same place");
  assert.equal(tall.verified, true, tall.why);
});

test("a short schedule's target is its own range, a long one's the fixed distance, and the distance scrolled is recorded", async () => {
  const run = async (maxTop: number) => (await scrollY(steppedPage(90, maxTop), steppedSide, 60, 800, new AbortController().signal)) as any;
  const [short, long] = [await run(10_000), await run(60_000)];
  assert.deepEqual([short.target, short.verified, short.why], [10_000, true, undefined], "the range, not 40,000 px it cannot reach");
  assert.deepEqual([long.target, long.verified], [40_000, true]);
  // The distance is what the view went: within the 0.5% tolerance of the target, and never more than the range.
  assert.ok(Math.abs(short.scrolled - 10_000) <= 50 && short.scrolled <= 10_000, `short scrolled ${short.scrolled}`);
  assert.ok(Math.abs(long.scrolled - 40_000) <= 200, `long scrolled ${long.scrolled}`);
  assert.equal(long.scrolled, long.turn - long.from);
  // The same rule along the time axis, 20 widths of 1280 px at most: DHTMLX's tree4:50000 axis scrolls 18,707 px.
  const x = async (maxLeft: number) => (await scrollX(steppedPage(90, 0, maxLeft), steppedSide, 60, 1280, new AbortController().signal)) as any;
  const [narrow, wide] = [await x(18_707), await x(100_000)];
  assert.deepEqual([narrow.target, wide.target], [18_707, SCROLL_X_VIEWS * 1280]);
  assert.deepEqual([narrow.verified, wide.verified], [true, true], `${narrow.why} ${wide.why}`);
  assert.ok(narrow.scrolled <= 18_707 && narrow.scrolled > 18_600, `narrow scrolled ${narrow.scrolled}`);
  // A side with no range at all has nothing to scroll: unverified, not skipped.
  const none = await run(0);
  assert.deepEqual([none.target, none.verified, none.why, none.scrolled], [0, false, "nothing to scroll", 0]);
});

test("the rule's arithmetic: the smaller of the fixed distance and what is left of the range past where the view starts", () => {
  assert.deepEqual([sweepTarget(40_000, 100_000, 0), sweepTarget(40_000, 10_000, 0), sweepTarget(40_000, 10_000, 4_000), sweepTarget(40_000, Infinity, 7), sweepTarget(40_000, 0, 5)], [40_000, 10_000, 6_000, 40_000, 0]);
});

test("the cap on a sweep's events is twice what the smallest step needs: 90 px an event", () => {
  assert.deepEqual([wheelCap(40_000), wheelCap(SCROLL_X_VIEWS * 1280)], [890, 570]);
  // A short range never cuts the cap below what a side that follows the wheel's delta needs: SCROLL_STEPS events each way.
  assert.deepEqual([wheelCap(90), wheelCap(3_000), wheelCap(5_000), wheelCap(0)], Array(4).fill(2 * SCROLL_STEPS));
});

test("a short range is reached in SCROLL_STEPS events by a side that follows the wheel's delta, and verifies", async () => {
  // 3,000 px: the old cap was 2 * ceil(3000 / 90) = 68 events, so this sweep stopped at 68 of its 120 steps.
  const r = (await scrollY(steppedPage("delta", 3_000), steppedSide, 60, 800, new AbortController().signal)) as any;
  assert.deepEqual([r.verified, r.why, r.target, r.wheelEvents], [true, undefined, 3_000, { out: SCROLL_STEPS, back: SCROLL_STEPS }]);
  assert.ok(Math.abs(r.turn - 3_000) <= 15 && r.back === 0, `turn ${r.turn}, back ${r.back}`);
});

test("a side that never reaches the target stops at the cap, unverified", async () => {
  // 1 px an event: the cap's worth out, and back to within the return's 2 px.
  const cap = wheelCap(SCROLL_Y_VIEWS * 800);
  const r = (await scrollY(steppedPage(1, 100_000), steppedSide, 60, 800, new AbortController().signal)) as any;
  assert.deepEqual([r.verified, r.why, r.wheelEvents], [false, `went ${cap} px of 40000`, { out: cap, back: cap - 2 }]);
});

// The slow link (`network.ts`): the option, the profile reaching the CDP call, and the compressing server.
const slowArgs = ["--url", "http://127.0.0.1:1/", "--probe", "ours", "--rows", "1"];

test("--network names a profile; without it the options hold none", () => {
  assert.equal((parseArgs([...slowArgs, "--network", "cable"]) as { network?: string }).network, "cable");
  assert.equal("network" in (parseArgs(slowArgs) as object), false);
  for (const bad of [["--network", "dialup"], ["--network"], ["--network", "cable", "--network", "cable"]]) assert.equal(typeof parseArgs([...slowArgs, ...bad]), "string", bad.join(" "));
  assert.equal(typeof parseArgs(["--url", "file:///x", "--probe", "ours", "--rows", "1", "--network", "cable"]), "string");
});

test("the profile reaches the CDP call, after Network.enable", async () => {
  const sent: unknown[][] = [];
  await applyNetwork({ send: async (...a) => void sent.push(a) }, "cable");
  assert.deepEqual(sent, [["Network.enable"], ["Network.emulateNetworkConditions", { offline: false, ...NETWORK_PROFILES.cable }]]);
});

test("the server brotli-compresses a script once, keeps its Vary and leaves a picture as it is", async () => {
  const hits: string[] = [];
  const body = "x".repeat(1000);
  const origin = createServer((req, res) => {
    hits.push(req.url!);
    const vary = req.url === "/star.js" ? "*" : "Origin";
    const sent = req.url === "/late.js" ? "y".repeat(70_000) : body;
    res.writeHead(200, { "content-type": req.url!.endsWith(".js") ? "text/javascript" : "image/png", "content-length": sent.length, vary }).end(sent);
  });
  await new Promise<void>((r) => origin.listen(0, "127.0.0.1", r));
  const proxy = await brotliProxy(`http://127.0.0.1:${(origin.address() as { port: number }).port}/`);
  try {
    const [js, png, star] = [await fetch(`${proxy.url}a.js`), await fetch(`${proxy.url}b.png`), await fetch(`${proxy.url}star.js`)];
    assert.deepEqual([js.headers.get("content-encoding"), png.headers.get("content-encoding")], ["br", null]);
    assert.deepEqual([js.headers.get("vary"), star.headers.get("vary")], ["Origin, accept-encoding", "*"]);
    assert.equal(await js.text(), body);
    assert.notEqual(js.headers.get("content-length"), "1000");
    proxy.freeze();
    await (await fetch(`${proxy.url}a.js`)).text();
    await Promise.all([fetch(`${proxy.url}late.js`), fetch(`${proxy.url}small.js`)]);
    assert.deepEqual([hits.filter((h) => h === "/a.js").length, proxy.late], [1, ["GET /late.js"]]); // small.js was asked for late too, but it is under the limit
  } finally {
    proxy.close();
    origin.close();
    origin.closeAllConnections();
  }
});

test("the warm-up loads the page, closes its browser, then freezes the server", async () => {
  const log: string[] = [];
  const browser = { newPage: async () => ({ goto: async (u: string) => void log.push(u) }), close: async () => void log.push("close") };
  await warmUp({ freeze: () => void log.push("freeze") } as never, async () => browser, "http://x/");
  assert.deepEqual(log, ["http://x/", "close", "freeze"]);
});

test("the browser launch is retried 3 times on ETXTBSY and on no other error", async () => {
  let calls = 0;
  const busy = () => Object.assign(new Error("spawn ETXTBSY"), { code: "ETXTBSY" });
  assert.equal(await launchRetrying(async () => { if (++calls <= 2) throw busy(); return "up"; }, 1), "up");
  assert.equal(calls, 3, "failed twice, started on the third try");
  calls = 0;
  await assert.rejects(launchRetrying(async () => { calls++; throw busy(); }, 1), /ETXTBSY/);
  assert.equal(calls, 4, "the first try and 3 retries");
  calls = 0;
  await assert.rejects(launchRetrying(async () => { calls++; throw new Error("spawn ENOENT"); }, 1), /ENOENT/);
  assert.equal(calls, 1, "any other error is not retried");
});
