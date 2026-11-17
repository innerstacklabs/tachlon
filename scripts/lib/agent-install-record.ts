// What `agent-install-check.ts --record <dir>` keeps for the install video (gantt#1560, build row B7): for each assistant the stream (JSON lines), the
// page screenshot (written by the mount probe), the timings, the results, and a story file of one narration line per cue, which `scripts/narrate.ts`
// reads. Record every assistant into the same folder: a story says the install "works the same in others" only when another assistant's results there passed.
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { PRODUCT_NAME } from "../../packages/gantt/src/product.ts";
import type { Result } from "./agent-install-check.ts";
import type { Assistant, Probes } from "./agent-install-run.ts";

const LABELS: Record<string, string> = { claude: "Claude Code", codex: "Codex" };
type Run = { assistant: string; work: string; results: Result[]; timings: Record<string, number> };

/** The assistant's arguments, asking for its stream as JSON lines on standard output. */
export const streaming = (name: string, a: Assistant): Assistant =>
  ({ ...a, args: name === "codex" ? [a.args[0]!, "--json", ...a.args.slice(1)] : [...a.args, "--output-format", "stream-json", "--verbose"] });

/** The probes, each one's seconds written to `into[<probe>]` (`into.run`, the whole run's, is the caller's). */
export const timed = (probes: Probes, into: Record<string, number>): Probes => Object.fromEntries(Object.entries(probes).map(([k, f]) =>
  [k, async (...a: unknown[]) => { const t = Date.now(); try { return await (f as (...x: unknown[]) => unknown)(...a); } finally { into[k] = (Date.now() - t) / 1000; } }])) as Probes;

const passed = (results: Result[]) => results.length > 0 && results.every((r) => r.ok);
const read = (dir: string, file: string): Run | undefined => {
  try {
    const run = JSON.parse(readFileSync(join(dir, file), "utf8")) as Run;
    return Array.isArray(run.results) ? run : undefined;
  } catch {
    return undefined;
  }
};

/** The narration, a line for each cue. The product's name is read from `product.ts`, never written here. */
export function story(name: string, results: Result[], seconds: number, othersPassed: boolean): string {
  const label = LABELS[name] ?? name;
  return `${[
    `Install ${PRODUCT_NAME} with a coding assistant.`,
    `One prompt from the README goes to ${label}, in a starter app.`,
    `${label} installs the package, then ${results.length} checks judge the app.`,
    `${results.filter((r) => r.ok).length} of ${results.length} checks pass, and the run took ${Math.round(seconds)} seconds.`,
    ...(passed(results) && othersPassed ? ["The install works the same in others."] : []),
  ].join("\n")}\n`;
}

/** Keeps `run` in `dir`, then writes each recorded assistant's story, so recording order does not matter. */
export function record(dir: string, run: Run): void {
  mkdirSync(dir, { recursive: true });
  const stream = readFileSync(join(run.work, "assistant.log"), "utf8").split("\n").filter((l) => l.startsWith("{"));
  writeFileSync(join(dir, `stream-${run.assistant}.jsonl`), stream.map((l) => `${l}\n`).join(""));
  writeFileSync(join(dir, `results-${run.assistant}.json`), `${JSON.stringify(run, null, 2)}\n`);
  const runs = readdirSync(dir).flatMap((f) => (/^results-.+\.json$/.test(f) ? [read(dir, f)] : [])).filter((r): r is Run => r !== undefined);
  for (const r of runs) {
    const others = runs.filter((o) => o.assistant !== r.assistant);
    writeFileSync(join(dir, `story-${r.assistant}.txt`), story(r.assistant, r.results, r.timings?.run ?? 0, others.length > 0 && others.every((o) => passed(o.results))));
  }
}
