// `Gantt.update` (gantt#2087, decision 0019 rule 13): the host gives the chart new values for many tasks by key, applied in place as one call.
// A `duration` is in the unit the host mounted with (decision 0017 rule 5); this is the one place it becomes the app's working minutes.
import { readCalendars, toMinutes } from "./adapters/plain-calendars.js";
import type { PublicApp } from "./api.js";
import { toSeparate } from "./constraint-form.js";
import { timedDate } from "./date-form.js";
import { readAnswer, refused, type Refusal } from "./edit-result.js";
import type { HostDifference, MountOptions, UpdateResult } from "./gantt-types.js";

/** The members that count working time in the host's unit. */
const TIMED = ["duration", "remaining"] as const;
/** A record that gives a number for one of them. */
const hasTime = (record: unknown): record is Partial<Record<(typeof TIMED)[number], number>> => TIMED.some((member) => typeof (record as Record<string, unknown> | null)?.[member] === "number");

/** `Gantt.pushHostEntry`: a `token` that is not text reads as empty, which the app refuses as `badToken`. */
export const pushHostEntry = (app: PublicApp, token: string): { ok: true } | Refusal => readAnswer(app.try_push_host_entry(typeof token === "string" ? token : "")) as { ok: true } | Refusal;

/** The working day in minutes of the task with each key, from the saved document: its own calendar's, as the plain-data reader counts a day. */
function dayOfKey(app: PublicApp): (key: unknown) => number {
  const saved = JSON.parse(app.save("update")) as { calendars: Parameters<typeof readCalendars>[0]; tasks: { key?: string; calendar: number }[] };
  const { list } = readCalendars(saved.calendars);
  const days = new Map(saved.tasks.flatMap((task) => (task.key === undefined ? [] : [[task.key, list[task.calendar]!.day] as const])));
  return (key) => days.get(key as string) ?? list[0]!.day;
}

/** Answer what `app.update_tasks` (or `app.host_edit`, the same records as one undo step) answers for `records`; `busy` is `Gantt.busy()`, `unit` the unit of the host's durations, `dateForm` the form of its dates (under `"day"` the app reads a date alone on the task's calendar and a date with a time of day is `badValue`), and `wrote` is told the slots of the tasks written (the table's). */
export function update(app: PublicApp, busy: () => boolean, unit: MountOptions["durationUnit"], dateForm: MountOptions["dateForm"], records: unknown, wrote: (slots: readonly number[]) => void, call: "update_tasks" | "host_edit" = "update_tasks"): UpdateResult {
  if (!Array.isArray(records)) return Object.freeze({ ok: false, reason: "badRecords", message: "update takes an array of records." });
  if (busy()) return refused("busy");
  let separate: unknown[];
  try { separate = records.map((record) => (typeof record === "object" && record !== null && !Array.isArray(record) ? toSeparate(record as Record<string, unknown>, true) : record)); } catch (err) { return Object.freeze({ ok: false, reason: "badRecords", message: `Nothing changed: ${(err as Error).message}` }); }
  const timed = timedDate(dateForm, separate);
  if (timed !== undefined) return timed;
  const day = unit === "hours" || unit === "minutes" || !records.some(hasTime) ? () => 0 : dayOfKey(app);
  const inMinutes = (record: unknown): unknown => (hasTime(record) ? { ...record, ...Object.fromEntries(TIMED.flatMap((member) => (typeof record[member] === "number" ? [[member, toMinutes(record[member], unit, day((record as { key?: unknown }).key))]] : []))) } : record);
  const answer = readAnswer(app[call](JSON.stringify(separate.map(inMinutes))));
  if (answer.ok) {
    wrote(JSON.parse(app.task_slots(records.map((record) => (record as { key: string }).key))) as number[]); // an accepted record names a live task by its key
    const { differs } = answer as { differs?: readonly HostDifference[] };
    return Object.freeze({ ok: true, updated: answer.updated!, ...(differs === undefined ? {} : { differs }) });
  }
  const key = (records[answer.record ?? -1] as { key?: unknown } | undefined)?.key;
  const undo = ({ hasActuals: "has actual dates, or the record sets them", notAllowed: `cannot be put back by an Undo under the host's capability profile (${answer.member})` } as Record<string, string>)[answer.reason];
  const why = undo !== undefined ? `task ${String(key)} ${undo}, so hostEdit cannot take it; call update, which sets it without an undo step` :`${answer.reason}${answer.record === undefined ? "" : ` in record ${answer.record}`}${answer.member === undefined ? "" : ` (${answer.member})`}`;
  return Object.freeze({ ...answer, message: `Nothing changed: ${why}.`, ...(typeof key === "string" ? { key } : {}) });
}
