import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import type { TableRow } from "../src/api";
import type { MeasurementError, MeasurementResult } from "../src/measure";
import type { HostInit } from "../src/measure/config";
import { dragCandidates } from "../src/measure/critical";
import { EditSteps } from "../src/measure/edit-phase";
import type { PhaseRec } from "../src/measure/phases";
import { acrossReload, expect, test } from "./support/gantt";
import * as knobs from "./support/knobs";

test("scripted measurement smoke", async ({ page }, testInfo) => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const query = new URLSearchParams({
    measure: "1",
    n: String(Math.min(knobs.n, 5000)),
    repeats: "1",
    warmup: "0",
    dragMs: "500",
    panMs: "500",
    editRepeats: "2",
    machine: "playwright-ci",
  });
  if (knobs.backend === "webgl") query.set("backend", "webgl");
  await page.goto(`${knobs.url}?${query}`);

  let measured: MeasurementResult | MeasurementError | null = null;
  try {
    const handle = await acrossReload(page, () =>
      page.waitForFunction(() => window.gantt?.measurement ?? null, null, { timeout: 220_000 }),
    );
    measured = await handle.jsonValue();
  } catch {
    // The soft completion check below preserves the verifier's failure semantics.
  }

  expect.soft(measured !== null, "the scripted measurement completes").toBe(true);
  if (measured === null) return;

  await testInfo.attach("measure.json", {
    body: JSON.stringify(measured, null, 2),
    contentType: "application/json",
  });

  const result = measured;
  const measurementError = "error" in result ? result.error : undefined;
  expect
    .soft(
      measurementError === undefined,
      `the scripted measurement ran without an error${measurementError ? `: ${measurementError}` : ""}`,
    )
    .toBe(true);

  const get = (path: string): unknown =>
    path.split(".").reduce<unknown>((value, key) => {
      if (typeof value !== "object" || value === null) return undefined;
      return (value as Record<string, unknown>)[key];
    }, result);
  const phase = (name: string): string[] =>
    ["count", "p50", "p95", "missed", "missedPct", "encodeMs.p50", "encodeMs.p95", "totalMs", "spanMs"].map(
      (field) => `${name}.${field}`,
    );
  const fields = [
    "environment.machine",
    "environment.gpu.webglRenderer",
    "environment.gpu.webgpuAdapter",
    "environment.browser",
    "environment.backend",
    "environment.hardwareAcceleration",
    "environment.viewport.canvasCssWidth",
    "environment.viewport.canvasCssHeight",
    "environment.devicePixelRatio",
    "environment.refreshRateHz",
    "environment.generatorSeed",
    "environment.releaseBuild",
    ...phase("zoom"),
    ...phase("pan"),
    ...phase("drag"),
    "dragResponseMs.p50",
    "dragResponseMs.p95",
    "recomputeMs.p50",
    "recomputeMs.p95",
    "encodeMs.p50",
    "encodeMs.p95",
    "wasm.gzipBytes",
    "wasm.memoryBytes",
    "jsHeapBytes",
    "zoomAnchor.maxDriftPx",
  ];
  const absent = fields.filter((field) => {
    const value = get(field);
    return value === undefined || value === null || (typeof value === "number" && Number.isNaN(value));
  });
  expect
    .soft(
      absent.length === 0,
      `the measurement result has all ${fields.length} fields${absent.length ? `; missing: ${absent.join(", ")}` : ""}`,
    )
    .toBe(true);
  expect.soft(typeof get("wasm.gzipBytes") === "number", "the gzip WASM size is a number").toBe(true);

  const positiveNumber = (value: unknown): boolean => typeof value === "number" && value > 0;
  expect
    .soft(
      ["zoom", "pan", "drag"].every((name) => positiveNumber(get(`${name}.count`))) &&
        positiveNumber(get("dragResponseMs.count")) &&
        positiveNumber(get("recomputeMs.count")),
      "the measurement recorded zoom, pan and drag frames, drag responses and recomputes",
    )
    .toBe(true);

  const phases = ["zoom", "pan", "drag"];
  const phaseSpansMatch = phases.every((name) => {
    const total = get(`${name}.totalMs`);
    const span = get(`${name}.spanMs`);
    return typeof total === "number" && typeof span === "number" && Math.abs(total - span) < 0.01;
  });
  const fixed = (value: unknown): string | undefined =>
    typeof value === "number" ? value.toFixed(2) : undefined;
  expect
    .soft(
      phaseSpansMatch,
      `the recorded frame intervals of each phase sum to its span (${phases
        .map((name) => `${name} ${fixed(get(`${name}.totalMs`))} of ${fixed(get(`${name}.spanMs`))} ms`)
        .join(", ")})`,
    )
    .toBe(true);

  expect
    .soft(
      get("dragResponseMs.count") === get("drag.count"),
      `one drag response per drag frame (${get("dragResponseMs.count")} responses, ${get("drag.count")} frames)`,
    )
    .toBe(true);
  expect
    .soft(
      ["on", "off", "unknown"].includes(String(get("environment.hardwareAcceleration"))),
      `hardware acceleration is on, off or unknown (${get("environment.hardwareAcceleration")})`,
    )
    .toBe(true);
  expect
    .soft(
      "verdicts" in result &&
        result.verdicts.length === 7 &&
        result.verdicts.every((verdict) => verdict.verdict === "PASS" || verdict.verdict === "MISS"),
      "the measurement judges all 7 thresholds PASS or MISS",
    )
    .toBe(true);

  // The edit phase: the script's own report of what it edited and how many rows moved. A phase that
  // skipped `App.set_duration` records frames but reads 0 moved, so the frame counts alone do not pass it.
  const edit = "edit" in result ? result.edit : null;
  expect.soft(edit !== null && edit !== undefined, "the result has an edit phase").toBe(true);
  if (edit) {
    expect
      .soft(
        positiveNumber(edit.slide?.count) && positiveNumber(edit.afterEdit.count) && positiveNumber(edit.noEdit.count),
        `the edit phase recorded frames in the slide window (${edit.slide?.count}), the window after it (${edit.afterEdit.count}) and with no edit (${edit.noEdit.count})`,
      )
      .toBe(true);
    // The frame draws the slide (gs#487): each recorded edit starts one, so the summary has one copy time per edit.
    expect
      .soft(
        edit.copyMs?.count === edit.movedPerRepeat.length &&
          [edit.copyMs?.p50, edit.copyMs?.p95].every((ms) => typeof ms === "number" && Number.isFinite(ms) && ms >= 0),
        `copyMs has one finite value per edit (${JSON.stringify(edit.copyMs)} for ${edit.movedPerRepeat.length} edits)`,
      )
      .toBe(true);
    expect
      .soft(
        edit.movedPerRepeat.length === 2 && edit.movedPerRepeat.every(positiveNumber) && edit.moved > 0,
        `every recorded edit moved rows (moved per edit: ${edit.movedPerRepeat.join(", ")}; task ${edit.task})`,
      )
      .toBe(true);
    expect
      .soft(
        edit.candidates.some((c) => c.task === edit.task && c.moved >= edit.moved),
        "the chosen task is one of the candidates the script tried",
      )
      .toBe(true);
    console.log(`  edit: task ${edit.task} moved ${edit.moved} rows; ${edit.candidates.length} candidates`);
  }

  // The link-edit phase (gs#725): a link added by the pointer path, removed, undone and redone. As above, `moved` is the
  // script's own count, so a phase whose drop added nothing reads 0 even though it recorded frames.
  const link = "linkEdit" in result ? result.linkEdit : undefined;
  expect.soft(link !== null && link !== undefined, "the result has a link-edit phase").toBe(true);
  if (link) {
    const windows = [link.slide, link.afterEdit, link.remove.slide, link.remove.afterEdit, link.undoRedo.slide, link.undoRedo.afterEdit, link.noEdit];
    expect
      .soft(
        windows.every((w) => positiveNumber(w.count)),
        `the link-edit phase recorded frames in all seven windows (${windows.map((w) => w.count).join(", ")})`,
      )
      .toBe(true);
    expect
      .soft(
        link.copyMs?.count === link.movedPerRepeat.length &&
          [link.copyMs?.p50, link.copyMs?.p95].every((ms) => typeof ms === "number" && Number.isFinite(ms) && ms >= 0),
        `copyMs has one finite value per link edit (${JSON.stringify(link.copyMs)} for ${link.movedPerRepeat.length} edits)`,
      )
      .toBe(true);
    expect
      .soft(
        link.setMs.count === 2 && link.movedPerRepeat.length === 2 && link.movedPerRepeat.every(positiveNumber) && link.moved > 0,
        `every recorded link edit moved rows (moved per edit: ${link.movedPerRepeat.join(", ")}; ${link.pred} -> ${link.task})`,
      )
      .toBe(true);
    expect
      .soft(
        link.candidates.some((c) => c.pred === link.pred && c.task === link.task && c.moved >= link.moved),
        "the chosen pair is one of the candidates the script tried",
      )
      .toBe(true);
    console.log(`  link edit: ${link.pred} -> ${link.task} moved ${link.moved} rows; ${link.candidates.length} candidates, ${link.refused} refused`);
  }

  // The select and Fit phase (gs#309): 400 ms of frames after selecting a task with a long driving chain, and 400 ms after Fit.
  const select = "select" in result ? result.select : undefined;
  const fit = "fit" in result ? result.fit : undefined;
  expect.soft(select !== null && select !== undefined, "the result has a select block").toBe(true);
  expect.soft(fit !== null && fit !== undefined, "the result has a fit block").toBe(true);
  if (select) {
    expect.soft(positiveNumber(select.count), `the select window recorded frames (${select.count})`).toBe(true);
    expect.soft(positiveNumber(select.back), `the chosen task has a driving chain (${select.back} behind task ${select.task})`).toBe(true);
    expect.soft(select.candidates.some((c) => c.task === select.task && c.back >= select.back), "the chosen task is one of the candidates").toBe(true);
  }
  if (fit) {
    expect.soft(positiveNumber(fit.count), `the Fit window recorded frames (${fit.count})`).toBe(true);
    expect
      .soft(
        fit.pxPerMinuteBefore.length === 2 && fit.pxPerMinuteBefore.every((b, i) => b !== fit.pxPerMinuteAfter[i]),
        `Fit changed the scale in every repeat (${fit.pxPerMinuteBefore.join(", ")} -> ${fit.pxPerMinuteAfter.join(", ")})`,
      )
      .toBe(true);
  }

  // The open draw-in phase (gs#309): 400 ms of frames after `App.open` of the loaded schedule.
  const drawIn = "drawIn" in result ? result.drawIn : undefined;
  expect.soft(drawIn !== null && drawIn !== undefined, "the result has a drawIn block").toBe(true);
  if (drawIn) expect.soft(positiveNumber(drawIn.count), `the draw-in window recorded frames (${drawIn.count})`).toBe(true);

  const verdict = "verdict" in result ? result.verdict : undefined;
  const warnings = "warnings" in result ? result.warnings : [];
  console.log(
    `  measurement: ${verdict} on ${get("environment.backend")}; warnings: ${warnings.join(" | ") || "none"}`,
  );
});

/** Frames the stub's slide reports active after the undo. */
const STUB_ACTIVE_READS = 4;

/** What the stub host remembers between calls. */
interface StubState {
  edited: boolean;
  /** Reads of `status().slide` that still report active. */
  activeLeft: number;
}

/** A host whose edit moves two of three rows and whose undo starts a slide that reports active for `STUB_ACTIVE_READS` reads. */
function stubHost(state: StubState): HostInit {
  const host = {
    app: {
      set_duration: () => {
        state.edited = true;
      },
      undo: () => {
        const had = state.edited;
        state.edited = false;
        state.activeLeft = STUB_ACTIVE_READS;
        return had;
      },
      row_start: (row: number) => (state.edited && row > 0 ? 1000 + row : row),
      table_rows: () => JSON.stringify([1, 2, 3].map((id) => [id, 0, 0, 0, 0, 0])),
    },
    status: () => ({
      n: 3,
      get slide() {
        const active = state.activeLeft > 0;
        if (active) state.activeLeft--;
        return { active, copyMs: 0.5 };
      },
    }),
  };
  return host as unknown as HostInit;
}

/** The edit phase on the stub host; it records how many active reads were left when the quiet window started. */
class StubEdit extends EditSteps {
  progress = "";
  /** Null if the quiet rec never started. */
  activeLeftAtQuietStart: number | null = null;

  constructor(private readonly state: StubState) {
    super(stubHost(state));
  }

  protected override startPhase(rec: PhaseRec | null): void {
    if (rec?.name === "quiet") this.activeLeftAtQuietStart = this.state.activeLeft;
    super.startPhase(rec);
  }

  /** Run one recorded edit to the end; real time, about 1.2 s of windows. */
  runOnce(): void {
    const run = this.editRun({ warmup: 0, editRepeats: 1 } as Parameters<EditSteps["editRun"]>[0]);
    while (!run.next().done);
  }

  result() {
    return this.editResult(16.7);
  }
}

// The plan's break for this story: remove the wait after the undo. Before the slide exists (gs#485)
// the smoke run above never sees it active, so this is the test that fails without the call.
test("the edit phase starts the quiet window only after the undo's slide has ended", () => {
  const steps = new StubEdit({ edited: false, activeLeft: 0 });
  steps.runOnce();
  expect(steps.activeLeftAtQuietStart, "the quiet window started while the slide was still active").toBe(0);
  const result = steps.result();
  expect(result?.copyMs?.count, "one copy time per recorded edit").toBe(1);
});

test("baseline=1 sets a baseline before the first phase and the result records it", async ({ page }) => {
  const run = async (baseline: boolean) => {
    const query = new URLSearchParams({
      measure: "1",
      n: "500",
      repeats: "1",
      warmup: "0",
      dragMs: "300",
      panMs: "300",
      editRepeats: "0",
      machine: "playwright-ci",
    });
    if (baseline) query.set("baseline", "1");
    if (knobs.backend === "webgl") query.set("backend", "webgl");
    await page.goto(`${knobs.url}?${query}`);
    const handle = await acrossReload(page, () =>
      page.waitForFunction(() => window.gantt?.measurement ?? null, null, { timeout: 220_000 }),
    );
    const result = (await handle.jsonValue()) as MeasurementResult | MeasurementError;
    expect("error" in result ? result.error : undefined, "the run finished without an error").toBeUndefined();
    // The run makes no edit that clears the baseline, so rows still held at the end were set before its first phase.
    const rows = await page.evaluate(() => window.gantt!.app.baseline_rows());
    return { config: (result as MeasurementResult).config, rows };
  };

  const without = await run(false);
  expect(without.config.baseline, "no option: config.baseline is 0").toBe(0);
  expect(without.rows, "no option: no baseline").toBe(0);

  const withBaseline = await run(true);
  expect(withBaseline.config.baseline, "baseline=1 is echoed in the result's config").toBe(1);
  expect(withBaseline.rows, "baseline=1: the app holds a baseline").toBeGreaterThan(0);
});

test("dragTarget=critical offers the earliest critical leaf only, and head every leaf", () => {
  const row = (critical: boolean, kind: 0 | 1 | 2): TableRow => [0, "", "", null, critical, kind, null, null, null, null];
  const rows = [row(false, 0), row(true, 1), row(true, 0), row(true, 0), row(false, 0)];
  const start = (r: number) => [0, 0, 50, 10, 0][r] ?? 0;
  expect(dragCandidates(rows, "critical", start), "the critical leaf that starts first, not a critical summary").toEqual([3]);
  expect(dragCandidates(rows, "head", start), "every leaf").toEqual([0, 2, 3, 4]);
});

test("dragTarget=critical drags a critical task and the project finish moves", async ({ page }) => {
  const query = new URLSearchParams({ measure: "1", n: "500", repeats: "1", warmup: "0", dragMs: "300", panMs: "300", editRepeats: "0", dragTarget: "critical", machine: "playwright-ci" });
  if (knobs.backend === "webgl") query.set("backend", "webgl");
  await page.goto(`${knobs.url}?${query}`);
  const handle = await acrossReload(page, () => page.waitForFunction(() => window.gantt?.measurement ?? null, null, { timeout: 220_000 }));
  const result = (await handle.jsonValue()) as MeasurementResult | MeasurementError;
  expect("error" in result ? result.error : undefined, "the trial drag moved the project finish").toBeUndefined();
  expect((result as MeasurementResult).config.dragTarget, "dragTarget=critical is echoed in the result's config").toBe("critical");
  expect((result as MeasurementResult).recomputeMs.count, "the drag recorded recomputes").toBeGreaterThan(0);
});

test("?measure=1&schedule=1 runs the frame phases on the served file and records its hash", async ({ page }) => {
  const body = readFileSync(new URL("./fixtures/measure-chain.xml", import.meta.url));
  await page.route("**/schedule.dat", (route) => route.fulfill({ status: 200, contentType: "application/xml", body }));
  const query = new URLSearchParams({
    measure: "1",
    schedule: "1",
    repeats: "1",
    warmup: "0",
    dragMs: "500",
    panMs: "300",
    editRepeats: "1",
    machine: "playwright-ci",
  });
  if (knobs.backend === "webgl") query.set("backend", "webgl");
  await page.goto(`${knobs.url}?${query}`);
  // A page that refuses the query says why in its overlay and never measures; read that instead of waiting out the timeout.
  const handle = await acrossReload(page, () =>
    page.waitForFunction(
      () => {
        const refusal = document.getElementById("overlay")?.textContent ?? "";
        return window.gantt?.measurement ?? (refusal.startsWith("could not") ? { error: refusal } : null);
      },
      null,
      { timeout: 220_000 },
    ),
  );
  const result = (await handle.jsonValue()) as MeasurementResult | MeasurementError;
  expect("error" in result ? result.error : undefined, "the run finished without an error").toBeUndefined();
  const { environment, drag } = result as MeasurementResult;
  expect(environment.fixture, "the result names the file as its fixture").toBe("file");
  expect(environment.scheduleSha256, "the result carries the file's hash").toBe(
    createHash("sha256").update(body).digest("hex"),
  );
  expect(environment.tasks, "the 11 tasks of the file, not the stress fixture").toBe(11);
  expect(drag.count, "the drag phase recorded frames").toBeGreaterThan(0);
  expect((result as MeasurementResult).dragResponseMs.count, "the drag phase recorded responses").toBeGreaterThan(0);
});
