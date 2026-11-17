// `toPlain` (gantt#2077): a document as the plain tasks and links `fromTasks` reads, with the start of each task given by the caller (the
// start the live chart has solved; the document stores limits and actuals, not solved dates). A pure function: no DOM, no wasm, no clock.
import { DAY, LIMIT_KEYS, WORKING_DAY, WORKING_WEEK, plural, type DocumentLimit, type ScheduleOptions } from "./dhtmlx.ts";
import type { PlainLink, PlainTask } from "./plain.ts";

/** The part of a saved document `toPlain` reads. */
interface Saved {
  origin: number; data_date: number | null;
  tasks: { name: string; code: string | null; parent: number | null; duration: number; progress: number; calendar: number; label?: string; color?: string; key?: string; snet: number | null; limit?: DocumentLimit; actual_start: number | null; actual_finish: number | null; remaining: number | null; wbs_code: string | null; activity_codes: unknown[] }[];
  links: { pred: number; succ: number; kind: PlainLink["type"]; lag: number }[];
  calendars: unknown[];
  baseline: unknown;
  notes: string[];
  options?: ScheduleOptions;
  must_finish_by?: number;
}

/** The date text the session writes, `YYYY-MM-DD HH:MM`, as the plain text `YYYY-MM-DDTHH:mm`. */
const plainDate = (text: string): string => text.replace(" ", "T");

/** The limit keys of a plain task, `LIMIT_KEYS` read the other way: the document key, then the plain key. */
const PLAIN_LIMIT_KEYS = Object.entries(LIMIT_KEYS).map(([plain, field]) => [field, plain] as const);

/** `minutes` (from the document's origin day) as the text `YYYY-MM-DDTHH:mm`; a datetime is read as it stands, so a finish limit never takes the end-of-day reading of a date alone. */
const limitDate = (minutes: number, origin: number): string => new Date((minutes + origin * DAY) * 60_000).toISOString().slice(0, 16);

/** The plain limit keys of `task`. `snet` is left out when `fromTasks` would set the same floor from `start` (`start` itself, or the midnight before a start at 08:00 of that day). */
function limitKeys(task: Saved["tasks"][number], origin: number, start: string, unstarted: boolean): Record<string, string | number | true> {
  const keys: Record<string, string | number | true> = {};
  if (task.snet !== null) {
    const floor = limitDate(task.snet, origin);
    if (floor !== start && !(floor.endsWith("T00:00") && start === `${floor.slice(0, 10)}T08:00`)) keys.snet = floor;
  }
  for (const [field, plain] of PLAIN_LIMIT_KEYS) {
    const at = task.limit?.[field];
    if (at !== undefined) keys[plain] = limitDate(at, origin);
  }
  if (task.limit?.alap === true) keys.alap = true;
  if (task.actual_start !== null) { // the other progress keys describe a started task, as the readers take them
    keys.actualStart = limitDate(task.actual_start, origin);
    if (task.actual_finish !== null) keys.actualFinish = limitDate(task.actual_finish, origin);
    if (task.remaining !== null) keys.remaining = task.remaining / WORKING_DAY;
    if (task.limit?.resume !== undefined) keys.resume = limitDate(task.limit.resume, origin);
  } else if (unstarted && task.remaining !== null) keys.remaining = task.remaining / WORKING_DAY; // `validate` takes `remaining` alone only under `options.unstartedRemaining`
  return keys;
}

/** The tasks and links of `document`, `dates[i]` the solved start (`YYYY-MM-DD HH:MM`) of its task `i`, and one line for each thing plain data cannot carry. An id is the task's activity ID, or its place in the list (from 1) when the IDs are not all there and different. */
export function toPlain(document: Saved, dates: readonly string[]): { tasks: PlainTask[]; links: PlainLink[]; leftOut: string[]; dataDate?: string; mustFinishBy?: string; options?: ScheduleOptions } {
  const codes = document.tasks.map((task) => task.code ?? "");
  const named = new Set(codes).size === codes.length && !codes.includes("");
  const ids = document.tasks.map((_, at) => (named ? codes[at]! : String(at + 1)));
  const limits = document.tasks.map((task, at) => limitKeys(task, document.origin, plainDate(dates[at]!), document.options?.unstartedRemaining === "on"));
  const tasks = document.tasks.map((task, at): PlainTask => ({
    id: ids[at]!,
    name: task.name,
    start: plainDate(dates[at]!),
    ...limits[at],
    duration: task.duration / WORKING_DAY,
    ...(task.parent === null ? {} : { parent: ids[task.parent]! }),
    ...("remaining" in limits[at]! ? {} : { progress: task.progress }), // `progress` and `remaining` are not given together
    ...(task.label === undefined ? {} : { label: task.label }),
    ...(task.color === undefined ? {} : { color: task.color }),
    ...(task.key === undefined ? {} : { key: task.key }),
  }));
  const links = document.links.map((link): PlainLink => ({ from: ids[link.pred]!, to: ids[link.succ]!, type: link.kind, lag: link.lag / WORKING_DAY }));
  const count = (test: (task: Saved["tasks"][number]) => boolean): number => document.tasks.filter(test).length;
  const left: [number, string][] = [
    [count((task) => task.actual_start === null && task.actual_finish !== null), "an actual finish with no actual start"],
    [count((task) => task.calendar !== 0), "a calendar of its own"],
    [count((task) => task.wbs_code !== null || task.activity_codes.length > 0), "a WBS or activity code"],
  ];
  const leftOut = left.filter(([n]) => n > 0).map(([n, what]) => `${plural(n, "task has", "tasks have")} ${what}, which plain data cannot carry.`);
  if (document.calendars.length > 1 || (document.calendars.length === 1 && JSON.stringify(document.calendars[0]) !== JSON.stringify({ week: WORKING_WEEK, exceptions: [] }))) leftOut.push("The schedule has a calendar other than the one Monday to Friday week, or more than one; this list of tasks carries the one week, and its durations and lags count 8-hour days.");
  if (document.baseline !== null) leftOut.push("The schedule has a baseline, which plain data cannot carry.");
  if (document.notes.length > 0) leftOut.push(`The schedule has ${plural(document.notes.length, "import note", "import notes")}, which plain data cannot carry.`);
  if (!named) leftOut.push("Not every task has a different activity ID, so each id is the task's place in the list, from 1.");
  return { tasks, links, leftOut, ...(document.data_date === null ? {} : { dataDate: limitDate(document.data_date, document.origin) }), ...(document.must_finish_by === undefined ? {} : { mustFinishBy: limitDate(document.must_finish_by, document.origin) }), ...(document.options === undefined ? {} : { options: { ...document.options } }) };
}
