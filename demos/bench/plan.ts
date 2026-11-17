// The dates of a benchmark schedule, worked out the way our engine works them out, so a side that
// draws given dates is handed the dates our chart computes.
//
// One calendar: Monday to Friday, 08:00 to 16:00, the "Standard" calendar `to-mspdi.ts` writes
// (and Bryntum's adapter). Time is counted in working minutes from the project start (a working
// day, 08:00), with the one rule of the engine's (`crates/engine/src/cpm/admissible.rs`) that
// minutes alone lose: the end of a working day (16:00) comes before the next day's 08:00. So a
// point is a key: 2m for the working minute m, and 2m - 1 for the day end just before it.
// A forward pass over the links (lag 0) gives each task its earliest start: no earlier than the
// project start and its own `start` (start-no-earlier-than, 08:00 on a bare date), and
// - FS: no earlier than the predecessor's finish;
// - SS: no earlier than the predecessor's start;
// - FF: so that it finishes no earlier than the predecessor's finish;
// - SF: so that it finishes no earlier than the predecessor's start.
// A task with work finishes at 16:00 when its work ends with the day and starts on a working minute
// (08:00, not the 16:00 before it); a milestone (no work) may sit on a day end. So an FF or SF
// bound at 08:00 is met by a finish at 08:01, not by 16:00 the day before (the engine's
// start-to-finish edge case in docs/MEASURING.md, "The Bryntum side").

import type { DatasetLink, DatasetTask } from "../dhtmlx-compare/to-mspdi.ts";

export const DAY_MINUTES = 480;
const DAY_START_MINUTE = 8 * 60;
const MS_PER_DAY = 86_400_000;

const dayNumber = (date: string) => Date.parse(`${date}T00:00:00Z`) / MS_PER_DAY;
/** Monday 0 to Sunday 6, for a YYYY-MM-DD date. */
const weekday = (date: string) => (new Date(`${date}T00:00:00Z`).getUTCDay() + 6) % 7;
const two = (n: number) => String(n).padStart(2, "0");

/** True on Monday to Friday. */
export const isWorkingDay = (date: string) => weekday(date) < 5;

/** The working minute `minutes` after `start` 08:00, as YYYY-MM-DDTHH:MM. */
export function stampAt(start: string, minutes: number): string {
  if (!isWorkingDay(start)) throw new Error(`the project start ${start} is not a working day`);
  const s = weekday(start);
  const total = s + Math.floor(minutes / DAY_MINUTES);
  const days = Math.floor(total / 5) * 7 + (total % 5) - s;
  const at = DAY_START_MINUTE + (minutes % DAY_MINUTES);
  const date = new Date((dayNumber(start) + days) * MS_PER_DAY).toISOString().slice(0, 10);
  return `${date}T${two(Math.floor(at / 60))}:${two(at % 60)}`;
}

/** A key as YYYY-MM-DDTHH:MM: a day end is 16:00 of the day it ends. */
export function stampKey(start: string, key: number): string {
  if (key % 2 === 0) return stampAt(start, key / 2);
  return `${stampAt(start, (key + 1) / 2 - DAY_MINUTES).slice(0, 11)}16:00`;
}

/** The key of a start-no-earlier-than at `when` (YYYY-MM-DD, as 08:00, or YYYY-MM-DDTHH:MM). */
export function keyOf(start: string, when: string): number {
  const date = when.slice(0, 10);
  let clock = when.length > 10 ? Number(when.slice(11, 13)) * 60 + Number(when.slice(14, 16)) : DAY_START_MINUTE;
  const days = dayNumber(date) - dayNumber(start);
  if (!Number.isInteger(days) || days < 0) return 0;
  let work = Math.floor(days / 7) * 5;
  for (let i = 0; i < days % 7; i++) if ((weekday(start) + i) % 7 < 5) work++;
  if (!isWorkingDay(date)) clock = DAY_START_MINUTE; // the next working day's 08:00
  const into = Math.min(Math.max(clock - DAY_START_MINUTE, 0), DAY_MINUTES);
  const m = work * DAY_MINUTES + into;
  return into === DAY_MINUTES ? 2 * m - 1 : 2 * m;
}

/** The task's duration in working minutes; throws unless it is a whole number of minutes, 0 or more. */
export function workMinutes(t: DatasetTask): number {
  const m = t.duration * DAY_MINUTES;
  if (typeof t.duration !== "number" || !Number.isInteger(m) || m < 0) {
    throw new Error(`task ${t.id} has duration ${JSON.stringify(t.duration)}, not a whole number of minutes, 0 or more`);
  }
  return m;
}

/** The finish key of a task that starts at key `s` with `d` working minutes. */
export function finishKey(s: number, d: number): number {
  if (d === 0) return s;
  const m = s / 2 + d;
  return m % DAY_MINUTES === 0 ? 2 * m - 1 : 2 * m;
}

/** The smallest start key whose finish is at or after key `x`, for `d` working minutes. */
function startToFinishBy(x: number, d: number): number {
  if (d === 0) return x;
  const m = Math.ceil(x / 2);
  // An odd x is a day end, met by work that ends with that day; an even x at 08:00 is not.
  return 2 * (m - d + (x % 2 === 0 && m % DAY_MINUTES === 0 ? 1 : 0));
}

/** The earliest start key of each task by id, by the forward pass; summaries are not scheduled (none may be linked). */
export function forwardPass(start: string, data: DatasetTask[], links: DatasetLink[]): Map<string, number> {
  const dur = new Map(data.map((t) => [String(t.id), workMinutes(t)]));
  const into = new Map<string, DatasetLink[]>();
  const outOf = new Map<string, string[]>();
  const waiting = new Map<string, number>([...dur.keys()].map((id) => [id, 0]));
  for (const l of links) {
    const [p, s] = [String(l.source), String(l.target)];
    if (!dur.has(p) || !dur.has(s)) throw new Error(`link ${l.id} names an unknown task`);
    (into.get(s) ?? into.set(s, []).get(s)!).push(l);
    (outOf.get(p) ?? outOf.set(p, []).get(p)!).push(s);
    waiting.set(s, waiting.get(s)! + 1);
  }
  const snet = new Map(data.flatMap((t) => (t.start === undefined ? [] : [[String(t.id), keyOf(start, t.start)]])));
  const es = new Map<string, number>();
  const ready = [...waiting].filter(([, n]) => n === 0).map(([id]) => id);
  while (ready.length > 0) {
    const id = ready.pop()!;
    const d = dur.get(id)!;
    let lo = snet.get(id) ?? 0;
    let finLo = -Infinity;
    for (const l of into.get(id) ?? []) {
      const p = String(l.source);
      const ps = es.get(p)!;
      const type = String(l.type);
      if (type === "0") lo = Math.max(lo, finishKey(ps, dur.get(p)!));
      else if (type === "1") lo = Math.max(lo, ps);
      else if (type === "2") finLo = Math.max(finLo, finishKey(ps, dur.get(p)!));
      else if (type === "3") finLo = Math.max(finLo, ps);
      else throw new Error(`link ${l.id} has unknown type ${JSON.stringify(l.type)}`);
    }
    if (d > 0 && lo % 2 !== 0) lo += 1; // work starts on a working minute: the next 08:00
    es.set(id, finLo === -Infinity ? lo : Math.max(lo, startToFinishBy(finLo, d)));
    for (const s of outOf.get(id) ?? []) {
      waiting.set(s, waiting.get(s)! - 1);
      if (waiting.get(s) === 0) ready.push(s);
    }
  }
  if (es.size !== dur.size) throw new Error("the links form a loop");
  return es;
}

/**
 * Give every leaf its forward-pass start as a YYYY-MM-DD date (08:00), for a schedule whose every
 * leaf starts at 08:00 (whole-day work, finish-to-start links); throws on any other.
 */
export function placeLeafDays(start: string, data: DatasetTask[], links: DatasetLink[]): void {
  const parents = new Set(data.flatMap((t) => (t.parent === undefined ? [] : [String(t.parent)])));
  const es = forwardPass(start, data, links);
  for (const t of data) {
    if (parents.has(String(t.id))) continue;
    const at = stampKey(start, es.get(String(t.id))!);
    if (!at.endsWith("T08:00")) throw new Error(`task ${t.id} starts at ${at}, not at the start of a day`);
    t.start = at.slice(0, 10);
  }
}
