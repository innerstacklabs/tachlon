// The DHTMLX Gantt data adapter (gs#1097): `fromDhtmlx` reads the JSON that DHTMLX Gantt loads and saves
// (`{ data: [...], links: [...] }`) and returns a document v1 (decision 0010) for `Gantt.load({ document })`,
// plus one line for each thing it leaves out. It is a pure function of its input: no DOM, no wasm, no clock, so a
// host page or a Node script can call it. docs/DHTMLX.md lists the mapping and what is left out.

/** A task of DHTMLX data. Only the fields named here are read; any other field is listed in `leftOut`. */
export interface DhtmlxTask {
  id: string | number;
  text?: string;
  start_date?: string;
  end_date?: string;
  duration?: number | string;
  progress?: number | string;
  parent?: string | number;
  type?: string;
  /** `asap`, `alap`, `snet`, `snlt`, `fnet`, `fnlt`, `mso` or `mfo`; the date is `constraint_date`. */
  constraint_type?: string;
  constraint_date?: string;
  [field: string]: unknown;
}

/** A link of DHTMLX data: `type` is `"0"` finish to start, `"1"` start to start, `"2"` finish to finish, `"3"` start to finish. */
export interface DhtmlxLink {
  id?: string | number;
  source: string | number;
  target: string | number;
  type?: string | number;
  lag?: number | string;
  [field: string]: unknown;
}

/** The JSON DHTMLX Gantt loads: the tasks and the links. */
export interface DhtmlxData {
  data: DhtmlxTask[];
  links?: DhtmlxLink[];
}

/** What `fromDhtmlx` takes beside the data. */
export interface DhtmlxOptions {
  /** The schedule's name. Default `untitled`. */
  name?: string;
  /** The unit of the data's `duration` and `lag` (DHTMLX `duration_unit`): `day` (default) is a working day of 8 hours, `hour` an hour. */
  durationUnit?: "day" | "hour";
  /** The word the `leftOut` lines open with, naming where the data came from. Default `DHTMLX`. */
  label?: string;
}

/** The date limits of a task of the document (decision 0010): only the keys that are set. */
export interface DocumentLimit {
  snlt?: number;
  fnet?: number;
  fnlt?: number;
  pin_start?: number;
  pin_finish?: number;
  alap?: true;
  milestone?: "start" | "finish";
  resume?: number;
}

/** A task of the document. */
export interface DocumentTask {
  duration: number;
  calendar: number;
  snet: number | null;
  limit?: DocumentLimit;
  actual_start: number | null;
  actual_finish: number | null;
  remaining: number | null;
  name: string;
  parent: number | null;
  outline_level: number;
  code: string | null;
  wbs_code: null;
  activity_codes: [];
  progress: number;
  /** The text the bar shows in place of the name, only when set. */
  label?: string;
  /** The bar's own color as `#rrggbb`, only when set. */
  color?: string;
  /** The host's own key for the task, only when set. */
  key?: string;
}

/** The calendar a lag is counted on: the successor's, the predecessor's, a 24-hour day, or the calendar at that position. */
export type DocumentLagCalendar = "successor" | "predecessor" | "24h" | { calendar: number };

/** A link of the document. */
export interface DocumentLink {
  pred: number;
  succ: number;
  kind: "FS" | "SS" | "FF" | "SF";
  lag: number;
  lag_calendar: DocumentLagCalendar | null;
  /** The host's own key for the link, only when set. */
  key?: string;
}

/** The schedule options of a document and of plain data (decision 0018): only the members set, each spelled as the engine does. */
export interface ScheduleOptions { startedLateStart?: "p6" | "project"; unstartedRemaining?: "off" | "on" }

/** The document v1 that `fromDhtmlx` writes: keys in the order the writer uses. */
export interface ScheduleDocument {
  format: "gantt";
  version: 1;
  name: string;
  origin: number;
  data_date: number | null;
  lag_calendar: DocumentLagCalendar;
  calendars: { week: [number, number][][]; exceptions: [number, [number, number][]][]; key?: string }[];
  tasks: DocumentTask[];
  links: DocumentLink[];
  baseline: { data_date: null; project_finish: number; tasks: ([number, number, null] | null)[] } | null;
  view: { collapsed: [] };
  notes: string[];
  source: null;
  options?: ScheduleOptions;
  /** The date the project must finish by, in minutes from `origin`; only when set. */
  must_finish_by?: number;
}

/** What `fromDhtmlx` returns: the document, and one line for each thing of the data it did not carry over (also in `document.notes`). */
export interface DhtmlxResult {
  document: ScheduleDocument;
  leftOut: string[];
}

export const DAY = 1440;
export const WORKING_DAY = 480;
/** Working time of the one calendar: Monday to Friday, 08:00 to 12:00 and 13:00 to 17:00. */
export const WORKING_WEEK: [number, number][][] = [0, 1, 2, 3, 4, 5, 6].map((day) => (day < 5 ? [[480, 720], [780, 1020]] : []));
const KINDS = { "0": "FS", "1": "SS", "2": "FF", "3": "SF" } as const;
const TASK_FIELDS = new Set(["id", "text", "start_date", "end_date", "planned_start", "planned_end", "duration", "progress", "parent", "type", "constraint_type", "constraint_date", "color"]);
/** The DHTMLX constraint types that give a dated limit, and the document key each sets. `snet` is the start floor, `asap` nothing, `alap` the one undated line. */
export const LIMIT_KEYS = { snlt: "snlt", fnet: "fnet", fnlt: "fnlt", mso: "pin_start", mfo: "pin_finish" } as const;
const LINK_FIELDS = new Set(["id", "source", "target", "type", "lag"]);

/** Minutes since 1970-01-01 00:00 of the wall-clock time `text` names, or null. Reads `YYYY-MM-DD[ T]HH:mm[:ss]` (a trailing `Z` is not converted) and `DD-MM-YYYY[ HH:mm]`. */
export function minutesOf(text: string): number | null {
  const iso = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::\d{2}(?:\.\d+)?)?Z?)?$/.exec(text.trim());
  const dmy = /^(\d{2})-(\d{2})-(\d{4})(?: (\d{2}):(\d{2}))?$/.exec(text.trim());
  const found = iso !== null ? [iso[1], iso[2], iso[3], iso[4], iso[5]] : dmy !== null ? [dmy[3], dmy[2], dmy[1], dmy[4], dmy[5]] : null;
  if (found === null) return null;
  const [year, month, day, hour, minute] = found.map((part) => (part === undefined ? 0 : Number(part))) as [number, number, number, number, number];
  const date = new Date(Date.UTC(year, month - 1, day));
  if (date.getUTCFullYear() !== year || date.getUTCMonth() !== month - 1 || date.getUTCDate() !== day || hour > 23 || minute > 59) return null;
  return date.getTime() / 60_000 + hour * 60 + minute;
}

/** A finite number from a number or a numeric string, else null. */
function numberOf(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value === "string" && value.trim() !== "" && Number.isFinite(Number(value))) return Number(value);
  return null;
}

/** The bar look a task gives, in the document's key order: `color` as `#rrggbb`, and `label` text when `withLabel`. A value of another form is counted in `unread` under its key. */
export function barLook(task: Record<string, unknown>, unread: Map<string, number>, withLabel: boolean): Pick<DocumentTask, "label" | "color"> {
  const look: Pick<DocumentTask, "label" | "color"> = {};
  const miss = (key: string): void => void unread.set(key, (unread.get(key) ?? 0) + 1);
  if (withLabel && task.label !== undefined) typeof task.label === "string" ? (look.label = task.label) : miss("label");
  if (task.color !== undefined) typeof task.color === "string" && /^#[0-9a-f]{6}$/i.test(task.color) ? (look.color = task.color) : miss("color");
  return look;
}

/** Each field of `items` that is not in `known`, with the number of items that carry it (added to `counts`), as one line each. */
function unreadFields(items: readonly object[], known: Set<string>, what: string, label: string, counts = new Map<string, number>()): string[] {
  for (const item of items) {
    for (const key of Object.keys(item)) {
      if (!known.has(key) && !key.startsWith("$")) counts.set(key, (counts.get(key) ?? 0) + 1);
    }
  }
  return unreadLines(counts, what, label);
}

/** The `leftOut` lines for the unread fields in `counts` (field to the number of items that carry it), by field name. */
export function unreadLines(counts: Map<string, number>, what: string, label: string): string[] {
  return [...counts].sort(([a], [b]) => (a < b ? -1 : 1)).map(([key, count]) => `${label} ${what} field "${key}" is not read (${count}).`);
}

export function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

/** Read DHTMLX Gantt JSON into a document v1. Throws an `Error` whose message names the task or link it could not read. */
export function fromDhtmlx(source: unknown, options: DhtmlxOptions = {}): DhtmlxResult { // oxlint-disable-line max-lines-per-function -- existing, over cap
  if (typeof source !== "object" || source === null || !Array.isArray((source as { data?: unknown }).data)) {
    throw new Error('DHTMLX data: expected an object with a "data" array of tasks');
  }
  const input = source as { data: unknown[]; links?: unknown };
  if (input.links !== undefined && !Array.isArray(input.links)) throw new Error('DHTMLX data: "links" must be an array');
  const linkItems: unknown[] = input.links ?? [];
  const unit = options.durationUnit === "hour" ? 60 : WORKING_DAY;
  const asObject = (item: unknown, where: string): Record<string, unknown> => {
    if (typeof item !== "object" || item === null || Array.isArray(item)) throw new Error(`DHTMLX data: ${where} is not an object`);
    return item as Record<string, unknown>;
  };

  const tasks = input.data.map((item, i) => asObject(item, `data[${i}]`) as DhtmlxTask);
  const index = new Map<string, number>();
  tasks.forEach((task, i) => {
    if (typeof task.id !== "string" && typeof task.id !== "number") throw new Error(`DHTMLX data: data[${i}] has no id`);
    const key = String(task.id);
    if (key === "0") throw new Error(`DHTMLX data: task id 0 is the root, not a task (data[${i}])`);
    if (index.has(key)) throw new Error(`DHTMLX data: task id ${key} is used twice`);
    index.set(key, i);
  });

  // The tree: children in file order, and the preorder the document lists them in.
  const children: number[][] = tasks.map(() => []);
  const roots: number[] = [];
  tasks.forEach((task, i) => {
    const parent = task.parent === undefined || task.parent === null || String(task.parent) === "0" || task.parent === "" ? null : String(task.parent);
    if (parent === null) roots.push(i);
    else {
      const at = index.get(parent);
      if (at === undefined) throw new Error(`DHTMLX data: task ${String(task.id)} has parent ${parent}, which is not a task`);
      children[at]!.push(i);
    }
  });
  const order: number[] = [];
  const level = new Map<number, number>();
  const visit = (i: number, depth: number): void => {
    level.set(i, depth);
    order.push(i);
    for (const child of children[i]!) visit(child, depth + 1);
  };
  for (const root of roots) visit(root, 1);
  if (order.length !== tasks.length) throw new Error("DHTMLX data: the parents of some tasks form a loop");
  const place = new Map(order.map((taskIndex, at) => [taskIndex, at]));

  const leftOut: string[] = [];
  const label = options.label ?? "DHTMLX";
  const starts: (number | null)[] = [];
  const planned: ([number, number] | null)[] = [];
  let noDuration = 0;
  let oneSided = 0;
  const constraintDates: number[] = [];
  let summaryValues = 0;
  let unreadConstraints = 0;
  const unreadTask = new Map<string, number>();
  const documentTasks: DocumentTask[] = order.map((i) => {
    const task = tasks[i]!;
    const id = String(task.id);
    const summary = children[i]!.length > 0;
    const parentAt = task.parent !== undefined && index.has(String(task.parent)) ? place.get(index.get(String(task.parent))!)! : null;
    const base = { ...barLook(task, unreadTask, false), calendar: 0, actual_start: null, actual_finish: null, remaining: null, name: typeof task.text === "string" && task.text !== "" ? task.text : id, parent: parentAt, outline_level: level.get(i)!, code: id, wbs_code: null, activity_codes: [] } satisfies Omit<DocumentTask, "duration" | "snet" | "progress">;
    const date = (field: "start_date" | "end_date" | "planned_start" | "planned_end" | "constraint_date"): number | null => {
      const text = task[field];
      if (text === undefined || text === null || text === "") return null;
      const minutes = typeof text === "string" ? minutesOf(text) : null;
      if (minutes === null) throw new Error(`DHTMLX data: task ${id}: ${field} ${JSON.stringify(text)} is not a date (read YYYY-MM-DD HH:mm and DD-MM-YYYY HH:mm)`);
      return minutes;
    };
    const start = date("start_date");
    const end = date("end_date");
    const plannedStart = date("planned_start");
    const plannedEnd = date("planned_end");
    const constraint = typeof task.constraint_type === "string" ? task.constraint_type : "";
    const constraintAt = date("constraint_date");
    const constrained = constraint !== "" && constraint !== "asap";
    if (summary) {
      if (start !== null || end !== null || constrained || (numberOf(task.duration) ?? 0) !== 0 || (numberOf(task.progress) ?? 0) !== 0) summaryValues += 1;
      starts.push(null);
      planned.push(null);
      return { ...base, duration: 0, snet: null, progress: 0 };
    }
    // A start-no-earlier-than constraint is the start floor in place of `start_date`.
    const floor = constraint === "snet" && constraintAt !== null ? constraintAt : start;
    starts.push(floor);
    if ((plannedStart === null) !== (plannedEnd === null)) oneSided += 1;
    planned.push(plannedStart !== null && plannedEnd !== null ? [plannedStart, plannedEnd] : null);
    let limit: DocumentLimit | undefined;
    if (constraint === "alap") limit = { alap: true };
    else if (Object.hasOwn(LIMIT_KEYS, constraint) && constraintAt !== null) limit = { [LIMIT_KEYS[constraint as keyof typeof LIMIT_KEYS]]: constraintAt };
    else if (constrained && !(constraint === "snet" && constraintAt !== null)) unreadConstraints += 1;
    if (limit !== undefined && constraintAt !== null) constraintDates.push(constraintAt);
    let duration = 0;
    if (task.type !== "milestone") {
      const given = task.duration === undefined || task.duration === null || task.duration === "" ? null : numberOf(task.duration);
      if (given === null && task.duration !== undefined && task.duration !== null && task.duration !== "") throw new Error(`DHTMLX data: task ${id}: duration ${JSON.stringify(task.duration)} is not a number`);
      if (given !== null) duration = given * unit;
      else if (start !== null && end !== null) duration = ((end - start) / (unit === 60 ? 60 : DAY)) * unit;
      else noDuration += 1;
      if (duration < 0) throw new Error(`DHTMLX data: task ${id} has a negative duration`);
    }
    const fraction = task.progress === undefined || task.progress === null || task.progress === "" ? 0 : numberOf(task.progress);
    if (fraction === null) throw new Error(`DHTMLX data: task ${id}: progress ${JSON.stringify(task.progress)} is not a number`);
    return { ...base, duration: Math.round(duration), snet: floor, ...(limit === undefined ? {} : { limit }), progress: Math.round(Math.min(1, Math.max(0, fraction)) * 100) };
  });

  // The origin is the Monday on or before the earliest start or constraint date (1970-01-01 was a Thursday); they become minutes from it.
  const known = [...starts.filter((start): start is number => start !== null), ...constraintDates];
  const first = known.length > 0 ? Math.floor(Math.min(...known) / DAY) : Math.floor(Date.UTC(2026, 0, 5) / 86_400_000);
  const origin = first - ((first + 3) % 7);
  documentTasks.forEach((task, at) => {
    const start = starts[at];
    if (start !== null && start !== undefined) task.snet = start - origin * DAY;
    if (task.limit !== undefined) task.limit = Object.fromEntries(Object.entries(task.limit).map(([key, value]) => [key, typeof value === "number" ? value - origin * DAY : value])) as DocumentLimit;
  });

  // The planned dates are the baseline: no float, and the project finish is the latest planned finish.
  const entries = planned.map((dates) => (dates === null ? null : ([dates[0] - origin * DAY, dates[1] - origin * DAY, null] as [number, number, null])));
  const plannedFinish = Math.max(...entries.map((entry) => (entry === null ? -Infinity : entry[1])));
  const baseline: ScheduleDocument["baseline"] = plannedFinish === -Infinity ? null : { data_date: null, project_finish: plannedFinish, tasks: entries };

  const isSummary = (i: number): boolean => children[i]!.length > 0;
  const seen = new Set<string>();
  const documentLinks: DocumentLink[] = [];
  let toSummary = 0;
  let repeated = 0;
  const links = linkItems.map((item, i) => asObject(item, `links[${i}]`) as unknown as DhtmlxLink);
  links.forEach((link, i) => {
    const where = `link ${link.id === undefined ? `links[${i}]` : String(link.id)}`;
    const ends = [link.source, link.target].map((end) => index.get(String(end)));
    if (ends[0] === undefined || ends[1] === undefined) throw new Error(`DHTMLX data: ${where} names a task that is not in data`);
    const [pred, succ] = ends as [number, number];
    if (pred === succ) throw new Error(`DHTMLX data: ${where} joins task ${String(link.source)} to itself`);
    const kind = KINDS[String(link.type ?? "0") as keyof typeof KINDS];
    if (kind === undefined) throw new Error(`DHTMLX data: ${where} has type ${JSON.stringify(link.type)}; expected "0", "1", "2" or "3"`);
    const lag = link.lag === undefined || link.lag === null || link.lag === "" ? 0 : numberOf(link.lag);
    if (lag === null) throw new Error(`DHTMLX data: ${where}: lag ${JSON.stringify(link.lag)} is not a number`);
    if (isSummary(pred) || isSummary(succ)) {
      toSummary += 1;
      return;
    }
    const pair = `${pred}>${succ}`;
    if (seen.has(pair)) {
      repeated += 1;
      return;
    }
    seen.add(pair);
    documentLinks.push({ pred: place.get(pred)!, succ: place.get(succ)!, kind, lag: Math.round(lag * unit), lag_calendar: null });
  });

  if (summaryValues > 0) leftOut.push(`${label} summary tasks take their dates and progress from their children: the own start, end, duration, progress and constraint of ${plural(summaryValues, "summary task", "summary tasks")} are not read.`);
  if (unreadConstraints > 0) leftOut.push(`${label} ${plural(unreadConstraints, "task has", "tasks have")} a constraint with no date or of a type that is not known, so ${unreadConstraints === 1 ? "it is" : "they are"} not read.`);
  if (oneSided > 0) leftOut.push(`${label} ${plural(oneSided, "task has", "tasks have")} only one of planned_start and planned_end, so ${oneSided === 1 ? "it has" : "they have"} no baseline.`);
  if (noDuration > 0) leftOut.push(`${label} ${plural(noDuration, "task has", "tasks have")} no duration and no end_date, so ${noDuration === 1 ? "it is" : "they are"} read as zero length.`);
  if (toSummary > 0) leftOut.push(`${label} ${plural(toSummary, "link touches", "links touch")} a summary task, which a schedule here cannot link; ${toSummary === 1 ? "it is" : "they are"} left out.`);
  if (repeated > 0) leftOut.push(`${label} ${plural(repeated, "link repeats", "links repeat")} a pair of tasks already linked; the first is kept.`);
  leftOut.push(...unreadFields(tasks, TASK_FIELDS, "task", label, unreadTask), ...unreadFields(links, LINK_FIELDS, "link", label));
  const others = Object.keys(input).filter((key) => key !== "data" && key !== "links").sort();
  if (others.length > 0) leftOut.push(`${label} data holds ${others.map((key) => `"${key}"`).join(", ")}, which is not read.`);

  const document: ScheduleDocument = {
    format: "gantt",
    version: 1,
    name: options.name ?? "untitled",
    origin,
    data_date: null,
    lag_calendar: "successor",
    calendars: [{ week: WORKING_WEEK, exceptions: [] }],
    tasks: documentTasks,
    links: documentLinks,
    baseline,
    view: { collapsed: [] },
    notes: leftOut,
    source: null,
  };
  return { document, leftOut };
}
