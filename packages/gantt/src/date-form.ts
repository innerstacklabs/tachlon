// Under `dateForm: "day"` a host writes a date as the chart wrote it, `YYYY-MM-DD` (decision 0017, rule 7): a text date with a time of day is refused before the app is called.
import type { MountOptions, UpdateResult } from "./gantt-types.js";

/** The members of a record, in the chart's own names, that hold a date. */
const DATES = ["start", "actualStart", "actualFinish", "snet", "snlt", "fnet", "fnlt", "mso", "mfo", "hostStart", "hostFinish", "hostLateStart", "hostLateFinish"];

/** The `badValue` answer for the first record of `records` (with a typed `constraint` already spelled as members) that gives a text date with a time of day under `"day"`; other types are left to the app. */
export function timedDate(dateForm: MountOptions["dateForm"], records: readonly unknown[]): UpdateResult | undefined {
  if (dateForm !== "day") return undefined;
  for (const [record, row] of records.entries()) {
    const found = typeof row === "object" && row !== null ? Object.entries(row).find(([name, value]) => DATES.includes(name) && typeof value === "string" && value.includes("T")) : undefined;
    if (found === undefined) continue;
    const [member, value] = found as [string, string];
    const key = (row as { key?: unknown }).key;
    return Object.freeze({ ok: false, reason: "badValue", record, member, message: `Nothing changed: ${typeof key === "string" ? `task ${key}` : `record ${record}`} gives ${member} as "${value}"; this chart mounted with dateForm: "day", which takes a date as YYYY-MM-DD.`, ...(typeof key === "string" ? { key } : {}) });
  }
  return undefined;
}
