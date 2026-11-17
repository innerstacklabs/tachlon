// Checks the open-time part of measure.mjs (`--open`): the p50 and p95 rule, the shape of the result's `open`
// block, the verdict rule, and the page driver against a stand-in page. It needs no browser and no build.
//   node demos/50k/test-measure-open.mjs
// The smoke run itself (a real page opening a real file) is not in CI: its numbers depend on the machine.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { OPEN_PHASES, OPEN_SPAN, OPEN_TARGET_MS, measureOpens, openArguments, openBlock, openOnce, openVerdict, percentile, summarize } from "./measure.mjs";

let passed = 0;
let failed = 0;
async function check(name, action) {
  try {
    await action();
    passed++;
    console.log(`ok - ${name}`);
  } catch (error) {
    failed++;
    console.log(`not ok - ${name}: ${error?.message ?? error}`);
  }
}

/**
 * One open's timings whose phases add up to `total`, and whose Rust phases add up to `rustSpan` (the span Rust
 * measured outside them): `scale` multiplies the base split.
 */
function timings(scale = 1) {
  const split = { read: 30, paint: 17, copy: 25, parse: 55, build: 20, compute: 8, swap: 3, firstFrame: 12 };
  const phases = Object.fromEntries(Object.entries(split).map(([phase, ms]) => [phase, ms * scale]));
  return {
    ...phases,
    total: Object.values(phases).reduce((sum, ms) => sum + ms, 0),
    [OPEN_SPAN]: (split.parse + split.build + split.compute) * scale,
  };
}

await check("percentile is nearest-rank, as the page's summaries are", () => {
  const five = [10, 20, 30, 40, 50];
  assert.equal(percentile(five, 50), 30);
  assert.equal(percentile(five, 95), 50);
  assert.equal(percentile(five, 0), 10);
  assert.equal(percentile([7], 50), 7);
  assert.equal(percentile([7], 95), 7);
  assert.ok(Number.isNaN(percentile([], 50)));
  const twenty = Array.from({ length: 20 }, (_, i) => i + 1);
  assert.equal(percentile(twenty, 95), 19);
  assert.equal(percentile(twenty, 50), 10);
});

await check("summarize sorts first and counts", () => {
  assert.deepEqual(summarize([50, 10, 40, 20, 30]), { count: 5, p50: 30, p95: 50 });
  assert.deepEqual(summarize([]).count, 0);
});

await check("openBlock: fixed repeats give fixed p50, p95 and per-phase statistics", () => {
  const repeats = [1, 2, 3, 4, 5].map((scale) => timings(scale));
  const block = openBlock({ file: "/f/50k.xer", bytes: 48_000_000, tasks: 50_000, warmup: timings(9), repeats });
  assert.deepEqual(Object.keys(block).sort(), ["bytes", "file", "p50", "p95", "phases", "repeats", "tasks", "warmup"]);
  assert.equal(block.file, "/f/50k.xer");
  assert.equal(block.bytes, 48_000_000);
  assert.equal(block.tasks, 50_000);
  assert.equal(block.repeats.length, 5);
  // total at scale 1 is 170 ms, so p50 is the scale-3 total and p95 the scale-5 total; the warm-up is in neither.
  assert.equal(block.p50, 170 * 3);
  assert.equal(block.p95, 170 * 5);
  assert.deepEqual(Object.keys(block.phases), OPEN_PHASES);
  assert.deepEqual(block.phases.parse, { p50: 55 * 3, p95: 55 * 5 });
  assert.deepEqual(block.phases.total, { p50: 170 * 3, p95: 170 * 5 });
  assert.deepEqual(block.warmup, timings(9));
  assert.deepEqual(JSON.parse(JSON.stringify(block)), block, "the block survives JSON, so the result file keeps it whole");
});

await check("openBlock stops on a missing, negative or non-numeric phase", () => {
  const good = timings();
  const base = { file: "f", bytes: 1, tasks: 1, warmup: good };
  for (const phase of [...OPEN_PHASES, OPEN_SPAN]) {
    const missing = { ...good };
    delete missing[phase];
    assert.throws(() => openBlock({ ...base, repeats: [good, missing] }), new RegExp(`${phase} is`));
  }
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, copy: -1 }] }), /copy is -1/);
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, parse: Number.NaN }] }), /parse is null/);
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, build: "20" }] }), /build is "20"/);
  assert.throws(() => openBlock({ ...base, warmup: { ...good, read: -5 }, repeats: [good] }), /read is -5/);
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, [OPEN_SPAN]: Number.NaN }] }), /rustSpan is null/);
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, [OPEN_SPAN]: -2 }] }), /rustSpan is -2/);
  assert.throws(() => openBlock({ ...base, repeats: [] }), /no open was recorded/);
});

await check("openBlock stops when the phases do not add up to the total (a phase lost or counted twice)", () => {
  const good = timings();
  const base = { file: "f", bytes: 1, tasks: 1, warmup: good };
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, total: good.total - 20 }] }), /counted twice/);
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, total: good.total + 20 }] }), /missing/);
  // The clock's coarseness is not a lost phase.
  assert.doesNotThrow(() => openBlock({ ...base, repeats: [{ ...good, total: good.total + 0.2 }] }));
});

await check("openBlock refuses an invalid phase: a negative or NaN the page carried, never a 0", () => {
  const good = timings();
  const base = { file: "f", bytes: 1, tasks: 1, warmup: good };
  // The page keeps a negative phase (a clock that went backwards) and NaN (a value Rust marked null); neither is 0.
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, parse: -4 }] }), /parse is -4/);
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, compute: Number.NaN }] }), /compute is null/);
  // A negative `copy` is Rust claiming more time than the call took; it is refused, not floored to 0.
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, copy: -0.4, total: good.total - 25.4 }] }), /copy is -0.4/);
});

await check("openBlock rejects an under-reported Rust phase even when the residual absorbs it and the sum balances", () => {
  const good = timings();
  const base = { file: "f", bytes: 1, tasks: 1, warmup: good };
  // The compensating error: parse is reported as 0 (really 55) and `copy` grows by 55, so the phases still add up
  // to `total`. The sum check cannot see it; the Rust span, measured outside the phases, can.
  const wrong = { ...good, parse: 0, copy: good.copy + 55 };
  const sum = OPEN_PHASES.filter((phase) => phase !== "total").reduce((total, phase) => total + wrong[phase], 0);
  assert.equal(sum, wrong.total, "the sum balances, as the finding describes");
  assert.throws(() => openBlock({ ...base, repeats: [wrong] }), /Rust phases .* add up to 28\.00 ms but Rust measured its own span as 83\.00 ms.*under- or over-reported/);
  // The same for compute (8 ms reported as 0 while copy takes the 8), in the warm-up slot.
  const compute = { ...good, compute: 0, copy: good.copy + 8 };
  assert.throws(() => openBlock({ ...base, warmup: compute, repeats: [good] }), /Rust measured its own span/);
  // The cost of the clock calls at the two ends of the span (up to 3 ms) is not a wrong phase; 3.5 ms is.
  assert.doesNotThrow(() => openBlock({ ...base, repeats: [{ ...good, parse: good.parse - 2.5, copy: good.copy + 2.5 }] }));
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, parse: good.parse - 3.5, copy: good.copy + 3.5 }] }), /Rust measured its own span/);
  // The same from the other side: a phase reported too long.
  assert.throws(() => openBlock({ ...base, repeats: [{ ...good, build: good.build + 20, copy: good.copy - 20 }] }), /Rust measured its own span/);
});

await check("openArguments: a trailing --open, or a flag without its value, is an error, not a silent skip", () => {
  assert.equal(openArguments([]).file, undefined);
  assert.deepEqual(openArguments(["--open", "f.xer"]), { file: new URL("f.xer", `file://${process.cwd()}/`).pathname, repeats: 5, project: "all" });
  assert.equal(openArguments(["--open", "f.xer", "--open-repeats", "3", "--open-project", "1001"]).repeats, 3);
  assert.throws(() => openArguments(["--headless", "--open"]), /--open needs a value, got nothing/);
  assert.throws(() => openArguments(["--open", "--headless"]), /--open needs a value, got --headless/);
  assert.throws(() => openArguments(["--open", "f.xer", "--open-repeats"]), /--open-repeats needs a value/);
  assert.throws(() => openArguments(["--open", "f.xer", "--open-project"]), /--open-project needs a value/);
  assert.throws(() => openArguments(["--open", "f.xer", "--open-repeats", "0"]), /whole number of at least 1/);
  assert.throws(() => openArguments(["--open-repeats", "3"]), /need --open <file>/);
});

await check("openVerdict: PASS or MISS only on a release build with hardware acceleration", () => {
  const block = (p95) => ({ p95 });
  const release = { hardwareAcceleration: "on", releaseBuild: true };
  assert.equal(openVerdict(release, block(1999.9)).verdict, "PASS");
  assert.equal(openVerdict(release, block(OPEN_TARGET_MS)).verdict, "PASS");
  assert.equal(openVerdict(release, block(2000.1)).verdict, "MISS");
  const row = openVerdict(release, block(1500));
  assert.deepEqual([row.value, row.limit, row.unit, row.note], [1500, OPEN_TARGET_MS, "ms", null]);
  for (const environment of [
    { hardwareAcceleration: "off", releaseBuild: true },
    { hardwareAcceleration: "unknown", releaseBuild: true },
    { hardwareAcceleration: "on", releaseBuild: false },
    { hardwareAcceleration: "off", releaseBuild: false },
    undefined,
  ]) {
    const smoke = openVerdict(environment, block(100));
    assert.equal(smoke.verdict, null, JSON.stringify(environment));
    assert.match(smoke.note, /no verdict .*: a smoke run, not a result/);
  }
  assert.match(openVerdict({ hardwareAcceleration: "off", releaseBuild: false }, block(1)).note, /hardware acceleration is off, not a release build/);
});

/**
 * A stand-in for the Playwright page: `setInputFiles` makes the stand-in page react as `script` says, and
 * `waitForFunction` returns the state the real in-page function would compute from it.
 */
function standIn(script) {
  let epoch = 7;
  let state = null;
  const log = [];
  const page = {
    log,
    evaluate: async () => epoch,
    setInputFiles: async (selector, file) => {
      log.push(["setInputFiles", selector, file]);
      state = script.shift();
      if (state.opened) {
        epoch += 1;
      }
    },
    waitForFunction: async () => ({ jsonValue: async () => structuredClone(state) }),
    locator: (selector) => ({
      count: async () => (selector.includes('data-project="all"') || selector.includes('data-project="1001"') ? 1 : 0),
      click: async () => {
        log.push(["click", selector]);
        const next = script.shift();
        state = next;
        if (next.opened) epoch += 1;
      },
    }),
  };
  return page;
}

const opened = (scale = 1) => ({ opened: { name: "f.xer", bytes: 123, format: "xer", tasks: 99, timings: timings(scale) } });

await check("measureOpens opens once to warm up, then the asked number of times, and returns the block", async () => {
  const page = standIn([opened(9), opened(1), opened(2), opened(3)]);
  const block = await measureOpens(page, { file: "/f/f.xer", repeats: 3, project: "all" });
  assert.equal(page.log.filter(([kind]) => kind === "setInputFiles").length, 4);
  assert.deepEqual(page.log[0], ["setInputFiles", "#open-input", "/f/f.xer"]);
  assert.equal(block.repeats.length, 3);
  assert.deepEqual(block.warmup, timings(9));
  assert.equal(block.p50, 170 * 2);
  assert.equal(block.bytes, 123);
  assert.equal(block.tasks, 99);
});

await check("openOnce chooses the project on a picker, and says which ids exist when the choice is not there", async () => {
  const page = standIn([{ picker: true }, opened(1)]);
  const done = await openOnce(page, "/f/two.xer", "all");
  assert.equal(done.tasks, 99);
  assert.deepEqual(page.log.at(-1), ["click", '#picker-list button[data-project="all"]']);
  const missing = standIn([{ picker: true }]);
  await assert.rejects(openOnce(missing, "/f/two.xer", "9999"), /none has the id 9999.*--open-project/);
});

await check("openOnce reports the page's own message when the page refuses the file", async () => {
  const page = standIn([{ error: "could not open cut.xer: unexpected end of file" }]);
  await assert.rejects(openOnce(page, "/f/cut.xer", "all"), /could not open .*cut\.xer: could not open cut\.xer: unexpected end of file/);
});

/** The "Open time" section of docs/MEASURING.md: from its heading to the next `###` heading. */
function openTimeSection() {
  const text = readFileSync(new URL("../../docs/MEASURING.md", import.meta.url), "utf8");
  const start = text.indexOf("### Open time");
  assert.notEqual(start, -1, "docs/MEASURING.md has no 'Open time' section");
  const next = text.indexOf("\n### ", start + 1);
  return text.slice(start, next === -1 ? undefined : next);
}

await check("the Open time table in docs/MEASURING.md has a measured 50,000-activity row for an XER and an MS Project file", () => {
  const section = openTimeSection();
  assert.ok(!section.includes("TODO(measure)"), "a cell of the Open time table still says TODO(measure)");
  const rows = section.split("\n").filter((line) => line.startsWith("|") && /\b50,000\b/.test(line));
  const columns = section.split("\n").find((line) => line.startsWith("| Machine"))?.split("|").length;
  assert.ok(columns, "the Open time table has no header row");
  for (const [label, pattern] of [["XER", /XER/], ["MS Project", /MS Project/]]) {
    const row = rows.find((line) => pattern.test(line));
    assert.ok(row, `the Open time table has no 50,000-activity ${label} row`);
    assert.equal(row.split("|").length, columns, `the ${label} row has a different number of cells than the header`);
    assert.ok(row.split("|").slice(1, -1).every((cell) => cell.trim() !== ""), `the ${label} row has an empty cell`);
  }
});

console.log(`${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
