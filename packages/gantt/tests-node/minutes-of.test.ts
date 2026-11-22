// `minutesOf` reads a date by its characters. Over a generated set of date strings it gives what the two regular
// expressions it replaced gave, kept here as `byPattern`.
import assert from "node:assert/strict";
import { test } from "node:test";
import { minutesOf } from "../src/adapters/dhtmlx.ts";

function byPattern(text: string): number | null {
  const iso = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::\d{2}(?:\.\d+)?)?Z?)?$/.exec(text.trim());
  const dmy = /^(\d{2})-(\d{2})-(\d{4})(?: (\d{2}):(\d{2}))?$/.exec(text.trim());
  const found = iso !== null ? [iso[1], iso[2], iso[3], iso[4], iso[5]] : dmy !== null ? [dmy[3], dmy[2], dmy[1], dmy[4], dmy[5]] : null;
  if (found === null) return null;
  const [year, month, day, hour, minute] = found.map((part) => (part === undefined ? 0 : Number(part))) as [number, number, number, number, number];
  const date = new Date(Date.UTC(year, month - 1, day));
  if (date.getUTCFullYear() !== year || date.getUTCMonth() !== month - 1 || date.getUTCDate() !== day || hour > 23 || minute > 59) return null;
  return date.getTime() / 60_000 + hour * 60 + minute;
}

const years = ["2026", "2024", "2023", "1900", "0000", "0099", "9999", "202", "20266", "2O26"];
const months = ["01", "02", "09", "12", "13", "00", "1", "1x"];
const days = ["01", "09", "28", "29", "30", "31", "32", "00", "9", "٠١"];
const times = ["", " 08:00", "T08:00", " 8:00", "T23:59", " 24:00", " 12:60", "T08:00:30", " 08:00:30.5", "T08:00:30.123Z", "T08:00Z",
  " 08:00Z", "Z", "T", " ", "T08:00:3", "T08:00:30.", "T08:00:30.5x", "T08:00:30Z ", " 08:00 x", " 08-00", "t08:00", "T08:00:30.12.3"];
const edges = [["", ""], [" ", ""], ["", " "], ["\t ", "\n"], [" ", "﻿"], ["x", ""], ["", "x"]];

function generated(): string[] {
  const out = ["", " ", "date", "2026/03/02", "2026-03-0", "-2026-03-02", "02-03-2026T08:00", "2026-03-02T08:00:30.5Z"];
  for (const y of years) for (const m of months) for (const d of days) for (const t of times) {
    for (const [before, after] of edges) out.push(`${before}${y}-${m}-${d}${t}${after}`, `${before}${d}-${m}-${y}${t}${after}`);
  }
  return out;
}

test("minutesOf gives what the two patterns gave, over generated dates, times, spaces and trailing text", () => {
  const texts = generated();
  const differ = texts.filter((text) => minutesOf(text) !== byPattern(text));
  assert.deepEqual(differ.slice(0, 10).map((text) => [text, minutesOf(text), byPattern(text)]), []);
  const read = texts.filter((text) => byPattern(text) !== null).length;
  assert.ok(read > 2_000 && texts.length - read > 2_000, `${read} of ${texts.length} read: the set needs both kinds`);
});
