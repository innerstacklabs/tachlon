// The unit and the date form a host mounted with, applied to the changes it is told of (decision 0022, rules 2.3 and 2.4; decision 0017, rules 5 and 7): the engine writes a duration or a lag as days
// of the calendar that measures it and a date as `YYYY-MM-DDTHH:mm`, and a host that mounted with `durationUnit` or `dateForm` gets those two in its own form.
import type { Change } from "./events.js";
import type { Stamped } from "./change-set.js";
import type { MountOptions } from "./gantt-types.js";

const SPANS = new Set(["duration", "remaining"]);
const isSpan = (change: Change, name: string): boolean => (change.record === "link" ? name === "lag" : SPANS.has(name));
const STARTS = new Set(["snet", "snlt", "mso", "resume", "actualStart", "dataDate"]);
const FINISHES = new Set(["actualFinish", "fnet", "fnlt", "mfo", "projectFinish"]);

/** A date as its day, `YYYY-MM-DD`: a start is its own day, a finish at `T00:00` is the day before. */
export function dayOf(value: string, finishing: boolean): string {
  return finishing && value.endsWith("T00:00") ? new Date(Date.parse(`${value.slice(0, 10)}T00:00:00Z`) - 86_400_000).toISOString().slice(0, 10) : value.slice(0, 10);
}

/** One member's value in the host's form: a duration or lag in `unit`, a date in `dateForm`; any other value as it is. A finish-kind date is the day of its instant less one minute. */
function formOf(change: Stamped, name: string, value: unknown, unit: MountOptions["durationUnit"], dateForm: MountOptions["dateForm"]): unknown {
  if (unit !== undefined && unit !== "days" && typeof value === "number" && change.dayMinutes !== undefined && isSpan(change, name)) {
    const minutes = Math.round(value * change.dayMinutes);
    return unit === "hours" ? minutes / 60 : minutes;
  }
  if (name === "baseline" && value !== null && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([held, v]) => [held, formOf(change, held, v, unit, dateForm)]));
  if (dateForm === "day" && typeof value === "string" && change.record !== "link" && (STARTS.has(name) || FINISHES.has(name))) return dayOf(value, FINISHES.has(name));
  return value;
}

/**
 * `changes` with every duration and lag in `unit` and every date in `dateForm`. The app stamps each change with `dayMinutes`, the working day of the calendar that measures its
 * duration (its task's) or lag (its link's lag calendar), where it makes the change; that number is the whole of what a duration is measured by, and it is removed here.
 */
export function formChanges(changes: readonly Stamped[], unit: MountOptions["durationUnit"], dateForm: MountOptions["dateForm"]): Change[] {
  const form = (change: Stamped, members: Record<string, unknown> | undefined): Record<string, unknown> | undefined =>
    members && Object.fromEntries(Object.entries(members).map(([name, value]) => [name, formOf(change, name, value, unit, dateForm)]));
  return changes.map((change) => {
    const out: Stamped = { ...change, ...(change.set ? { set: form(change, change.set) } : {}), ...(change.was ? { was: form(change, change.was) } : {}) };
    delete out.dayMinutes;
    return out;
  });
}
