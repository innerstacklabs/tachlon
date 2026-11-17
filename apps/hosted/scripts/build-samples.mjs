// @ts-check
// Builds the hosted app's samples (gs#511 S6a, S6b). Run it from anywhere before `pnpm build` in apps/hosted:
//
//   node apps/hosted/scripts/build-samples.mjs
//
// It writes the small office fit-out project as MS Project XML from the table in `samples/office-fitout.ts` (needs
// Node 22.18 or later, which runs that TypeScript file as it is), asks the `xer` crate's `synth` example for a 5,000 and
// a 50,000 activity file, gzips each, and writes them with `samples.json` to `apps/hosted/public/samples/`, which Vite
// copies into `dist/samples/`. Nothing it writes is committed (`public/samples/` is git-ignored). It fails if a gzip
// file reaches the 25 MiB cap that a Pages deploy allows per file (`sample-cap.ts`), naming the file and stopping before
// it builds the next one, and it fails if a file does not hold the activity count it was asked for. Setting
// BUILD_SAMPLES_MAX_BYTES lowers the cap for a test; a value above 25 MiB is an error, not a higher cap. The synth example needs `cargo` on the PATH (and honours CARGO_TARGET_DIR).
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { ROWS, activities as leafRows, assignedTasks, toMspdi } from "../samples/office-fitout.ts";
import { capFor, checkSize } from "./sample-cap.ts";

/** @typedef {import("../src/samples.ts").Sample} Sample */

const root = fileURLToPath(new URL("../../..", import.meta.url));
const out = fileURLToPath(new URL("../public/samples/", import.meta.url));

/** The generated files, in menu order. `seed` is the synth example's seed. */
const GENERATED = [
  { id: "medium-5k", label: "Medium: 5,000 activities (generated XER)", description: "A generated XER file of 5,000 activities", activities: 5000, seed: 1 },
  { id: "large-50k", label: "Large: 50,000 activities (generated XER)", description: "A generated XER file of 50,000 activities", activities: 50000, seed: 1 },
];

/**
 * The notes the `synth` example's XER reports by construction, as the beginnings of the lines (the counts and the file's
 * names after them change with the size): the two tables the reader does not use, the scheduling options, the
 * level-of-effort activities and the percent complete column the file lacks. The line about in-progress dates is
 * not here: it reports a real difference between the file and the engine, so the sample still shows it.
 */
const SYNTH_EXPECTED_NOTES = [
  "Parts of the file that are not read, with their row counts: CURRTYPE",
  "Scheduling options other than the lag calendar were not read",
  "Level of effort activities were imported as zero-day tasks",
  "Percent complete was not read: the file measures it as duration or units percent complete",
];

/** Built into the chart, no file: the schedule `LoadedSchedule.demo` makes, which the screencast names tasks in. */
const BUILT_IN = [
  {
    id: "demo-50k",
    label: "Generated demo: 50,000 tasks",
    description: "A generated schedule of 50,000 tasks; it opens by reloading the page",
    activities: 50000,
  },
];

/**
 * Count the tasks and WBS rows of a synth XER: the `%R` lines under `%T TASK` and `%T PROJWBS`. The page counts rows,
 * and a synth file adds its WBS as summary rows on top of its activities.
 * @param {string} text
 */
function countRows(text) {
  /** @type {Record<string, number>} */
  const counts = { TASK: 0, PROJWBS: 0 };
  /** @type {string | null} */
  let table = null;
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith("%T")) table = line.slice(2).trim();
    else if (line.startsWith("%R") && table !== null && table in counts) counts[table] += 1;
  }
  return { activities: counts.TASK, rows: counts.TASK + counts.PROJWBS };
}

/**
 * The office fit-out's counts, from its table: an activity is a row that is not a summary task, and the page shows
 * every row. The XML the writer made must hold the same number of tasks, or the writer dropped a row.
 * @param {string} xml
 */
function countFitout(xml) {
  const counts = { activities: leafRows().length, rows: ROWS.length };
  const written = xml.split("<Task>").length - 1;
  if (written !== counts.rows) throw new Error(`office-fitout: the table has ${counts.rows} rows, the XML ${written} tasks`);
  return counts;
}

/**
 * Write one synthetic XER with the example and return its bytes. The samples take the construction names (gs#703:
 * "Pour slab, level 3, zone B"); the example's own default stays "Activity N".
 * @param {string} path
 * @param {number} activities
 * @param {number} seed
 */
function synthesize(path, activities, seed) {
  execFileSync(
    process.env.CARGO ?? "cargo",
    ["run", "--release", "--quiet", "-p", "xer", "--features", "synth", "--example", "synth", "--", "--out", path, "--activities", String(activities), "--seed", String(seed), "--names", "site"],
    { cwd: root, stdio: ["ignore", "inherit", "inherit"] },
  );
  return readFileSync(path);
}

function main() {
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  // The type is the check (gs#621): `tsc` (`pnpm typecheck`) fails here when a `format` written below is not a member of
  // `Sample["format"]` in `src/samples.ts`, so the union and this script cannot drift apart unseen.
  /** @type {Sample[]} */
  const samples = [];
  const cap = capFor();

  /**
   * Gzip `bytes` into `<id>.<ext>.gz` and record the sample. A file at or over the cap stops the build here.
   * @param {{ id: string, label: string, description: string }} item
   * @param {Sample["format"]} format
   * @param {string} ext
   * @param {Buffer} bytes
   * @param {{ activities: number, rows: number }} counted
   * @param {string[]} [expectedNotes]
   */
  function pack(item, format, ext, bytes, counted, expectedNotes) {
    const file = `${item.id}.${ext}.gz`;
    const packed = gzipSync(bytes, { level: 9 });
    checkSize(file, packed.length, cap);
    writeFileSync(join(out, file), packed);
    samples.push({
      id: item.id,
      label: item.label,
      description: item.description,
      format,
      file,
      activities: counted.activities,
      rows: counted.rows,
      bytes: packed.length,
      sha256: createHash("sha256").update(packed).digest("hex"),
      ...(expectedNotes === undefined ? {} : { expectedNotes }),
    });
    console.log(`build-samples: ${file} ${packed.length} bytes, ${counted.activities} activities, ${counted.rows} rows`);
  }

  // The reader notes the crews' assignments ("47 tasks have resource assignments; ..."), which the sample knows it reports.
  const xml = toMspdi();
  pack(
    {
      id: "office-fitout",
      label: "Small project: office fit-out, 50 activities",
      description: "A small made-up office fit-out project: 9 phases with 50 activities under them",
    },
    "mspdi", "xml", Buffer.from(xml, "utf8"), countFitout(xml), [`${assignedTasks()} tasks have resource assignments`]);
  for (const item of GENERATED) {
    const raw = join(out, `${item.id}.xer`);
    const bytes = synthesize(raw, item.activities, item.seed);
    rmSync(raw);
    const counted = countRows(bytes.toString("latin1"));
    if (counted.activities !== item.activities) {
      throw new Error(`${item.id}: asked for ${item.activities} activities, the file holds ${counted.activities}`);
    }
    pack(item, "xer", "xer", bytes, counted, SYNTH_EXPECTED_NOTES);
  }
  for (const item of BUILT_IN) {
    samples.push({ id: item.id, label: item.label, description: item.description, format: "demo", file: null, activities: item.activities, rows: item.activities, bytes: 0, sha256: null });
  }
  writeFileSync(join(out, "samples.json"), `${JSON.stringify({ samples }, null, 2)}\n`);
  console.log(`build-samples: wrote ${join(out, "samples.json")} with ${samples.length} samples`);
}

try {
  main();
} catch (err) {
  // A failed build leaves no half-written sample for `vite build` to copy into dist.
  rmSync(out, { recursive: true, force: true });
  console.error(`build-samples: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
}
