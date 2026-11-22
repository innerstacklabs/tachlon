// `validate(data)` (tl#1249): check plain task data before `fromTasks` reads it, and name every problem: the path in the
// plain field names, what was expected, what was there, and one sentence for the fix. It states the same shape as
// `plain.schema.json`, plus what a schema cannot: ids that repeat or are 0, a parent loop, a date that does not exist, an end before the start, a link from a task to itself, a parent or link that names a missing task, a calendar key that repeats or that nothing defines, and the progress keys that need each other (`actualStart`, `actualFinish`, `remaining`, `resume`). A pure function of its input.
import { dateAt } from "./plain-actuals.ts";
import { readCalendars } from "./plain-calendars.ts";
import type { PlainCalendar } from "./plain.ts";

/** One problem: `path` is like `tasks[0].start`; `fix` is a sentence a person or an assistant can act on. */
export interface PlainIssue {
  path: string;
  expected: string;
  got: string;
  fix: string;
}

/** What `validate` returns: `{ ok: true }`, or every problem, up to 50. */
export type PlainValidation = { ok: true } | { ok: false; issues: PlainIssue[] };

const MAX = 50;
/** The number the `count` digits at `at` of `s` write, or -1 when one of them is not a digit 0 to 9. */
function digits(s: string, at: number, count: number): number {
  let n = 0;
  for (let i = at; i < at + count; i++) {
    const d = s.charCodeAt(i) - 48;
    if (!(d >= 0 && d <= 9)) return -1;
    n = n * 10 + d;
  }
  return n;
}
const MONTH_DAYS = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
/** Whether `v` is a date `fromTasks` reads, YYYY-MM-DD or YYYY-MM-DDTHH:mm from year 0100 and hours 00 to 23, on a day its month has: February 29 only in a year 4 divides, and of the years 100 divides only those 400 divides. */
function isDate(v: string): boolean {
  if (v.length === 16) {
    const h = digits(v, 11, 2), m = digits(v, 14, 2);
    if (v[10] !== "T" || v[13] !== ":" || h < 0 || h > 23 || m < 0 || m > 59) return false;
  } else if (v.length !== 10) return false;
  const y = digits(v, 0, 4), mo = digits(v, 5, 2), d = digits(v, 8, 2);
  if (v[4] !== "-" || v[7] !== "-" || y < 100 || mo < 1 || mo > 12 || d < 1 || d > MONTH_DAYS[mo - 1]!) return false;
  return mo !== 2 || d < 29 || (y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0));
}
const TYPES = ["FS", "SS", "FF", "SF"];

/** What `fromTasks` throws for data `validate` refuses; `issues` is the list, `message` is the list as lines. */
export class PlainDataError extends Error {
  readonly issues: PlainIssue[];

  constructor(issues: PlainIssue[]) {
    const count = issues.length >= MAX ? `${MAX} or more problems (the first ${MAX})` : `${issues.length} problem${issues.length === 1 ? "" : "s"}`;
    super(`Task data has ${count}:\n${issues.map((i) => `  ${i.path || "data"}: expected ${i.expected}, got ${i.got}. ${i.fix}`).join("\n")}`);
    this.name = "PlainDataError";
    this.issues = issues;
  }
}

type Kind = "id" | "text" | "date" | "number" | "type" | "flag" | "milestone" | "constraint";
const LIMITS = ["snet", "snlt", "fnet", "fnlt", "mso", "mfo"];
/** The types of a `constraint`: the six limit members, and `so` and `fo`, which are no member. */
const CONSTRAINT_TYPES = [...LIMITS, "so", "fo"];
interface Field {
  kind: Kind;
  required?: boolean;
  nullable?: boolean;
  min?: number;
  max?: number;
}

const TASK: Record<string, Field> = {
  id: { kind: "id", required: true },
  name: { kind: "text", required: true },
  start: { kind: "date", required: true },
  end: { kind: "date" },
  duration: { kind: "number", min: 0 },
  parent: { kind: "id", nullable: true },
  progress: { kind: "number", min: 0, max: 100 },
  key: { kind: "text" },
  snet: { kind: "date" },
  snlt: { kind: "date" },
  fnet: { kind: "date" },
  fnlt: { kind: "date" },
  mso: { kind: "date" },
  mfo: { kind: "date" },
  constraint: { kind: "constraint" },
  alap: { kind: "flag" },
  milestone: { kind: "milestone" },
  calendar: { kind: "text" },
  actualStart: { kind: "date" },
  actualFinish: { kind: "date" },
  remaining: { kind: "number", min: 0 },
  resume: { kind: "date" },
};
const LINK: Record<string, Field> = {
  from: { kind: "id", required: true },
  to: { kind: "id", required: true },
  type: { kind: "type" },
  lag: { kind: "number" },
  key: { kind: "text" },
};

const EXAMPLE: Record<Kind, string> = { id: '"site" or 7', text: '"Site works"', date: '"2026-03-02" or "2026-03-02T08:00"', number: "5", type: '"FS"', flag: "true", milestone: '"start"', constraint: '{ "type": "fnlt", "date": "2026-03-20" } or null' };

function show(v: unknown): string {
  if (v === undefined) return "nothing";
  if (v === null) return "null";
  if (Array.isArray(v)) return "an array";
  if (typeof v === "object") return "an object";
  const s = JSON.stringify(v);
  return `${typeof v} ${s.length > 40 ? `${s.slice(0, 37)}...` : s}`;
}

const isId = (v: unknown): v is string | number => typeof v === "string" || typeof v === "number";

/** Whether `v` is a good value for a field of this kind: the cheap test, so a good value allocates nothing. */
function fine(f: Field, v: unknown): boolean {
  if (v === null && f.nullable) return true;
  switch (f.kind) {
    case "id":
      return typeof v === "string" || typeof v === "number";
    case "text":
      return typeof v === "string";
    case "date":
      return typeof v === "string" && isDate(v);
    case "type":
      return typeof v === "string" && TYPES.includes(v);
    case "flag":
      return typeof v === "boolean";
    case "milestone":
      return v === "start" || v === "finish";
    case "constraint":
      return v === null || (typeof v === "object" && !Array.isArray(v) && CONSTRAINT_TYPES.includes((v as { type?: string }).type!) && fine({ kind: "date" }, (v as { date?: unknown }).date));
    case "number":
      return typeof v === "number" && Number.isFinite(v) && (f.min === undefined || v >= f.min) && (f.max === undefined || v <= f.max);
  }
}

/** The problem with one field's value. */
function problem(name: string, f: Field, v: unknown): Pick<PlainIssue, "expected" | "fix"> {
  const as = (expected: string) => ({ expected, fix: `Write ${name} as ${expected}, for example ${EXAMPLE[f.kind]}.` });
  switch (f.kind) {
    case "id":
      return as("text or a number");
    case "flag":
      return as("true or false");
    case "milestone":
      return as('"start" or "finish"');
    case "constraint":
      return as("null, or { type, date } with a type of snet, snlt, fnet, fnlt, mso, mfo, so or fo and a date");
    case "text":
      return as("text");
    case "date":
      return as("a date as YYYY-MM-DD or YYYY-MM-DDTHH:mm");
    case "type":
      return { expected: 'one of "FS", "SS", "FF", "SF"', fix: 'Use "FS" (finish to start), "SS", "FF" or "SF", or leave type out for FS.' };
    case "number": {
      if (typeof v !== "number" || !Number.isFinite(v)) return as("a number");
      const range = f.max === undefined ? `${f.min} or more` : `${f.min} to ${f.max}`;
      return { expected: `a number from ${range}`, fix: `Use a value of ${range} for ${name}.` };
    }
  }
}

/** The problems of a task's progress keys that no one field shows: `remaining` with `progress`, a key that needs an `actualStart`, and an actual finish before the start. */
function progressIssues(t: Record<string, unknown>, path: string, add: Add, unstarted: boolean, week: [number, number][][] | undefined, bad: number): void {
  if (t.remaining !== undefined && t.progress !== undefined) add(`${path}.remaining`, "remaining or progress, not both", show(t.remaining), "Keep remaining (the working days left) and remove progress, which the chart derives from it, or keep progress and remove remaining.");
  for (const key of ["actualFinish", "remaining", "resume"]) if (t[key] !== undefined && t.actualStart === undefined && !(unstarted && key === "remaining")) add(`${path}.${key}`, "an actualStart on the same task", "nothing", `Add actualStart, or remove ${key}: it describes a task that has started (remaining alone is read when options.unstartedRemaining is "on").`);
  if (t.actualStart !== undefined && t.actualFinish !== undefined && (bad & (BIT.actualStart! | BIT.actualFinish!)) === 0 && dateAt(t.actualFinish as string, "finish", week) < dateAt(t.actualStart as string, "start", week)) add(`${path}.actualFinish`, "an actual finish on or after the actual start", show(t.actualFinish), "Move actualFinish to the actualStart or later, or swap them.");
}

/** The schedule options of decision 0018 and the values each takes: the names the document and the engine use. */
const OPTIONS: Record<string, string[]> = { startedLateStart: ["p6", "project"], unstartedRemaining: ["off", "on"] };

/** The problems of the `options` object: a member that is not a schedule option, and a value it does not take. */
function optionIssues(options: unknown, add: (path: string, expected: string, got: string, fix: string) => void): void {
  if (typeof options !== "object" || options === null || Array.isArray(options)) return add("options", "an object of schedule options", show(options), 'Write options as an object, for example { "unstartedRemaining": "on" }.');
  const names = Object.keys(OPTIONS).join(" and ");
  for (const [name, value] of Object.entries(options)) {
    const values = Object.hasOwn(OPTIONS, name) ? OPTIONS[name]!.map((v) => JSON.stringify(v)).join(", ") : undefined;
    if (values === undefined) add(`options.${name}`, `one of ${names}`, show(value), `Remove ${name}, or spell it as ${names}.`);
    else if (!OPTIONS[name]!.includes(value as string)) add(`options.${name}`, `one of ${values}`, show(value), `Write ${name} as one of ${values}.`);
  }
}

type Add = (path: string, expected: string, got: string, fix: string) => void;

/** Whether `day` is a list of `[from, to]` minutes of the day, in order, none empty or past 24:00. */
const inOrder = (day: unknown): boolean => Array.isArray(day) && day.every((p: unknown, i) => Array.isArray(p) && Number.isInteger(p[0]) && Number.isInteger(p[1]) && p[0] >= 0 && p[0] < p[1] && p[1] <= 1440 && (i === 0 || p[0] >= (day[i - 1] as number[])[1]!));

/** The problems of one calendar: its key (added to `keys`), its one form (`workDays` with `hoursPerDay`, or `week`) and its exceptions. */
function calendarIssue(c: unknown, path: string, keys: Set<string>, add: Add): void {
  if (typeof c !== "object" || c === null || Array.isArray(c)) return add(path, "an object", show(c), "Write each calendar as an object in braces, with its key and its days.");
  const { key, workDays, hoursPerDay, week, exceptions } = c as Record<string, unknown>;
  if (typeof key !== "string" || key === "") add(`${path}.key`, "a key, text that is not empty", show(key), 'Give the calendar a key, for example "site".');
  else if (keys.has(key)) add(`${path}.key`, "a key no other calendar has", show(key), "Give each calendar its own key.");
  else keys.add(key);
  if (week !== undefined && (workDays !== undefined || hoursPerDay !== undefined)) add(`${path}.week`, "workDays with hoursPerDay, or week, not both", "both forms", "Keep workDays and hoursPerDay, or keep week, and remove the other.");
  else if (week !== undefined) {
    if (!Array.isArray(week) || week.length !== 7 || !week.every(inOrder) || week.every((day: unknown[]) => day.length === 0)) add(`${path}.week`, "seven days, Monday first, each a list of [from, to] minutes of the day in order, and one day at least worked", show(week), "Write week as seven lists, for example [[[480, 1020]], [], [], [], [], [], []].");
  } else {
    if (!Array.isArray(workDays) || workDays.length === 0 || workDays.some((d) => !Number.isInteger(d) || d < 1 || d > 7)) add(`${path}.workDays`, "a list of weekday numbers, 1 for Monday to 7", show(workDays), "Write workDays as a list such as [1, 2, 3, 4, 5].");
    const minutes = typeof hoursPerDay === "number" ? Math.round(hoursPerDay * 60) : 0;
    if (!(minutes >= 1 && minutes <= 960)) add(`${path}.hoursPerDay`, "a number of hours, more than 0 and at most 16", show(hoursPerDay), "Write hoursPerDay as a number such as 8.");
  }
  if (exceptions === undefined) return;
  if (!Array.isArray(exceptions)) return add(`${path}.exceptions`, "a list of { date, working }", show(exceptions), 'Write exceptions as a list such as [{ "date": "2026-12-25", "working": false }].');
  const days = new Set<string>();
  exceptions.forEach((e: unknown, j) => {
    const { date, working } = (e ?? {}) as Record<string, unknown>;
    if (typeof date !== "string" || date.length !== 10 || !fine(TASK.end!, date)) add(`${path}.exceptions[${j}].date`, "a day as YYYY-MM-DD", show(date), `Write date as a day, for example ${EXAMPLE.date.slice(0, 12)}.`);
    else if (days.has(date)) add(`${path}.exceptions[${j}].date`, "a day no other exception of this calendar has", show(date), "Keep one exception for each day.");
    else days.add(date);
    if (typeof working !== "boolean") add(`${path}.exceptions[${j}].working`, "true or false", show(working), "Write working as true for an extra working day or false for a holiday.");
  });
}

/** The problems of the data's `durationUnit`, `lagCalendar` and `calendars`; the keys of the calendars, which a task's `calendar` must be one of. */
function calendarIssues(data: Record<string, unknown>, add: Add): Set<string> {
  const { durationUnit, lagCalendar, calendars } = data;
  const keys = new Set<string>();
  if (durationUnit !== undefined && !["days", "hours", "minutes"].includes(durationUnit as string)) add("durationUnit", 'one of "days", "hours", "minutes"', show(durationUnit), 'Write durationUnit as "days" (the default), "hours" or "minutes".');
  if (calendars === undefined) { /* the one Monday to Friday calendar */ } else if (!Array.isArray(calendars)) add("calendars", "an array of calendars", show(calendars), 'Pass calendars as an array: [{ "key": "site", "workDays": [1, 2, 3, 4, 5], "hoursPerDay": 8 }], or leave calendars out.');
  else calendars.forEach((c, i) => calendarIssue(c, `calendars[${i}]`, keys, add));
  const named = (lagCalendar as { calendar?: unknown } | null)?.calendar;
  if (lagCalendar !== undefined && !["successor", "predecessor", "24h"].includes(lagCalendar as string) && !(typeof named === "string" && keys.has(named))) add("lagCalendar", 'one of "successor", "predecessor", "24h", or { "calendar": the key of a calendar }', show(lagCalendar), 'Write lagCalendar as "successor" (the default), "predecessor", "24h", or { "calendar": "site" } naming a calendar in calendars.');
  return keys;
}

/** A record's fields in order, and the names it requires. */
interface Table { entries: [string, Field][]; required: string[] }
const table = (fields: Record<string, Field>): Table => ({ entries: Object.entries(fields), required: Object.keys(fields).filter((name) => fields[name]!.required) });
const TASK_TABLE = table(TASK);
const LINK_TABLE = table(LINK);
/** Each task field's bit in the mask `fields` returns of the fields with a wrong value: its place in `TASK`. */
const BIT: Record<string, number> = Object.fromEntries(Object.keys(TASK).map((name, k) => [name, 1 << k]));

/** Check plain task data and name every problem, up to 50. */
export function validate(data: unknown): PlainValidation {
  const { issues } = inspect(data);
  return issues.length === 0 ? { ok: true } : { ok: false, issues };
}

/** `validate`'s one walk of the data: the problems, and the index of each task by `String(id)` for the reader to use. */
export function inspect(data: unknown): { issues: PlainIssue[]; ids: Map<string, number> } { // oxlint-disable-line max-lines-per-function -- the loop walk took it over the cap
  const issues: PlainIssue[] = [];
  const ids = new Map<string, number>();
  const add = (path: string, expected: string, got: string, fix: string) => {
    if (issues.length < MAX) issues.push({ path, expected, got, fix });
  };
  let dated = false; // `projectStart` is a date, so a task needs no `start`
  const keys = { tasks: new Map<string, number>(), links: new Map<string, number>() };
  const keyed = (kind: "tasks" | "links", item: unknown, i: number) => { // a key is unique among the records of its kind and is not empty (decision 0016)
    const key = (item as { key?: unknown } | null)?.key;
    const one = kind === "tasks" ? "task" : "link";
    const first = typeof key === "string" ? keys[kind].get(key) : undefined;
    if (key === "") add(`${kind}[${i}].key`, "a key that is not empty", '""', "Give the key some text, or leave key out.");
    else if (typeof key === "string" && first === undefined) keys[kind].set(key, i);
    else if (first !== undefined) add(`${kind}[${i}].key`, `a key no other ${one} has`, `${show(key)}, also ${kind}[${first}].key`, `Give each ${one} its own key.`);
  };
  let parents: Set<string> | undefined; // the ids tasks name as parent ("" and null are no parent), read only when a task's start or end needs it
  const named = (id: string | undefined) => id !== undefined && (parents ??= new Set((data as { tasks: unknown[] }).tasks.flatMap((t) => { const p = (t as { parent?: unknown } | null)?.parent; return isId(p) && p !== "" ? [String(p)] : []; }))).has(id);
  /** The fields' problems, and the mask of the fields with a wrong value by their place in `table`: all bits for an entry that is no object. A task whose `id` other tasks name as parent takes its dates from them. */
  const fields = (item: unknown, path: string, { entries, required }: Table, id?: string): number => {
    if (typeof item !== "object" || item === null || Array.isArray(item)) {
      add(path, "an object", show(item), "Write each entry as an object in braces, with its fields.");
      return -1;
    }
    const rec = item as Record<string, unknown>;
    let bad = 0;
    for (let k = 0; k < entries.length; k++) {
      const v = rec[entries[k]![0]];
      if (v !== undefined && !fine(entries[k]![1], v)) bad |= 1 << k;
    }
    let complete = true;
    for (const name of required) if (rec[name] === undefined) complete = false;
    if (bad === 0 && complete) return 0;
    for (let k = 0; k < entries.length; k++) { // the problems in the table's order
      const [name, f] = entries[k]!;
      const v = rec[name];
      if (v === undefined) {
        if (f.required && !(name === "start" && (rec.constraint !== undefined || dated || named(id)))) add(`${path}.${name}`, `a ${name}`, "nothing", `Add ${name}, for example ${EXAMPLE[f.kind]}.`);
      } else if ((bad & (1 << k)) !== 0) {
        const p = problem(name, f, v);
        add(`${path}.${name}`, p.expected, show(v), p.fix);
      }
    }
    return bad;
  };
  if (typeof data !== "object" || data === null || Array.isArray(data)) {
    add("", 'an object with a "tasks" array', show(data), 'Pass { "tasks": [...], "links": [...] }.');
    return { issues, ids };
  }
  const { tasks, links, projectStart, dataDate, mustFinishBy, options } = data as { tasks?: unknown; links?: unknown; projectStart?: unknown; dataDate?: unknown; mustFinishBy?: unknown; options?: unknown };
  if (options !== undefined) optionIssues(options, add);
  const before = issues.length;
  const calendarKeys = calendarIssues(data as Record<string, unknown>, add);
  const read = issues.length === before && before < MAX && Array.isArray((data as { calendars?: unknown }).calendars) ? readCalendars((data as { calendars: PlainCalendar[] }).calendars) : undefined; // the calendars as `fromTasks` reads them, to read a date alone on the task's own week
  const unstarted = (options as { unstartedRemaining?: unknown } | null)?.unstartedRemaining === "on"; // `remaining` alone is then a task's length before it starts
  dated = projectStart !== undefined && fine(TASK.end!, projectStart);
  if (projectStart !== undefined && !dated) add("projectStart", "a date as YYYY-MM-DD or YYYY-MM-DDTHH:mm", show(projectStart), `Write projectStart as a date, for example ${EXAMPLE.date}.`);
  if (dataDate !== undefined && !fine(TASK.end!, dataDate)) add("dataDate", "a date as YYYY-MM-DD or YYYY-MM-DDTHH:mm", show(dataDate), `Write dataDate as a date, for example ${EXAMPLE.date}.`);
  if (mustFinishBy !== undefined && !fine(TASK.end!, mustFinishBy)) add("mustFinishBy", "a date as YYYY-MM-DD or YYYY-MM-DDTHH:mm", show(mustFinishBy), `Write mustFinishBy as a date, for example ${EXAMPLE.date}.`);
  if (!Array.isArray(tasks)) add("tasks", "an array of tasks", show(tasks), 'Pass the tasks as an array: { "tasks": [{ "id": 1, "name": "Task", "start": "2026-03-02" }] }.');
  if (links !== undefined && !Array.isArray(links)) add("links", "an array of links", show(links), 'Pass links as an array: [{ "from": 1, "to": 2 }], or leave links out.');
  const nums = new Map<number, number>(); // `ids` of the tasks with a number id, so a number is found without `String`
  const has = (v: string | number) => (typeof v === "number" ? nums.has(v) || ids.has(String(v)) : ids.has(v));
  if (Array.isArray(tasks)) {
    const late = new Uint8Array(tasks.length); // late[i]: an end before the start on a leaf whose end `fromTasks` reads
    for (let i = 0; i < tasks.length; i++) {
      if (!(i in tasks)) continue;
      const t = tasks[i];
      const id = (t as { id?: unknown } | null)?.id;
      const name = isId(id) ? String(id) : undefined;
      const bad = fields(t, `tasks[${i}]`, TASK_TABLE, name);
      keyed("tasks", t, i);
      const calendar = (t as { calendar?: unknown } | null)?.calendar;
      if (typeof calendar === "string" && !calendarKeys.has(calendar)) add(`tasks[${i}].calendar`, "the key of a calendar in calendars", show(calendar), "Use the key of a calendar in calendars, or leave calendar out for the first.");
      const twice = typeof t === "object" && t !== null && (t as { constraint?: unknown }).constraint !== undefined ? LIMITS.find((limit) => (t as Record<string, unknown>)[limit] !== undefined) : undefined;
      if (twice !== undefined) add(`tasks[${i}].${twice}`, "constraint or this member, not both", show((t as Record<string, unknown>)[twice]), `Keep constraint, or the limit members snet to mfo, and remove the other: they are two spellings of one date limit.`);
      const pins = t as { mso?: unknown; mfo?: unknown } | null;
      if (pins?.mso !== undefined && pins.mfo !== undefined) add(`tasks[${i}].mfo`, "one pin, mso or mfo, not both", show(pins.mfo), "Keep mso (must start on) or mfo (must finish on), and remove the other.");
      if (typeof t === "object" && t !== null) progressIssues(t as Record<string, unknown>, `tasks[${i}]`, add, unstarted, read?.list[typeof calendar === "string" ? read.at.get(calendar) ?? 0 : 0]?.week, bad);
      const first = name === undefined ? undefined : ids.get(name);
      if (name !== undefined) {
        if (name === "0") add(`tasks[${i}].id`, "an id other than 0", show(id), "Use another id: 0 is the root of the data, not a task.");
        if (first === undefined) {
          ids.set(name, i);
          if (typeof id === "number") nums.set(id, i);
        } else add(`tasks[${i}].id`, "an id no other task has", `${show(id)}, also tasks[${first}].id`, "Give each task its own id.");
      }
      // `fromTasks` reads an end only of a leaf (not the first task of an id named as parent) with no duration and no milestone type; a date alone is read as 00:00
      const { start, end, duration, type } = (t ?? {}) as { start?: unknown; end?: unknown; duration?: unknown; type?: unknown };
      if (duration !== undefined || type === "milestone" || start === undefined || end === undefined || (bad & (BIT.start! | BIT.end!)) !== 0) continue;
      const s = start as string, e = end as string;
      if ((s.length === e.length ? e < s : (e.length === 10 ? `${e}T00:00` : e) < (s.length === 10 ? `${s}T00:00` : s)) && !(first === undefined && named(name))) late[i] = 1;
    }
    const ROOT = -1, MISSING = -2, UNREAD = -3; // what `up` gives besides the index of a parent; a missing parent is a root to walk up from
    const above = new Int32Array(tasks.length).fill(UNREAD);
    const up = (i: number) => { // parent "" is a root, as `fromTasks` reads it, and missing unless a task has the id ""
      if (above[i] === UNREAD) { const p = (tasks[i] as { parent?: unknown } | null)?.parent; const at = typeof p === "number" ? nums.get(p) ?? ids.get(String(p)) : typeof p === "string" ? ids.get(p) : undefined; above[i] = at === undefined ? (isId(p) ? MISSING : ROOT) : p === "" ? ROOT : at; }
      return above[i]!;
    };
    const seen = new Int32Array(tasks.length); // one walk up from each task; seen[t] is the last walk (i + 1) to reach t, so a walk that meets itself found a loop
    const after: Parameters<Add>[] = []; // the end and loop problems, which come after every missing parent
    for (let i = 0; i < tasks.length; i++) {
      if (up(i) === MISSING) add(`tasks[${i}].parent`, "the id of a task in tasks", show((tasks[i] as { parent: unknown }).parent), "Use the id of a task in tasks, or leave parent out.");
      if (late[i] === 1 && after.length < MAX) after.push([`tasks[${i}].end`, "an end on or after the start", show((tasks[i] as { end: unknown }).end), "Move end to the start or later, or swap start and end."]);
      let at = i;
      for (; at >= 0 && seen[at] === 0; at = up(at)) seen[at] = i + 1;
      if (at >= 0 && seen[at] === i + 1 && after.length < MAX) after.push([`tasks[${at}].parent`, "a parent that is not below the task", show((tasks[at] as { parent?: unknown }).parent), "Change parent so the parents do not loop back to the task."]);
    }
    for (const a of after) add(...a);
  }
  if (Array.isArray(links)) {
    for (let i = 0; i < links.length; i++) {
      if (!(i in links)) continue;
      const l = links[i];
      fields(l, `links[${i}]`, LINK_TABLE);
      keyed("links", l, i);
      const { from, to } = (l ?? {}) as { from?: unknown; to?: unknown };
      if (isId(from) && isId(to) && (typeof from === typeof to ? from === to || (from !== from && to !== to) : String(from) === String(to))) add(`links[${i}].to`, "a task other than from", show(to), "Link two different tasks: a task cannot depend on itself.");
      for (const end of ["from", "to"]) {
        const v = (l as Record<string, unknown> | null)?.[end];
        if (Array.isArray(tasks) && isId(v) && !has(v)) add(`links[${i}].${end}`, "the id of a task in tasks", show(v), `Use the id of a task in tasks for ${end}, or remove this link.`);
      }
    }
  }
  return { issues, ids };
}
