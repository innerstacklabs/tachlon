import { expect, test } from "@playwright/test";
import { frameStats, judge, percentile, type FrameStats, type Summary } from "../src/metrics";

// The rules that decide whether a measured run passes (docs/MEASURING.md): nearest-rank percentiles, the missed-frame cut at
// 1.5 P, and the PASS or MISS limits. Pure functions, so the spec runs in Node with no page.

test("percentile is the nearest rank, and NaN for an empty list", () => {
  const four = [1, 2, 3, 4];
  expect(percentile(four, 50)).toBe(2);
  expect(percentile(four, 51)).toBe(3);
  expect(percentile(four, 100)).toBe(4);
  expect(percentile(four, 0)).toBe(1);
  expect(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 95)).toBe(10);
  expect(percentile([], 95)).toBeNaN();
});

test("a missed frame is an interval over 1.5 periods", () => {
  const stats = frameStats([10, 15, 15.1, 30], 10);
  expect(stats.count).toBe(4);
  expect(stats.missed).toBe(2);
  expect(stats.missedPct).toBe(50);
  expect(frameStats([], 10).missedPct).toBeNaN();
});

const P = 16;
const frame = (p95: number, missedPct: number): FrameStats => ({ count: 100, p50: 1, p95, missed: missedPct, missedPct });
interface Run {
  zoom?: FrameStats;
  pan?: FrameStats;
  drag?: FrameStats;
  response?: number;
  drift?: number;
}
const verdicts = (o: Run): string[] => {
  const response: Summary = { count: 100, p50: 1, p95: o.response ?? 0 };
  return judge(P, o.zoom ?? frame(0, 0), o.pan ?? frame(0, 0), o.drag ?? frame(0, 0), response, o.drift ?? 0).map((c) => c.verdict);
};
const misses = (v: string[]): number => v.filter((x) => x === "MISS").length;

test("a run inside every limit passes all seven checks", () => {
  expect(verdicts({})).toEqual(Array(7).fill("PASS"));
});

// [check, run with the value, last passing value, first missing value]: a p95 limit is at or under (1.25 P = 20 ms here), the others are under.
const edges: [string, (v: number) => Run, number, number][] = [
  ["zoom p95 at most 1.25 P", (v) => ({ zoom: frame(v, 0) }), 20, 20.01],
  ["pan p95 at most 1.25 P", (v) => ({ pan: frame(v, 0) }), 20, 20.01],
  ["zoom missed under 1%", (v) => ({ zoom: frame(0, v) }), 0.99, 1],
  ["pan missed under 1%", (v) => ({ pan: frame(0, v) }), 0.99, 1],
  ["drag response p95 at most 50 ms", (v) => ({ response: v }), 50, 50.01],
  ["drag missed under 5%", (v) => ({ drag: frame(0, v) }), 4.99, 5],
  ["anchor drift under 1 px", (v) => ({ drift: v }), 0.99, 1],
];
for (const [name, run, pass, miss] of edges) {
  test(`judge: ${name}`, () => {
    expect(misses(verdicts(run(pass)))).toBe(0);
    expect(misses(verdicts(run(miss)))).toBe(1);
  });
}

test("judge: a value that is not a number is a MISS", () => {
  expect(misses(verdicts({ zoom: frame(Number.NaN, Number.NaN), drift: Number.NaN }))).toBe(3);
});
