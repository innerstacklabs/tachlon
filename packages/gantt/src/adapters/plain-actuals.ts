import { DAY, WORKING_WEEK, minutesOf } from "./dhtmlx.ts";

/** Minutes since 1970 of the date `text`. A date alone is read by the kind of its field (decision 0017, rule 6): `resume` and `dataDate` are 00:00 of the day, an actual start the first working minute of the day, an actual finish the end of its last working minute (00:00 and 24:00 on a day with no working time), on the task's `week` (`WORKING_WEEK` by default). */
export function dateAt(text: string, edge?: "start" | "finish", week = WORKING_WEEK): number {
  const at = minutesOf(text)!;
  if (edge === undefined || text.length > 10) return at;
  const periods = week[(((Math.floor(at / DAY) + 3) % 7) + 7) % 7]!;
  const [from, to] = periods.length === 0 ? [0, DAY] : [periods[0]![0], periods[periods.length - 1]![1]];
  return at + (edge === "start" ? from : to);
}

/** The progress keys of `task` (checked by `validate`) as minutes since 1970 and `remaining` in minutes (`minutes` converts a duration), each null when not given; `dates` holds every date, a date alone read on the task's `week`. */
export function readActuals(task: Record<string, unknown>, week: [number, number][][], minutes: (value: number) => number) {
  const read = (key: string, edge?: "start" | "finish") => (typeof task[key] === "string" ? dateAt(task[key], edge, week) : null);
  const [start, finish, resume] = [read("actualStart", "start"), read("actualFinish", "finish"), read("resume")];
  const remaining = typeof task.remaining === "number" ? minutes(task.remaining) : null;
  return { start, finish, resume, remaining, dates: [start, finish, resume].filter((at): at is number => at !== null) };
}
