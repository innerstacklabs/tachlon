// The calendars and the duration unit of plain data (tl#2081, decision 0017 rules 5 and 6): which weekdays are worked and how long, the holidays and extra
// working days, the unit a duration or a lag is written in, and the calendar a lag is counted on. `validate` has already checked every member read here.
import { DAY, WORKING_DAY, WORKING_WEEK, minutesOf } from "./dhtmlx.ts";
import type { PlainCalendar } from "./plain.ts";

type Week = [number, number][][];
type Unit = "days" | "hours" | "minutes";
type Lag = "successor" | "predecessor" | "24h" | { calendar: string };

/** A calendar as the conversion reads it: its `week`, its `day` (the average working day in minutes, `day_minutes` in `crates/app/src/edit/duration.rs`), its `key` and `exceptions`. */
export interface ReadCalendar { week: Week; day: number; key?: string; exceptions: NonNullable<PlainCalendar["exceptions"]> }

/** The calendars of the data (the one Monday to Friday calendar, with no key, when it gives none or an empty list) and the position of each key. `workDays` with `hoursPerDay` is one period from 08:00 on each of those days. */
export function readCalendars(calendars: PlainCalendar[] | undefined): { list: ReadCalendar[]; at: Map<string, number> } {
  const list = calendars === undefined || calendars.length === 0 ? [{ week: WORKING_WEEK, day: WORKING_DAY, exceptions: [] }] : calendars.map((c): ReadCalendar => {
    const week = c.week ?? WORKING_WEEK.map((_, i): [number, number][] => (c.workDays!.includes(i + 1) ? [[480, 480 + Math.round(c.hoursPerDay! * 60)]] : []));
    const worked = week.filter((periods) => periods.length > 0);
    const total = worked.reduce((sum, periods) => sum + periods.reduce((day, [from, to]) => day + to - from, 0), 0);
    return { week, day: Math.max(1, Math.floor(total / worked.length)), key: c.key, exceptions: c.exceptions ?? [] };
  });
  return { list, at: new Map(list.flatMap((c, i) => (c.key === undefined ? [] : [[c.key, i] as const]))) };
}

/** `value` in working minutes: `unit` days count `day` minutes each, hours 60, minutes 1; a part of a minute rounds to the nearest. */
export function toMinutes(value: number, unit: Unit | undefined, day: number): number {
  return Math.round(value * (unit === "hours" ? 60 : unit === "minutes" ? 1 : day));
}

/** The minutes of a lag day for a link from a task on calendar `pred` to one on calendar `succ`: the successor's by default, the predecessor's, a full 24 hours, or that of the calendar the key names. */
export function lagDay(lag: Lag | undefined, calendars: { list: ReadCalendar[]; at: Map<string, number> }, pred: number, succ: number): number {
  if (lag === "24h") return DAY;
  return calendars.list[typeof lag === "object" ? calendars.at.get(lag.calendar)! : lag === "predecessor" ? pred : succ]!.day;
}

/** The document's `lag_calendar`: the three words as they are, a key as the calendar's position. */
export function lagCalendar(lag: Lag | undefined, at: Map<string, number>): "successor" | "predecessor" | "24h" | { calendar: number } {
  return typeof lag === "object" ? { calendar: at.get(lag.calendar)! } : lag ?? "successor";
}

/** The exceptions of `calendar` as `[day from the origin, working periods]`: none for a holiday, else the first worked weekday's. `before` counts those that fall before the origin, which are left out. */
export function exceptionsOf(calendar: ReadCalendar, origin: number): { list: [number, [number, number][]][]; before: number } {
  const periods = calendar.week.find((day) => day.length > 0)!;
  const days = calendar.exceptions.map(({ date, working }) => [Math.floor(minutesOf(date)! / DAY) - origin, working ? periods : []] as [number, [number, number][]]);
  return { list: days.filter(([day]) => day >= 0), before: days.filter(([day]) => day < 0).length };
}
