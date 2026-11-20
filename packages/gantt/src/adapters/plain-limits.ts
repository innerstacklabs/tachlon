// The limit keys of a plain task (tl#2081): `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo` (each a date), `alap: true` and `milestone`. Their names are
// the DHTMLX constraint types, so the one `LIMIT_KEYS` table of `dhtmlx.ts` says which document limit each sets, and the two paths cannot drift.
import { DAY, LIMIT_KEYS, minutesOf, type DocumentLimit } from "./dhtmlx.ts";
import type { ReadCalendar } from "./plain-calendars.ts";

/** The finish limits: a date alone on one is the end of the last working minute of that day on the task's calendar, and on any other limit the first working minute (decision 0017, rule 6; `day_edge` in `crates/app/src/host/read.rs`). */
const FINISH = new Set(["fnet", "fnlt", "mfo"]);

/** The working periods of `calendar` on the day `day` (days since 1970): its exception for the date, else its weekday's (1970-01-01 was a Thursday). */
function periodsOn(calendar: ReadCalendar, day: number): [number, number][] {
  const exception = calendar.exceptions.find(({ date }) => Math.floor(minutesOf(date)! / DAY) === day);
  if (exception !== undefined) return exception.working ? calendar.week.find((periods) => periods.length > 0)! : [];
  return calendar.week[(((day + 3) % 7) + 7) % 7]!;
}

/** Minutes since 1970 of the limit `text`: a time is read as it stands, a date alone as the first working minute of the day (the end of the last for a `finish` limit) on `calendar`, 00:00 (24:00) when the day has no working time. */
function limitAt(text: string, finish: boolean, calendar: ReadCalendar): number {
  const at = minutesOf(text)!;
  if (text.length > 10) return at;
  const periods = periodsOn(calendar, Math.floor(at / DAY));
  if (periods.length === 0) return at + (finish ? DAY : 0);
  return at + (finish ? periods[periods.length - 1]![1] : periods[0]![0]);
}

/** What the limit keys of `task` give, read on its `calendar`: `snet`, the start floor in place of `start`; the document `limit`; and `dates`, every date they hold (minutes since 1970). `validate` has already checked the dates. */
export function readLimits(task: Record<string, unknown>, calendar: ReadCalendar): { snet: number | null; limit?: DocumentLimit; dates: number[] } {
  const dates: number[] = [];
  const read = (key: string): number | null => {
    const text = task[key];
    if (typeof text !== "string") return null;
    const at = limitAt(text, FINISH.has(key), calendar);
    dates.push(at);
    return at;
  };
  const limit: DocumentLimit = task.alap === true ? { alap: true } : {};
  if (task.milestone === "start" || task.milestone === "finish") limit.milestone = task.milestone;
  for (const [key, field] of Object.entries(LIMIT_KEYS)) {
    const at = read(key);
    if (at !== null) limit[field] = at;
  }
  return { snet: read("snet"), dates, ...(Object.keys(limit).length === 0 ? {} : { limit }) };
}

/** `limit` with its dates counted from `origin` (a day number) instead of 1970. */
export function shiftLimit(limit: DocumentLimit, origin: number): DocumentLimit {
  return Object.fromEntries(Object.entries(limit).map(([key, value]) => [key, typeof value === "number" ? value - origin * DAY : value])) as DocumentLimit;
}
