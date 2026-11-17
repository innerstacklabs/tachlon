// The Bryntum Gantt adapter (gs#1098): reads the JSON a Bryntum Gantt project loads (its tasks and dependencies) into
// the document (decision 0010), which `Gantt.load({ document })` takes. Pure TypeScript: no wasm, no network, no DOM.
// `docs/adapters/bryntum.md` is the page for a developer; it lists what is read and what is left out.

/** The date limits a task holds beyond `snet`, minutes from the origin: the keys of the document's task `limit`, only those that are set. */
export interface DocumentLimit {
  /** Start no later than. */
  snlt?: number;
  /** Finish no earlier than. */
  fnet?: number;
  /** Finish no later than. */
  fnlt?: number;
  /** Must start on. */
  pin_start?: number;
  /** Must finish on. */
  pin_finish?: number;
  /** As late as possible: the task starts as late as the schedule allows. */
  alap?: true;
}

/** One task of the document, in the shape `App.save` writes (version 1). */
export interface DocumentTask {
  /** Working minutes of the task's calendar. */
  duration: number;
  calendar: number;
  /** Earliest start, minutes from the origin Monday 00:00; null for none. */
  snet: number | null;
  /** The date limits beyond `snet`; the key is absent for a task with none. */
  limit?: DocumentLimit;
  actual_start: number | null;
  actual_finish: number | null;
  remaining: number | null;
  name: string;
  /** The index in `tasks` of the parent, or null for a top-level task. */
  parent: number | null;
  /** 1 for a top-level task. */
  outline_level: number;
  code: string | null;
  wbs_code: string | null;
  activity_codes: [string, string][];
  /** Percent complete, 0 to 100. */
  progress: number;
}

/** One link of the document, between two indexes of `tasks`. */
export interface DocumentLink {
  pred: number;
  succ: number;
  kind: "FS" | "SS" | "FF" | "SF";
  /** Working minutes; negative for a lead. */
  lag: number;
  lag_calendar: null;
}

/** The document (version 1) with the keys the adapter fills; `Gantt.load({ document })` takes it. */
export interface ScheduleDocument {
  format: "gantt";
  version: 1;
  name: string;
  /** Days from 1970-01-01 to the Monday that is instant 0. */
  origin: number;
  data_date: null;
  lag_calendar: "successor";
  calendars: { week: [number, number][][]; exceptions: [] }[];
  tasks: DocumentTask[];
  links: DocumentLink[];
  baseline: null;
  view: { collapsed: [] };
  /** The lines of `leftOut`, which the chart shows as its import notes. */
  notes: string[];
  source: null;
}

/** What `fromBryntum` returns: the document and one line for each thing in the data it could not carry, or why there is no document. */
export type BryntumResult = { ok: true; document: ScheduleDocument; leftOut: string[] } | { ok: false; error: string };

/** The working minutes in one day, week (five days) and month (twenty days) of the document's calendar. */
const DAY = 480;
const MINUTES_PER_UNIT: Record<string, number> = {
  millisecond: 1 / 60_000,
  second: 1 / 60,
  minute: 1,
  hour: 60,
  day: DAY,
  week: 5 * DAY,
  month: 20 * DAY,
  quarter: 60 * DAY,
  year: 240 * DAY,
};
const UNIT_NAMES: Record<string, string> = {
  ms: "millisecond", millisecond: "millisecond",
  s: "second", sec: "second", second: "second",
  mi: "minute", min: "minute", minute: "minute",
  h: "hour", hr: "hour", hour: "hour",
  d: "day", day: "day",
  w: "week", wk: "week", week: "week",
  mo: "month", mon: "month", month: "month",
  q: "quarter", quarter: "quarter",
  y: "year", yr: "year", year: "year",
};
/** The constraint types (lower case) that become a `limit` key; `startnoearlierthan` becomes `snet`. */
const LIMIT_KEYS = new Map<string, Exclude<keyof DocumentLimit, "alap">>([
  ["startnolaterthan", "snlt"],
  ["finishnoearlierthan", "fnet"],
  ["finishnolaterthan", "fnlt"],
  ["muststarton", "pin_start"],
  ["mustfinishon", "pin_finish"],
]);
const KINDS: DocumentLink["kind"][] = ["SS", "SF", "FS", "FF"];
/** Monday to Friday, 08:00 to 12:00 and 13:00 to 17:00. */
const WORKING_DAY: [number, number][] = [[480, 720], [780, 1020]];
/** The most dependency lines listed one by one; the rest are counted. */
const MAX_LISTED = 20;

type Row = Record<string, unknown>;

const isRow = (value: unknown): value is Row => typeof value === "object" && value !== null && !Array.isArray(value);
const key = (id: unknown): string => String(id);

/** The rows of a Bryntum store: `{ rows: [...] }` (a load response) or a plain array (a project's `tasks`). */
function rowsOf(value: unknown): Row[] | undefined {
  const list = Array.isArray(value) ? value : isRow(value) ? value.rows : undefined;
  return Array.isArray(list) ? list.filter(isRow) : undefined;
}

/** Days since 1970-01-01 and minutes into the day of an ISO date or date-time text. A zone is ignored: the text is read as written. */
function parseDate(value: unknown): { day: number; minute: number } | undefined {
  const m = typeof value === "string" ? /^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2}))?)?/.exec(value) : null;
  if (m === null) return undefined;
  const [year, month, date] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const stamp = new Date(Date.UTC(year, month - 1, date));
  if (stamp.getUTCMonth() !== month - 1 || stamp.getUTCDate() !== date) return undefined;
  const day = stamp.getTime() / 86_400_000;
  return { day, minute: Number(m[4] ?? 0) * 60 + Number(m[5] ?? 0) + Math.round(Number(m[6] ?? 0) / 60) };
}

/** A length in working minutes of the document's calendar; undefined for a unit or number it cannot read. */
function minutes(amount: unknown, unit: unknown): number | undefined {
  const text = (unit ?? "day") as unknown;
  const lower = typeof text === "string" ? text.toLowerCase() : "";
  const name = UNIT_NAMES[lower] ?? UNIT_NAMES[lower.replace(/s$/, "")];
  if (typeof amount !== "number" || !Number.isFinite(amount) || name === undefined) return undefined;
  return Math.round(amount * MINUTES_PER_UNIT[name]!);
}

interface TaskNode {
  row: Row;
  id: string;
  children: TaskNode[];
}

/** The task rows as a forest: nested `children`, or flat rows tied by `parentId`. Duplicate ids are dropped and listed. */
function forestOf(rows: Row[], leftOut: string[]): TaskNode[] {
  const byId = new Map<string, TaskNode>();
  const flat: TaskNode[] = [];
  const visit = (row: Row, parent: TaskNode | undefined): TaskNode | undefined => {
    const id = row.id === undefined ? `row ${flat.length + 1}` : key(row.id);
    if (byId.has(id)) {
      leftOut.push(`Task ${id} appears more than once; the first one is read.`);
      return undefined;
    }
    const node: TaskNode = { row, id, children: [] };
    byId.set(id, node);
    flat.push(node);
    parent?.children.push(node);
    if (Array.isArray(row.children)) for (const child of row.children) if (isRow(child)) visit(child, node);
    return node;
  };
  const roots: TaskNode[] = [];
  const tied: [TaskNode, string][] = [];
  for (const row of rows) {
    const node = visit(row, undefined);
    if (node === undefined) continue;
    if (row.parentId === undefined || row.parentId === null) roots.push(node);
    else tied.push([node, key(row.parentId)]);
  }
  for (const [node, parentId] of tied) {
    const parent = byId.get(parentId);
    if (parent === undefined || parent === node) {
      leftOut.push(`Task ${node.id} names parent ${parentId}, which is not in the data; it is read as a top-level task.`);
      roots.push(node);
    } else parent.children.push(node);
  }
  const reached = new Set<TaskNode>();
  const walk = (node: TaskNode): void => {
    reached.add(node);
    node.children.forEach(walk);
  };
  roots.forEach(walk);
  const lost = flat.filter((node) => !reached.has(node));
  if (lost.length > 0) leftOut.push(`${lost.length} task(s) form a parent loop and are left out (${lost.slice(0, 5).map((n) => n.id).join(", ")}).`);
  return roots;
}

/** Monday 00:00 on or before `day`, in days since 1970-01-01 (1970-01-01 was a Thursday). */
const mondayOnOrBefore = (day: number): number => day - ((((day + 3) % 7) + 7) % 7);

/**
 * Read Bryntum Gantt data into the document. `data` is what a Bryntum project loads: `{ tasks, dependencies }`, each
 * a `{ rows: [...] }` store or an array (`tasksData` and `dependenciesData` are read too). A task is nested under its
 * `children` or tied to its parent by `parentId`. Never throws: data with no task list gives `{ ok: false, error }`.
 */
export function fromBryntum(data: unknown): BryntumResult { // oxlint-disable-line max-lines-per-function -- existing, over cap
  if (!isRow(data)) return { ok: false, error: "Bryntum data must be an object with a tasks list" };
  const taskRows = rowsOf(data.tasks ?? data.tasksData);
  if (taskRows === undefined) return { ok: false, error: "Bryntum data has no tasks list (tasks: { rows: [...] } or an array)" };
  const leftOut: string[] = [];
  const roots = forestOf(taskRows, leftOut);

  // Outline order: parents before their children.
  const order: TaskNode[] = [];
  const level = new Map<TaskNode, number>();
  const parentOf = new Map<TaskNode, TaskNode>();
  const flatten = (node: TaskNode, depth: number): void => {
    order.push(node);
    level.set(node, depth);
    for (const child of node.children) {
      parentOf.set(child, node);
      flatten(child, depth + 1);
    }
  };
  roots.forEach((root) => flatten(root, 1));
  const index = new Map(order.map((node, i) => [node.id, i]));

  const depRows = rowsOf(data.dependencies ?? data.dependenciesData) ?? [];
  const links: DocumentLink[] = [];
  const seen = new Set<string>();
  let more = 0;
  const drop = (text: string): void => {
    if (leftOut.length < MAX_LISTED) leftOut.push(text);
    else more += 1;
  };
  for (const [i, row] of depRows.entries()) {
    const id = row.id === undefined ? `${i + 1}` : key(row.id);
    const [from, to] = [key(row.from), key(row.to)];
    const [pred, succ] = [index.get(from), index.get(to)];
    const lag = row.lag === undefined || row.lag === null ? 0 : minutes(row.lag, row.lagUnit);
    const kind = row.type === undefined ? "FS" : Number.isInteger(row.type) ? KINDS[row.type as number] : undefined;
    if (pred === undefined || succ === undefined) {
      drop(`Dependency ${id} refers to task ${pred === undefined ? from : to}, which is not in the data.`);
    } else if (pred === succ) {
      drop(`Dependency ${id} joins task ${from} to itself.`);
    } else if (order[pred]!.children.length > 0 || order[succ]!.children.length > 0) {
      drop(`Dependency ${id} (task ${from} to task ${to}) touches a parent task; the document links tasks that have no children.`);
    } else if (kind === undefined || lag === undefined) {
      drop(`Dependency ${id} has a type, lag or lag unit that is not read.`);
    } else if (seen.has(`${pred} ${succ}`)) {
      drop(`Dependency ${id} repeats the link from task ${from} to task ${to}.`);
    } else {
      seen.add(`${pred} ${succ}`);
      links.push({ pred, succ, kind, lag, lag_calendar: null });
    }
  }
  if (more > 0) leftOut.push(`${more} more dependencies were left out for the same reasons.`);
  const hasPredecessor = new Set(links.map((l) => l.succ));

  const dated = order.flatMap((node) => [parseDate(node.row.startDate), parseDate(node.row.constraintDate)]).filter((d) => d !== undefined);
  const origin = mondayOnOrBefore(dated.length > 0 ? Math.min(...dated.map((d) => d.day)) : Date.UTC(2026, 0, 5) / 86_400_000);
  const instant = (d: { day: number; minute: number }): number => (d.day - origin) * 1440 + d.minute;

  let constraints = 0;
  const tasks: DocumentTask[] = order.map((node, i) => {
    const { row } = node;
    const parent = parentOf.get(node);
    const summary = node.children.length > 0;
    let duration = 0;
    let snet: number | null = null;
    let limit: DocumentLimit | undefined;
    if (!summary) {
      const length = row.duration === undefined ? 0 : minutes(row.duration, row.durationUnit);
      if (length === undefined) leftOut.push(`Task ${node.id} has a duration or unit that is not read; it is read as 0.`);
      duration = Math.max(0, length ?? 0);
      const type = typeof row.constraintType === "string" ? row.constraintType.toLowerCase() : "none";
      const fixed = parseDate(row.constraintDate);
      const start = parseDate(row.startDate);
      const limitKey = LIMIT_KEYS.get(type);
      const read = fixed !== undefined && (type === "startnoearlierthan" || limitKey !== undefined);
      const alap = type === "aslateaspossible";
      if (type !== "none" && !read && !alap) constraints += 1;
      if (alap) limit = { alap: true };
      if (read && limitKey !== undefined) limit = { [limitKey]: instant(fixed) };
      if (read && limitKey === undefined) snet = instant(fixed);
      else if (start !== undefined && (row.manuallyScheduled === true || !hasPredecessor.has(i))) snet = instant(start);
    }
    const percent = typeof row.percentDone === "number" && Number.isFinite(row.percentDone) ? Math.min(100, Math.max(0, Math.round(row.percentDone))) : 0;
    return {
      duration,
      calendar: 0,
      snet,
      ...(limit === undefined ? {} : { limit }),
      actual_start: null,
      actual_finish: null,
      remaining: null,
      name: typeof row.name === "string" ? row.name : "",
      parent: parent === undefined ? null : index.get(parent.id)!,
      outline_level: level.get(node)!,
      code: node.id,
      wbs_code: null,
      activity_codes: [],
      progress: percent,
    };
  });

  if (constraints > 0) leftOut.push(`${constraints} task(s) have a constraint that is not read (a type the chart cannot hold, or no date).`);
  const present = (value: unknown): boolean => (rowsOf(value)?.length ?? 0) > 0;
  if (present(data.resources) || present(data.assignments)) leftOut.push("Resources and assignments are not read.");
  if (present(data.calendars) || order.some((n) => n.row.calendar !== undefined)) {
    leftOut.push("Calendars are not read: every task follows Monday to Friday, 08:00 to 12:00 and 13:00 to 17:00.");
  }
  if (order.some((n) => Array.isArray(n.row.baselines) && n.row.baselines.length > 0)) leftOut.push("Baselines are not read.");

  const document: ScheduleDocument = {
    format: "gantt",
    version: 1,
    name: "Bryntum import",
    origin,
    data_date: null,
    lag_calendar: "successor",
    calendars: [{ week: [...Array.from({ length: 5 }, () => WORKING_DAY), [], []], exceptions: [] }],
    tasks,
    links,
    baseline: null,
    view: { collapsed: [] },
    notes: [...leftOut],
    source: null,
  };
  return { ok: true, document, leftOut: [...leftOut] };
}
