// Turns the DHTMLX comparison harness's generated schedule into an MS Project XML file the chart opens.
//
//   node demos/dhtmlx-compare/to-mspdi.ts <dataset.json> <out.xml>
//
// <dataset.json> is the harness's `buildDataset(size, shape)` result as JSON: `{ data, links }`, each
// task `{ id, text, duration, parent? }` (duration in days) and each link `{ id, source, target,
// type }` (type "0" finish-to-start, "1" start-to-start, "2" finish-to-finish, "3" start-to-finish).
// Dates are kept: the dataset's `start` (YYYY-MM-DD) is the file's `StartDate`, and a leaf's `start`
// (YYYY-MM-DD, read as 08:00, or YYYY-MM-DDTHH:MM) becomes a start-no-earlier-than constraint at
// that time, the form the chart reads; with no `start` the file starts on DEFAULT_START and the task
// has no constraint. A summary with a `start` is an error. A duration must be a number of days, 0 or
// more; a leaf of 0 days is written as a milestone.
// Every record becomes one task, summaries included, so the chart counts what the harness counts, and
// every link becomes one predecessor link. The file is a pure function of the dataset: the same
// records give the same bytes. The hash of the file is written to <out.xml>.sha256 in `sha256sum`
// format. The large files are generated at run time and never committed.
// Exit code: 0 done; 1 the dataset is inconsistent (the message says how); 64 usage.

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import { fileURLToPath } from "node:url";

export interface DatasetTask {
  id: number | string;
  text: string;
  /** Working days. */
  duration: number;
  parent?: number | string;
  /** YYYY-MM-DD (08:00) or YYYY-MM-DDTHH:MM: the task starts no earlier than this. Leaves only. */
  start?: string;
  /** The host's key for the task: only the bench's own side reads it. */
  key?: string;
}

export interface DatasetLink {
  id: number | string;
  source: number | string;
  target: number | string;
  type: string;
}

export interface Dataset {
  data: DatasetTask[];
  links: DatasetLink[];
  /** YYYY-MM-DD: the project start. */
  start?: string;
}

export const EXIT_BAD_DATASET = 1;
export const EXIT_USAGE = 64;

const HOURS_PER_DAY = 8;
/** The project start when the dataset names none (a Monday). */
export const DEFAULT_START = "2026-01-05";
/** The start of the working day in the file's calendar. */
const DAY_START = "T08:00";

/** YYYY-MM-DD (as 08:00) or YYYY-MM-DDTHH:MM, as the file's YYYY-MM-DDTHH:MM:00. */
export function stamp(date: string, what: string): string {
  const full = /^\d{4}-\d{2}-\d{2}$/.test(date) ? `${date}${DAY_START}` : date;
  const ok = /^(\d{4}-\d{2}-\d{2})T([01]\d|2[0-3]):[0-5]\d$/.exec(full);
  if (!ok || new Date(`${ok[1]}T00:00:00Z`).toISOString().slice(0, 10) !== ok[1]) {
    throw new Error(`${what} has start ${JSON.stringify(date)}, not a YYYY-MM-DD date or YYYY-MM-DDTHH:MM time`);
  }
  return `${full}:00`;
}
/** The dataset's link type to MS Project's `Type` (0 FF, 1 FS, 2 SF, 3 SS). */
const LINK_TYPE: Record<string, number> = { "0": 1, "1": 3, "2": 0, "3": 2 };

const WORK_DAY = `<WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes>`;

function calendar(): string {
  const days = [1, 2, 3, 4, 5, 6, 7]
    .map((d) => `<WeekDay><DayType>${d}</DayType><DayWorking>${d === 1 || d === 7 ? 0 : 1}</DayWorking>${d === 1 || d === 7 ? "" : WORK_DAY}</WeekDay>`)
    .join("");
  return `<Calendars><Calendar><UID>1</UID><Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID><WeekDays>${days}</WeekDays></Calendar></Calendars>`;
}

function escapeXml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** The tasks in outline order (a parent, then its children) with their outline levels: the XML's order, UID 1 first. */
export function outlineOrder(dataset: Dataset): { task: DatasetTask; level: number }[] {
  const byId = new Map<string, DatasetTask>();
  for (const t of dataset.data) {
    if (byId.has(String(t.id))) throw new Error(`duplicate task ${t.id}`);
    byId.set(String(t.id), t);
  }
  const children = new Map<string, DatasetTask[]>();
  const roots: DatasetTask[] = [];
  for (const t of dataset.data) {
    if (t.parent === undefined || t.parent === 0 || t.parent === "0") {
      roots.push(t);
      continue;
    }
    const key = String(t.parent);
    if (!byId.has(key)) throw new Error(`task ${t.id} has unknown parent ${t.parent}`);
    const list = children.get(key) ?? [];
    list.push(t);
    children.set(key, list);
  }

  const order: { task: DatasetTask; level: number }[] = [];
  const stack = roots.map((task) => ({ task, level: 1 })).reverse();
  while (stack.length > 0) {
    const next = stack.pop()!;
    order.push(next);
    const kids = children.get(String(next.task.id)) ?? [];
    for (let i = kids.length - 1; i >= 0; i--) stack.push({ task: kids[i], level: next.level + 1 });
  }
  if (order.length !== dataset.data.length) throw new Error("the parent links form a cycle");
  return order;
}

/** The MS Project XML for `dataset`: tasks in outline order (a parent, then its children), UIDs from 1. */
export function toMspdi(dataset: Dataset): string {
  const order = outlineOrder(dataset);
  const parents = new Set(dataset.data.map((t) => String(t.parent ?? "")));
  const uid = new Map<string, number>();
  order.forEach(({ task }, i) => uid.set(String(task.id), i + 1));
  const into = new Map<string, string[]>();
  for (const l of dataset.links) {
    const from = uid.get(String(l.source));
    if (from === undefined) throw new Error(`link ${l.id} names unknown task ${l.source}`);
    if (!uid.has(String(l.target))) throw new Error(`link ${l.id} names unknown task ${l.target}`);
    const type = LINK_TYPE[String(l.type)];
    if (type === undefined) throw new Error(`link ${l.id} has unknown type ${JSON.stringify(l.type)}`);
    const list = into.get(String(l.target)) ?? [];
    list.push(`<PredecessorLink><PredecessorUID>${from}</PredecessorUID><Type>${type}</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink>`);
    into.set(String(l.target), list);
  }

  const rows = order.map(({ task, level }) => {
    const summary = parents.has(String(task.id));
    const minutes = task.duration * HOURS_PER_DAY * 60;
    if (typeof task.duration !== "number" || !Number.isInteger(minutes) || minutes < 0) {
      throw new Error(`task ${task.id} has duration ${JSON.stringify(task.duration)}, not a whole number of minutes, 0 or more`);
    }
    const m = summary ? 0 : minutes;
    const milestone = !summary && m === 0 ? "<Milestone>1</Milestone>" : "";
    const preds = (into.get(String(task.id)) ?? []).join("");
    if (summary && task.start !== undefined) throw new Error(`summary task ${task.id} has a start; only a leaf can`);
    const snet = task.start === undefined ? "" : `<ConstraintType>4</ConstraintType><ConstraintDate>${stamp(task.start, `task ${task.id}`)}</ConstraintDate>`;
    return `<Task><UID>${uid.get(String(task.id))}</UID><Name>${escapeXml(task.text)}</Name><OutlineLevel>${level}</OutlineLevel><Summary>${summary ? 1 : 0}</Summary>${milestone}<Duration>PT${Math.floor(m / 60)}H${m % 60}M0S</Duration><DurationFormat>7</DurationFormat>${snet}${preds}</Task>`;
  });
  const start = stamp(dataset.start ?? DEFAULT_START, "the dataset");
  return `<?xml version="1.0" encoding="UTF-8"?>\n<Project xmlns="http://schemas.microsoft.com/project"><StartDate>${start}</StartDate><CalendarUID>1</CalendarUID>${calendar()}<Tasks>\n${rows.join("\n")}\n</Tasks></Project>\n`;
}

function main(argv: string[]): number {
  if (argv.length !== 2) {
    console.error("usage: node to-mspdi.ts <dataset.json> <out.xml>");
    return EXIT_USAGE;
  }
  const [input, out] = argv;
  try {
    const dataset = JSON.parse(readFileSync(input, "utf8")) as Dataset;
    const xml = toMspdi(dataset);
    writeFileSync(out, xml);
    const hash = createHash("sha256").update(xml).digest("hex");
    writeFileSync(`${out}.sha256`, `${hash}  ${basename(out)}\n`);
    console.log(`${dataset.data.length} tasks, ${dataset.links.length} links, sha256 ${hash}`);
    return 0;
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    return EXIT_BAD_DATASET;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
