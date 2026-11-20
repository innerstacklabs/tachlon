// The plain task adapter (tl#1079): `fromTasks` reads the smallest JSON a host page can write for a chart, tasks with a
// name and dates and links between them, and returns a document v1 (decision 0010) for `Gantt.load({ document })`. It
// reads the plain data once, with no copy in the DHTMLX shape, and shares the date reader, the one calendar and the
// `leftOut` lines with `fromDhtmlx` (it reads the same things the same way). A pure function of its input: no DOM, no
// wasm, no clock.
import { DAY, barLook, minutesOf, plural, unreadLines, type DocumentLink, type ScheduleOptions, type DocumentTask, type ScheduleDocument } from "./dhtmlx.ts";
import { dateAt, readActuals } from "./plain-actuals.ts";
import { exceptionsOf, lagCalendar, lagDay, readCalendars, toMinutes } from "./plain-calendars.ts";
import { readLimits, shiftLimit } from "./plain-limits.ts";
import { toSeparate } from "../constraint-form.ts";
import type { Constraint } from "../constraint-form.ts";
import { PlainDataError, inspect } from "./validate.ts";

/** A working calendar: its `key` (your own text, which a task's `calendar` and the data's `lagCalendar` name), and either `workDays` (ISO weekday numbers, 1 for Monday to 7, each worked from 08:00 for `hoursPerDay` hours) or `week` (seven lists of `[from, to]` minutes of the day, Monday first). `exceptions` are days that differ: `{ date, working }`, a holiday or an extra working day, which has the first worked weekday's hours. A calendar's day, the length a `days` duration counts, is `hoursPerDay`, or the average worked day of its `week`. */
export interface PlainCalendar {
  key: string;
  workDays?: number[];
  hoursPerDay?: number;
  week?: [number, number][][];
  exceptions?: { date: string; working: boolean }[];
  [field: string]: unknown;
}

/** A task: `start` and `end` are `YYYY-MM-DD[THH:mm]`, `duration` is in the data's `durationUnit` (working days of the task's `calendar`, by default), `progress` 0 to 100, `color` the bar's `#rrggbb`, `label` the text the bar shows in place of the name, `key` your own text id, kept through edits and saves. `snet` (start no earlier than, in place of `start`), `snlt`, `fnet`, `fnlt` (each a date), `mso` and `mfo` (a date each; one pin per task) and `alap: true` are the task's limits; a date alone on a finish limit is the end of the last working minute of that day on the task's calendar, and on a start limit the first working minute. `milestone` (`"start"` or `"finish"`) on a task with no duration puts it at the start of a working period or at the end of one (for example, after a task that ends at 17:00, `"start"` is 08:00 the next working day and `"finish"` is 17:00 that day). `actualStart` and `actualFinish` (dates; a date alone is the first working minute of the day, and the end of its last), `remaining` (as `duration` is) and `resume` (a date) say what happened; the last three need an `actualStart`, and `remaining` is not given with `progress`. `calendar` is the `key` of one of the data's `calendars`, the first when it is left out. A task with children is a summary that takes its dates from them. */
export interface PlainTask {
  id: string | number;
  calendar?: string;
  name: string;
  start?: string;
  end?: string;
  duration?: number;
  parent?: string | number;
  progress?: number;
  color?: string;
  label?: string;
  key?: string;
  snet?: string;
  snlt?: string;
  fnet?: string;
  fnlt?: string;
  mso?: string;
  mfo?: string;
  /** The one date limit, `{ type, date }` with a `type` of `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`, `so` or `fo`, or `null` for none: it is the limit its type names (`so` is `snet` and `snlt` on one date, `fo` is `fnet` and `fnlt` on one date), and it wins over `start`, which is then not a start-no-earlier-than (and may be left out). A task that gives it with one of the six members `snet`, `snlt`, `fnet`, `fnlt`, `mso` or `mfo` is refused. */
  constraint?: Constraint;
  alap?: boolean;
  milestone?: "start" | "finish";
  actualStart?: string;
  actualFinish?: string;
  remaining?: number;
  resume?: string;
  [field: string]: unknown;
}

/** A link from one task to another: `type` defaults to `FS` (finish to start), `lag` is in the data's `durationUnit` (working days of the data's `lagCalendar`, by default). */
export interface PlainLink {
  from: string | number;
  to: string | number;
  type?: "FS" | "SS" | "FF" | "SF";
  lag?: number;
  /** Your own text id for the link, kept through edits and saves. */
  key?: string;
  [field: string]: unknown;
}

/** What `fromTasks` takes beside the data. */
export interface PlainOptions {
  /** The schedule's name. Default `untitled`. */
  name?: string;
}

/** What `fromTasks` returns: the document, and one line for each thing of the data it did not carry over (also in `document.notes`). */
export interface PlainResult {
  document: ScheduleDocument;
  leftOut: string[];
}

const ACTUAL_KEYS = ["actualStart", "actualFinish", "remaining", "resume"];
const KNOWN_CALENDAR = new Set(["key", "workDays", "hoursPerDay", "week", "exceptions"]);
export const KNOWN_TASK = new Set(["id", "name", "start", "end", "duration", "parent", "progress", "text", "start_date", "end_date", "type", "color", "label", "key", "calendar", "snet", "snlt", "fnet", "fnlt", "mso", "mfo", "constraint", "alap", "milestone", ...ACTUAL_KEYS]);
export const KNOWN_LINK = new Set(["id", "from", "to", "type", "lag", "source", "target", "key"]);

/** Count the fields of `item` that are not in `known` (and do not start with `$`). */
function countUnread(item: object, known: Set<string>, counts: Map<string, number>): void {
  for (const key of Object.keys(item)) if (!known.has(key) && !key.startsWith("$")) counts.set(key, (counts.get(key) ?? 0) + 1);
}

/** Read plain tasks and links, the schedule's `dataDate` (a date, 00:00 of the day when it is a date alone) its `mustFinishBy` (a date; the end of that day when it is a date alone) and its `options` (passed through as one object, `validate` having checked its members), its `calendars`, its `durationUnit` (`"days"` by default, `"hours"` or `"minutes"`: what every `duration`, `remaining` and `lag` counts) and its `lagCalendar` (`"successor"` by default, `"predecessor"`, `"24h"` or `{ calendar: key }`: the calendar a lag day is counted on), into a document v1. Throws a `PlainDataError` that lists every problem (see `validate`) before it reads anything. */
export function fromTasks(data: { tasks: PlainTask[]; links?: PlainLink[]; dataDate?: string; mustFinishBy?: string; options?: ScheduleOptions; calendars?: PlainCalendar[]; durationUnit?: "days" | "hours" | "minutes"; lagCalendar?: "successor" | "predecessor" | "24h" | { calendar: string } }, options: PlainOptions = {}): PlainResult { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { issues, ids } = inspect(data);
  if (issues.length > 0) throw new PlainDataError(issues);
  const { tasks } = data;
  const links = data.links ?? [];
  const calendars = readCalendars(data.calendars);
  const calendarAt: number[] = []; // each task's calendar, by its place in `tasks`
  // The tree: children in file order, and the preorder the document lists them in.
  const children: number[][] = tasks.map(() => []);
  const roots: number[] = [];
  tasks.forEach((task, i) => {
    if (String(task.id) === "0") throw new Error(`DHTMLX data: task id 0 is the root, not a task (data[${i}])`);
    const p = task.parent;
    if (p === undefined || String(p) === "0" || p === "") roots.push(i);
    else children[ids.get(String(p))!]!.push(i);
  });
  const order: number[] = [];
  const level: number[] = [];
  const stack = roots.map((i) => i).reverse();
  roots.forEach((i) => (level[i] = 1));
  for (let i = stack.pop(); i !== undefined; i = stack.pop()) {
    order.push(i);
    const kids = children[i]!;
    for (let k = kids.length - 1; k >= 0; k--) {
      level[kids[k]!] = level[i]! + 1;
      stack.push(kids[k]!);
    }
  }
  if (order.length !== tasks.length) throw new Error("DHTMLX data: the parents of some tasks form a loop");
  const place: number[] = [];
  order.forEach((i, at) => (place[i] = at));

  const leftOut: string[] = [];
  const unreadTask = new Map<string, number>();
  const starts: (number | null)[] = [];
  const limitDates: number[] = [];
  const progress: (ReturnType<typeof readActuals> | null)[] = []; // each document task's progress keys; a summary has none
  let noDuration = 0;
  let summaryValues = 0;
  const date = (text: string | undefined, field: string, id: string): number | null => {
    if (text === undefined) return null;
    const minutes = minutesOf(text);
    if (minutes === null) throw new Error(`DHTMLX data: task ${id}: ${field} ${JSON.stringify(text)} is not a date (read YYYY-MM-DD HH:mm and DD-MM-YYYY HH:mm)`);
    return minutes;
  };
  const documentTasks: DocumentTask[] = order.map((i) => {
    const given = tasks[i]!;
    const constrained = given.constraint !== undefined; // the task's constraint wins over its start: `start` is then no floor (decision 0022, rule 9.5)
    const task = constrained ? (toSeparate(given) as PlainTask) : given;
    countUnread(task, KNOWN_TASK, unreadTask);
    const id = String(task.id);
    const parentAt = task.parent !== undefined && ids.has(String(task.parent)) ? place[ids.get(String(task.parent))!]! : null;
    const name = task.name !== "" ? task.name : id;
    const own = task.calendar === undefined ? 0 : calendars.at.get(task.calendar)!;
    calendarAt[i] = own;
    const { day, week } = calendars.list[own]!;
    const base = { ...barLook(task, unreadTask, true), ...(task.key === undefined ? {} : { key: task.key }), calendar: own, actual_start: null, actual_finish: null, remaining: null, name, parent: parentAt, outline_level: level[i]!, code: id, wbs_code: null, activity_codes: [] } satisfies Omit<DocumentTask, "duration" | "snet" | "progress">;
    const start = date(task.start, "start_date", id);
    const end = date(task.end, "end_date", id);
    const lim = readLimits(task, calendars.list[own]!);
    const fraction = task.progress === undefined ? 0 : task.progress / 100;
    if (children[i]!.length > 0) {
      if (start !== null || end !== null || (task.duration ?? 0) !== 0 || fraction !== 0 || ACTUAL_KEYS.some((key) => task[key] !== undefined)) summaryValues += 1;
      starts.push(null);
      progress.push(null);
      return { ...base, duration: 0, snet: null, progress: 0 };
    }
    const floor = constrained ? lim.snet : (lim.snet ?? start);
    starts.push(floor);
    const act = readActuals(task, week, (value) => toMinutes(value, data.durationUnit, day)); progress.push(act);
    limitDates.push(...lim.dates, ...act.dates);
    const limit = act.resume === null ? lim.limit : { ...lim.limit, resume: act.resume };
    let duration = 0;
    if (task.type !== "milestone") {
      if (task.duration !== undefined) duration = toMinutes(task.duration, data.durationUnit, day);
      else if (start !== null && end !== null) duration = ((end - start) / DAY) * day;
      else noDuration += 1;
      if (duration < 0) throw new Error(`DHTMLX data: task ${id} has a negative duration`);
    }
    return { ...base, duration: Math.round(duration), snet: floor, ...(limit === undefined ? {} : { limit }), progress: Math.round(Math.min(1, Math.max(0, fraction)) * 100) };
  });

  // The origin is the Monday on or before the earliest start (1970-01-01 was a Thursday); starts become minutes from it.
  const dataDate = data.dataDate === undefined ? null : dateAt(data.dataDate);
  const finishBy = data.mustFinishBy === undefined ? null : minutesOf(data.mustFinishBy)! + (data.mustFinishBy.length === 10 ? DAY : 0); // a finish date alone is the end of that day (decision 0017, rule 6)
  const known = [...starts.filter((start): start is number => start !== null), ...limitDates, ...(dataDate === null ? [] : [dataDate])];
  const first = known.length > 0 ? Math.floor(Math.min(...known) / DAY) : Math.floor(Date.UTC(2026, 0, 5) / 86_400_000);
  const origin = first - ((first + 3) % 7);
  const shift = (at: number | null): number | null => (at === null ? null : at - origin * DAY);
  documentTasks.forEach((task, at) => {
    const start = starts[at];
    if (start !== null && start !== undefined) task.snet = start - origin * DAY;
    if (task.limit !== undefined) task.limit = shiftLimit(task.limit, origin);
    const act = progress[at];
    if (act) [task.actual_start, task.actual_finish, task.remaining] = [shift(act.start), shift(act.finish), act.remaining];
  });

  const exceptions = calendars.list.map((calendar) => exceptionsOf(calendar, origin));
  const unreadCalendar = new Map<string, number>();
  data.calendars?.forEach((calendar) => countUnread(calendar, KNOWN_CALENDAR, unreadCalendar));
  const seen = new Set<number>();
  const unreadLink = new Map<string, number>();
  const documentLinks: DocumentLink[] = [];
  let toSummary = 0;
  let repeated = 0;
  links.forEach((link, i) => {
    countUnread(link, KNOWN_LINK, unreadLink);
    const where = `link ${link.id === undefined ? `links[${i}]` : String(link.id)}`;
    const pred = ids.get(String(link.from))!;
    const succ = ids.get(String(link.to))!;
    if (pred === succ) throw new Error(`DHTMLX data: ${where} joins task ${String(link.from)} to itself`);
    if (children[pred]!.length > 0 || children[succ]!.length > 0) {
      toSummary += 1;
      return;
    }
    const pair = pred * tasks.length + succ;
    if (seen.has(pair)) {
      repeated += 1;
      return;
    }
    seen.add(pair);
    documentLinks.push({ pred: place[pred]!, succ: place[succ]!, kind: link.type ?? "FS", lag: toMinutes(link.lag ?? 0, data.durationUnit, lagDay(data.lagCalendar, calendars, calendarAt[pred]!, calendarAt[succ]!)), lag_calendar: null,...(link.key === undefined ? {} : { key: link.key }) });
  });

  const label = "Plain JSON";
  if (summaryValues > 0) leftOut.push(`${label} summary tasks take their dates and progress from their children: the own start, end, duration, progress and constraint of ${plural(summaryValues, "summary task", "summary tasks")} are not read.`);
  if (noDuration > 0) leftOut.push(`${label} ${plural(noDuration, "task has", "tasks have")} no duration and no end_date, so ${noDuration === 1 ? "it is" : "they are"} read as zero length.`);
  if (toSummary > 0) leftOut.push(`${label} ${plural(toSummary, "link touches", "links touch")} a summary task, which a schedule here cannot link; ${toSummary === 1 ? "it is" : "they are"} left out.`);
  if (repeated > 0) leftOut.push(`${label} ${plural(repeated, "link repeats", "links repeat")} a pair of tasks already linked; the first is kept.`);
  const before = exceptions.reduce((sum, found) => sum + found.before, 0);
  if (before > 0) leftOut.push(`${label} ${plural(before, "calendar exception falls", "calendar exceptions fall")} before the schedule's first Monday, where no work starts; ${before === 1 ? "it is" : "they are"} left out.`);
  leftOut.push(...unreadLines(unreadTask, "task", label), ...unreadLines(unreadLink, "link", label), ...unreadLines(unreadCalendar, "calendar", label));

  const document: ScheduleDocument = {
    format: "gantt",
    version: 1,
    name: options.name ?? "untitled",
    origin,
    data_date: shift(dataDate),
    lag_calendar: lagCalendar(data.lagCalendar, calendars.at),
    calendars: calendars.list.map(({ week, key }, i) => ({ week, exceptions: exceptions[i]!.list, ...(key === undefined ? {} : { key }) })),
    tasks: documentTasks,
    links: documentLinks,
    baseline: null,
    view: { collapsed: [] },
    notes: leftOut,
    source: null,
    ...(data.options === undefined ? {} : { options: { ...data.options } }),
    ...(finishBy === null ? {} : { must_finish_by: shift(finishBy)! }),
  };
  return { document, leftOut };
}
