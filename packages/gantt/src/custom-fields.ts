// Custom fields (decision 0022, rule 8.3): a member of a host's row that the chart has no field for stays in the page, by the row's key, and comes back unchanged in a create's `set` and a
// delete's `was`, so the create that undoes a delete gives the host its own columns again. The chart never reads, shows or writes one, and the document does not hold them.
import { KNOWN_LINK, KNOWN_TASK } from "./adapters/plain.ts";
import type { Change } from "./events.js";
import type { Names } from "./field-map.js";

type Kind = "tasks" | "links";
type Row = Record<string, unknown>;

/** The names of the custom fields seen, for tasks and for links. */
export type CustomNames = Readonly<{ tasks: readonly string[]; links: readonly string[] }>;

/** `rows` without their custom members, and `commit`, which keeps those members once the chart took the rows. */
export interface Taken<T> {
  rows: T[] | undefined;
  commit(): void;
}

export interface CustomFields {
  /** Take the custom members out of the host's `list` of rows or `update` records (a row with no text key keeps them, so `fromTasks` counts them in `leftOut` as before); `list` itself when there are none. */
  take<T>(kind: Kind, list: readonly T[] | undefined): Taken<T>;
  /** The changes with the custom members of a created or deleted record, under the host's names; an update has none. */
  add(changes: readonly Change[]): Change[];
  /** The names seen: the same object until a new one is seen. */
  names(): CustomNames;
  /** Forget every row and name: a file was opened, so its keys are not the rows these members belong to. */
  clear(): void;
}

const isRow = (value: unknown): value is Row => typeof value === "object" && value !== null && !Array.isArray(value);

/** The custom fields of one chart; `maps` is how its host names become the chart's (`FieldMap`). */
export function customFields(maps: { tasks: Pick<Names<object>, "toChart">; links: Pick<Names<object>, "toChart"> }): CustomFields {
  const held = { tasks: new Map<string, Row>(), links: new Map<string, Row>() };
  const seen = { tasks: new Set<string>(), links: new Set<string>() };
  const known = { tasks: new Set([...KNOWN_TASK, "hostStart", "hostFinish"]), links: KNOWN_LINK }; // the two dates only `update` takes
  const cache = { tasks: new Map<string, boolean>(), links: new Map<string, boolean>() };
  const named = (tasks: string[], links: string[]): CustomNames => Object.freeze({ tasks: Object.freeze(tasks), links: Object.freeze(links) });
  let listed = named([], []);
  /** A host member is custom unless the map turns it into a member the chart reads: its own name for a mapped field is not one. */
  const isCustom = (kind: Kind, name: string): boolean => {
    let answer = cache[kind].get(name);
    if (answer === undefined) {
      const [chart] = Object.keys(maps[kind].toChart({ [name]: 0 }) as Row);
      answer = chart === undefined || !(known[kind].has(chart) || chart.startsWith("$"));
      cache[kind].set(name, answer);
    }
    return answer;
  };
  return {
    take<T>(kind: Kind, list: readonly T[] | undefined): Taken<T> {
      const keep: [string, Row][] = [];
      const rows = list?.map((item) => {
        if (!isRow(item) || !Object.keys(item).some((name) => isCustom(kind, name))) return item;
        const { key } = maps[kind].toChart(item) as Row;
        if (typeof key !== "string") return item;
        const mine = Object.fromEntries(Object.entries(item).filter(([name, value]) => value !== undefined && isCustom(kind, name)));
        keep.push([key, mine]);
        return Object.fromEntries(Object.entries(item).filter(([name]) => !isCustom(kind, name))) as T;
      });
      return {
        rows: keep.length === 0 ? (list as T[] | undefined) : rows,
        commit() {
          for (const [key, mine] of keep) {
            held[kind].set(key, { ...held[kind].get(key), ...mine });
            for (const name of Object.keys(mine)) seen[kind].add(name);
          }
          if (seen.tasks.size !== listed.tasks.length || seen.links.size !== listed.links.length) listed = named([...seen.tasks], [...seen.links]);
        },
      };
    },
    add: (changes) => changes.map((change) => {
      const kind = change.record === "task" ? "tasks" : change.record === "link" ? "links" : undefined;
      const mine = kind === undefined || change.key === null ? undefined : held[kind].get(change.key);
      if (mine === undefined) return change;
      if (change.op === "create" && change.set) return { ...change, set: { ...mine, ...change.set } };
      return change.op === "delete" && change.was ? { ...change, was: { ...mine, ...change.was } } : change;
    }),
    names: () => listed,
    clear() {
      for (const kind of ["tasks", "links"] as const) [held[kind], seen[kind]].forEach((store) => store.clear());
      if (listed.tasks.length + listed.links.length > 0) listed = named([], []);
    },
  };
}
