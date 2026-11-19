// The unit and the date form a host mounted with, applied to the changes it is told of (decision 0022, rules 2.3 and 2.4; decision 0017, rules 5 and 7): the engine writes a duration or a lag as days
// of the calendar that measures it and a date as `YYYY-MM-DDTHH:mm`, and a host that mounted with `durationUnit` or `dateForm` gets those two in its own form.
import { DAY } from "./adapters/dhtmlx.ts"; // `.ts`, so the Node tier can load this file; the types come from the `.js` names, which the declarations follow
import { readCalendars } from "./adapters/plain-calendars.ts";
import type { DocumentLagCalendar } from "./adapters/dhtmlx.js";
import type { PublicApp } from "./api.js";
import type { Change } from "./events.js";
import type { MountOptions } from "./gantt-types.js";

/** The working day in minutes that measures a change's duration (its task's calendar) or lag (its link's lag calendar). */
export type DayOf = (change: Change) => number;

const SPANS = new Set(["duration", "remaining"]);
const isSpan = (change: Change, name: string): boolean => (change.record === "link" ? name === "lag" : SPANS.has(name));
const STARTS = new Set(["snet", "snlt", "mso", "resume", "actualStart", "dataDate"]);
const FINISHES = new Set(["actualFinish", "fnet", "fnlt", "mfo"]);

interface Saved {
  calendars: Parameters<typeof readCalendars>[0];
  lag_calendar: DocumentLagCalendar;
  tasks: { key?: string; calendar: number }[];
  links: { key?: string; pred: number; succ: number; lag_calendar: DocumentLagCalendar | null }[];
}

/** The working day of every change, read from the saved document once: a row's own `calendar` and `lagCalendar` first (a create or a delete holds them), else the row's place in the document, else the first calendar's. */
export function readDays(app: PublicApp): DayOf {
  const saved = JSON.parse(app.save("update")) as Saved;
  const { list, at } = readCalendars(saved.calendars);
  const byKey = <T extends { key?: string }>(rows: T[]): Map<string, T> => new Map(rows.flatMap((row) => (row.key === undefined ? [] : [[row.key, row] as const])));
  const [tasks, links] = [byKey(saved.tasks), byKey(saved.links)];
  const place = (ref: unknown): number | undefined => (typeof ref === "string" ? at.get(ref) : typeof ref === "number" ? ref : (ref as { calendar?: unknown } | null)?.calendar === undefined ? undefined : place((ref as { calendar: unknown }).calendar));
  const day = (calendar: number | undefined): number => (list[calendar ?? 0] ?? list[0]!).day;
  return (change) => {
    const members: Record<string, unknown> = change.set ?? change.was ?? {};
    if (change.record !== "link") return day(place(members.calendar) ?? tasks.get(change.key as string)?.calendar);
    const link = links.get(change.key as string);
    const lag = (members.lagCalendar ?? link?.lag_calendar ?? saved.lag_calendar) as DocumentLagCalendar;
    if (lag === "24h") return DAY;
    if (typeof lag === "object") return day(place(lag));
    const [end, slot] = lag === "predecessor" ? [members.from, link?.pred] : [members.to, link?.succ];
    return day(typeof end === "string" ? tasks.get(end)?.calendar : slot === undefined ? undefined : saved.tasks[slot]?.calendar);
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

/** `changes` with every duration and lag in `unit` and every date in `dateForm` (`day` measures a duration or lag from the saved document, and is asked only for one that is not in days). */
export function formChanges(changes: readonly Change[], unit: MountOptions["durationUnit"], dateForm: MountOptions["dateForm"], day: DayOf): Change[] {
  const form = (change: Change, members: Record<string, unknown> | undefined): Record<string, unknown> | undefined =>
    members && Object.fromEntries(Object.entries(members).map(([name, value]) => [name, formOf(change, name, value, unit, dateForm, day)]));
  return changes.map((change) => ({ ...change, ...(change.set ? { set: form(change, change.set) } : {}), ...(change.was ? { was: form(change, change.was) } : {}) }));
}

/**
 * The function that puts a commit's changes in the host's form. `fresh` (the `edit` event) reads the saved document again when a change holds a duration or a lag; a before-edit call is made inside the app,
 * which it must not call, so it keeps the last read, which `early` makes at the start (a chart with no before-edit function never needs it).
 */
export function changeForm(app: PublicApp, unit: MountOptions["durationUnit"], dateForm: MountOptions["dateForm"], early: boolean): (changes: readonly Change[], fresh: boolean) => Change[] {
  const measured = unit !== undefined && unit !== "days";
  let day: DayOf = measured && early ? readDays(app) : () => 0;
  const holdsSpan = (change: Change): boolean => Object.keys({ ...change.set, ...change.was }).some((name) => isSpan(change, name));
  return (changes, fresh) => {
    if (fresh && measured && changes.some(holdsSpan)) day = readDays(app);
    return formChanges(changes, unit, dateForm, day);
  };
}
