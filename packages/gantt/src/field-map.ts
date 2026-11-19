// The field map (decision 0022, rule 8.4): a host names the members of its rows once, at mount, and the chart renames them where a task or link crosses the boundary.
// It renames members and converts no value. A change's own members, a `schedule` change, the document and `snapshot()` keep the chart's names.
import { customFields, type CustomFields } from "./custom-fields.ts";
import type { Change } from "./events.js";
import type { Fields } from "./gantt-types.js";
import type { PlainLink, PlainTask } from "./adapters/plain.js";

type Row = Record<string, unknown>;

const TASK_FIELDS = ["id", "name", "start", "end", "duration", "parent", "progress", "color", "label", "key", "calendar", "snet", "snlt", "fnet", "fnlt", "mso", "mfo", "alap", "milestone", "actualStart", "actualFinish", "remaining", "resume"] as const satisfies readonly (keyof PlainTask)[];
const LINK_FIELDS = ["id", "from", "to", "type", "lag", "key"] as const satisfies readonly (keyof PlainLink)[];
/** The chart's names for the members of a task and of a link: the keys a `Fields` map may have. */
export type TaskField = (typeof TASK_FIELDS)[number];
export type LinkField = (typeof LINK_FIELDS)[number];

/** One kind of record's names: the rows a host gives in the chart's names, and the members of a change in the host's. */
export interface Names<T extends object> {
  /** One record of the host's, as `update` takes it; a value that is not an object passes through for the chart to refuse. */
  toChart(record: unknown): unknown;
  /** The host's rows, as the chart starts or compares them; the list itself when there is no map. */
  rows(list: readonly object[]): T[];
  /** The members of a change's `set` or `was`. */
  toHost(members: Row | undefined): Row | undefined;
}

export interface FieldMap {
  tasks: Names<PlainTask>;
  links: Names<PlainLink>;
  /** The rows of a `setData` call, in the chart's names. */
  data(rows: { tasks?: readonly object[]; links?: readonly object[] }): { tasks?: PlainTask[]; links?: PlainLink[] };
  /** A commit's changes with the members of each `set` and `was` in the host's names; a change's own members and a `schedule` change are not renamed. */
  changes(changes: readonly Change[]): Change[];
  /** The members of a row that are not a chart field, kept by key (`custom-fields.ts`). */
  custom: CustomFields;
}

const isRow = (value: unknown): value is Row => typeof value === "object" && value !== null && !Array.isArray(value);

const same: Names<never> = { toChart: (record) => record, rows: (list) => list as never[], toHost: (members) => members };

function namesOf<T extends object>(kind: "tasks" | "links", chartFields: readonly string[], entries: Record<string, string | undefined> | undefined, warn: (message: string) => void): Names<T> {
  const pairs = Object.entries(entries ?? {}) as [string, string][]; // a value that is not a text is refused below
  if (pairs.length === 0) return same;
  const toHostName = new Map<string, string>();
  const toChartName = new Map<string, string>();
  for (const [field, host] of pairs) {
    if (!chartFields.includes(field)) throw new Error(`fields.${kind} has an entry for "${field}", which is not a ${kind === "tasks" ? "task" : "link"} field of the chart.`);
    if (typeof host !== "string" || host === "") throw new Error(`fields.${kind}.${field} must be the host's name for it, a text.`);
    const taken = toChartName.get(host);
    if (taken !== undefined) throw new Error(`fields.${kind} gives the host name "${host}" to both "${taken}" and "${field}".`);
    toHostName.set(field, host);
    toChartName.set(host, field);
  }
  for (const [field, host] of pairs) {
    if (chartFields.includes(host) && host !== field && !toHostName.has(host) && !(field === "key" && host === "id")) throw new Error(`fields.${kind}.${field} is "${host}", the chart's own name for another field, which has no entry: add one for "${host}".`);
  }
  const toChart = (record: unknown): unknown => {
    if (!isRow(record)) return record;
    return Object.fromEntries(Object.entries(record).flatMap(([name, value]) => (toChartName.has(name) ? [[toChartName.get(name)!, value]] : toHostName.has(name) ? [] : [[name, value]]))); // the chart's own name of a mapped field is the host's custom field (tl2091cf)
  };
  let checked = false;
  const fillId = kind === "tasks" && toHostName.has("key"); // a task needs an id until a record with a key may leave it out (rule 5.2)
  return {
    toChart,
    rows: (list) => {
      if (!checked && list.length > 0) {
        checked = true;
        if (!list.some((row) => isRow(row) && [...toChartName.keys()].some((host) => host in row))) warn(`gantt: no ${kind === "tasks" ? "task" : "link"} carries a member that fields.${kind} names (${[...toChartName.keys()].join(", ")}); check the spelling.`);
      }
      return list.map((row) => {
        const out = toChart(row);
        return fillId && isRow(out) && out.key !== undefined && out.id === undefined ? { ...out, id: out.key } : out;
      }) as T[];
    },
    toHost: (members) => members && Object.fromEntries(Object.entries(members).flatMap(([name, value]) => (toHostName.has(name) ? [[toHostName.get(name)!, value]] : toChartName.has(name) ? [] : [[name, value]]))),
  };
}

/** Check `map` and read it. Throws an `Error` that names the fields for each of the three mistakes of rule 8.4: an entry for a field the chart does not have, one host name given to two fields, and a host name that is another field's own name with no entry (`key: "id"` is the one exception). */
export function readFields(map: Fields | undefined, warn: (message: string) => void = console.warn): FieldMap {
  const tasks = namesOf<PlainTask>("tasks", TASK_FIELDS, map?.tasks, warn);
  const links = namesOf<PlainLink>("links", LINK_FIELDS, map?.links, warn);
  const named = (change: Change): Change => {
    const names = change.record === "task" ? tasks : change.record === "link" ? links : same;
    return names === same ? change : { ...change, ...(change.set ? { set: names.toHost(change.set) } : {}), ...(change.was ? { was: names.toHost(change.was) } : {}) };
  };
  const data: FieldMap["data"] = (rows) => ({ ...(rows.tasks && { tasks: tasks.rows(rows.tasks) }), ...(rows.links && { links: links.rows(rows.links) }) });
  return { tasks, links, data, changes: (changes) => changes.map(named), custom: customFields({ tasks, links }) };
}
