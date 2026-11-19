// `Gantt.update` (gantt#2087, decision 0019 rule 13): the host gives the chart new values for many tasks by key, applied in place as one call.
// A `duration` is in the unit the host mounted with (decision 0017 rule 5); this is the one place it becomes the app's working minutes.
import { readCalendars, toMinutes } from "./adapters/plain-calendars.js";
import type { PublicApp } from "./api.js";
import { readAnswer, refused } from "./edit-result.js";
import type { HostDifference, MountOptions, UpdateResult } from "./gantt-types.js";

/** A duration of a record that holds one. */
const hasDuration = (record: unknown): record is { duration: number } => typeof (record as { duration?: unknown } | null)?.duration === "number";

/** The working day in minutes of the task with each key, from the saved document: its own calendar's, as the plain-data reader counts a day. */
function dayOfKey(app: PublicApp): (key: unknown) => number {
  const saved = JSON.parse(app.save("update")) as { calendars: Parameters<typeof readCalendars>[0]; tasks: { key?: string; calendar: number }[] };
  const { list } = readCalendars(saved.calendars);
  const days = new Map(saved.tasks.flatMap((task) => (task.key === undefined ? [] : [[task.key, list[task.calendar]!.day] as const])));
  return (key) => days.get(key as string) ?? list[0]!.day;
}

/** Answer what `app.update_tasks` answers for `records`; `busy` is `Gantt.busy()`, `unit` the unit of the host's durations, and `wrote` is told the slots of the tasks written (the table's). */
export function update(app: PublicApp, busy: () => boolean, unit: MountOptions["durationUnit"], records: unknown, wrote: (slots: readonly number[]) => void): UpdateResult {
  if (!Array.isArray(records)) return Object.freeze({ ok: false, reason: "badRecords", message: "update takes an array of records." });
  if (busy()) return refused("busy");
  const day = unit === "hours" || unit === "minutes" || !records.some(hasDuration) ? () => 0 : dayOfKey(app);
  const answer = readAnswer(app.update_tasks(JSON.stringify(records.map((record) => (hasDuration(record) ? { ...record, duration: toMinutes(record.duration, unit, day((record as { key?: unknown }).key)) } : record)))));
  if (answer.ok) {
    wrote(JSON.parse(app.task_slots(records.map((record) => (record as { key: string }).key))) as number[]); // an accepted record names a live task by its key
    const { differs } = answer as { differs?: readonly HostDifference[] };
    return Object.freeze({ ok: true, updated: answer.updated!, ...(differs === undefined ? {} : { differs }) });
  }
  const key = (records[answer.record ?? -1] as { key?: unknown } | undefined)?.key;
  return Object.freeze({ ...answer, message: `Nothing changed: ${answer.reason}${answer.record === undefined ? "" : ` in record ${answer.record}`}${answer.member === undefined ? "" : ` (${answer.member})`}.`, ...(typeof key === "string" ? { key } : {}) });
}
