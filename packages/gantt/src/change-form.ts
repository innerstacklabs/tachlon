// The unit and the date form a host mounted with, applied to the changes it is told of (decision 0022, rules 2.3 and 2.4; decision 0017, rules 5 and 7): the engine writes a duration or a lag as days
// of the calendar that measures it and a date as `YYYY-MM-DDTHH:mm`, and a host that mounted with `durationUnit` or `dateForm` gets those two in its own form.
import type { PublicApp } from "./api.js";
import type { Change } from "./events.js";
import type { MountOptions } from "./gantt-types.js";

/** The working day in minutes that measures a change's duration (its task's calendar) or lag (its link's lag calendar). */
export type DayOf = (change: Change) => number;

const SPANS = new Set(["duration", "remaining"]);
const isSpan = (change: Change, name: string): boolean => (change.record === "link" ? name === "lag" : SPANS.has(name));
const STARTS = new Set(["snet", "snlt", "mso", "resume", "actualStart", "dataDate"]);
const FINISHES = new Set(["actualFinish", "fnet", "fnlt", "mfo"]);

/**
 * The working day of every change, by the session slot it carries (`task` or `link`), read from `App.day_lengths` once. A deleted row keeps its slot, so a delete, a cascade link delete and the undo
 * of either are measured as the row was. A link a before-edit call plans to create has no slot yet: its day is the schedule's lag calendar's, or its successor's or predecessor's, found by the key
 * or the `{ task }` slot in its `to` or `from`.
 */
export function readDays(app: PublicApp): DayOf {
  const { tasks, links, keys, lag } = JSON.parse(app.day_lengths()) as { tasks: number[]; links: number[]; keys: (string | null)[]; lag: number | "successor" | "predecessor" };
  const first = tasks[0] ?? 1440;
  const end = (ref: unknown): number => tasks[typeof ref === "string" ? keys.indexOf(ref) : (ref as { task: number }).task] ?? first;
  return (change) => {
    if (change.record !== "link") return tasks[change.task as number] ?? first;
    const members = change.set ?? change.was;
    return links[change.link as number] ?? (typeof lag === "number" ? lag : end(members?.[lag === "successor" ? "to" : "from"]));
  };
}

/** One member's value in the host's form: a duration or lag in `unit`, a date in `dateForm`; any other value as it is. A finish-kind date is the day of its instant less one minute. */
function formOf(change: Change, name: string, value: unknown, unit: MountOptions["durationUnit"], dateForm: MountOptions["dateForm"], day: DayOf): unknown {
  if (unit !== undefined && unit !== "days" && typeof value === "number" && isSpan(change, name)) {
    const minutes = Math.round(value * day(change));
    return unit === "hours" ? minutes / 60 : minutes;
  }
  if (dateForm === "day" && typeof value === "string" && change.record !== "link" && (STARTS.has(name) || FINISHES.has(name))) {
    const finishing = FINISHES.has(name) && value.endsWith("T00:00");
    return finishing ? new Date(Date.parse(`${value.slice(0, 10)}T00:00:00Z`) - 86_400_000).toISOString().slice(0, 10) : value.slice(0, 10);
  }
  return value;
}

/** `changes` with every duration and lag in `unit` and every date in `dateForm` (`day` measures a duration or lag, and is asked only for one that is not in days). */
export function formChanges(changes: readonly Change[], unit: MountOptions["durationUnit"], dateForm: MountOptions["dateForm"], day: DayOf): Change[] {
  const form = (change: Change, members: Record<string, unknown> | undefined): Record<string, unknown> | undefined =>
    members && Object.fromEntries(Object.entries(members).map(([name, value]) => [name, formOf(change, name, value, unit, dateForm, day)]));
  return changes.map((change) => ({ ...change, ...(change.set ? { set: form(change, change.set) } : {}), ...(change.was ? { was: form(change, change.was) } : {}) }));
}

export interface ChangeForm {
  /** Each list of changes in the host's form. `fresh` (the `edit` event) reads the app's day lengths again, once, when a change holds a duration or a lag. */
  lists(lists: readonly (readonly Change[])[], fresh: boolean): Change[][];
  /** Read the day lengths again: the schedule was replaced (an `open` event). */
  reread(): void;
}

/**
 * The host's form for a chart's changes. A before-edit call is made inside the app, which it must not call, so it keeps the last read of the day lengths: `early` makes it at the start
 * (a chart with no before-edit function never needs it), and `reread` makes it again when a load, open or new schedule replaces the one it described.
 */
export function changeForm(app: PublicApp, unit: MountOptions["durationUnit"], dateForm: MountOptions["dateForm"], early: boolean): ChangeForm {
  const measured = unit !== undefined && unit !== "days";
  let day: DayOf = measured && early ? readDays(app) : () => 0;
  const holdsSpan = (change: Change): boolean => Object.keys({ ...change.set, ...change.was }).some((name) => isSpan(change, name));
  return {
    lists(lists, fresh) {
      if (fresh && measured && lists.some((changes) => changes.some(holdsSpan))) day = readDays(app);
      return lists.map((changes) => formChanges(changes, unit, dateForm, day));
    },
    reread() {
      if (measured && early) day = readDays(app);
    },
  };
}
