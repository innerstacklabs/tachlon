// The limit keys of a plain task (tl#2081): `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo` (each a date), `alap: true` and `milestone`. Their names are
// the DHTMLX constraint types, so the one `LIMIT_KEYS` table of `dhtmlx.ts` says which document limit each sets, and the two paths cannot drift.
import { DAY, LIMIT_KEYS, minutesOf, type DocumentLimit } from "./dhtmlx.ts";

/** The finish limits: a date alone on one is the end of that day, as the limit editor reads it (`plan_limit_text` in `crates/app/src/edit/limit.rs`). */
const FINISH = new Set(["fnet", "fnlt", "mfo"]);

/** What the limit keys of `task` give: `snet`, the start floor in place of `start`; the document `limit`; and `dates`, every date they hold (minutes since 1970). `validate` has already checked the dates. */
export function readLimits(task: Record<string, unknown>): { snet: number | null; limit?: DocumentLimit; dates: number[] } {
  const dates: number[] = [];
  const read = (key: string): number | null => {
    const text = task[key];
    if (typeof text !== "string") return null;
    const at = minutesOf(text)! + (FINISH.has(key) && text.length === 10 ? DAY : 0);
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
