// Reading a file with the built WASM module, no page (gs#1163): `LoadedSchedule.parse_bytes` and `parse_document` for
// each format and sample, the import notes, and the files that cannot be read. Needs `packages/gantt/src/wasm/`, which
// scripts/build-web.sh writes. The checks moved here from open.spec.ts and the two adapter specs; each browser test
// keeps what only a page shows.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { initSync, LoadedSchedule } from "../src/wasm/app.js";
import { fromBryntum } from "../src/adapters/bryntum.ts";
import { fromDhtmlx } from "../src/adapters/dhtmlx.ts";

initSync({ module: readFileSync(new URL("../src/wasm/app_bg.wasm", import.meta.url)) });

const bytes = (path: string): Uint8Array => readFileSync(new URL(path, import.meta.url));
const json = (path: string): unknown => JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8"));
const ONE_PROJECT = bytes("../tests/fixtures/one-project.xer");

/** An MSPDI file of `count` independent tasks that each carry a finish no later than constraint with no date, so each adds one import note (as in open.spec.ts). */
function constraintProject(count: number): Uint8Array {
  let xml =
    `<?xml version="1.0" encoding="UTF-8"?>\n<Project xmlns="http://schemas.microsoft.com/project">` +
    `<StartDate>2026-01-05T08:00:00</StartDate><Tasks>`;
  for (let uid = 1; uid <= count; uid++) {
    xml +=
      `<Task><UID>${uid}</UID><Name>T${uid}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>` +
      `<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat><ConstraintType>7</ConstraintType></Task>`;
  }
  return new TextEncoder().encode(`${xml}</Tasks></Project>`);
}

test("each format and sample reads, and says which format it was", () => {
  const files: [string, Uint8Array, string][] = [
    ["one-project.xer", ONE_PROJECT, "xer"],
    ["two-projects.xer", bytes("../tests/fixtures/two-projects.xer"), "xer"],
    ["ledger-timeline.xer", bytes("../tests/fixtures/ledger-timeline.xer"), "xer"],
    ["summary.xml", bytes("../../../crates/import/tests/fixtures/summary.xml"), "mspdi"],
    ["office-fitout.xml", bytes("../tests/fixtures/office-fitout.xml"), "mspdi"],
  ];
  for (const [name, file, format] of files) expect(LoadedSchedule.parse_bytes(file, "all").format(), name).toBe(format);
  // A document reads back as the format its own `format` field names.
  const documents: [string, { format: string }][] = [
    ["the DHTMLX adapter's document", fromDhtmlx(json("../tests/fixtures/dhtmlx.json"), { name: "fit-out" }).document],
    ["the Bryntum adapter's document", (fromBryntum(json("../tests/fixtures/bryntum-gantt.json")) as { document: { format: string } }).document],
  ];
  for (const [name, document] of documents) expect(LoadedSchedule.parse_document(JSON.stringify(document)).format(), name).toBe(document.format);
});

test("an XER's import notes are read with it", () => {
  const notes = LoadedSchedule.parse_bytes(ONE_PROJECT, undefined).unsupported() as string[];
  expect(notes).toHaveLength(5);
  expect(notes.join("\n")).toContain("in-progress activities: 2 of 2");
  // The last line is the percent-complete note: the fixtures measure percent complete by duration but have no column for it.
  expect(notes.join("\n")).toContain(
    "Percent complete was not read: the file measures it as duration or units percent complete, and has no column for that; no dates are affected.",
  );
});

test("a truncated XER and a file that is neither format throw", () => {
  expect(() => LoadedSchedule.parse_bytes(ONE_PROJECT.subarray(0, 700), undefined), "cut.xer").toThrow();
  expect(() => LoadedSchedule.parse_bytes(new TextEncoder().encode("this is not a schedule\n"), undefined), "notes.xml").toThrow();
});

test("every import note of a file is kept, past the old limit of 20 and past the panel's 500", () => {
  // 25 such tasks make 25 lines, and the importer adds one for the missing calendars: 26 in all.
  expect(LoadedSchedule.parse_bytes(constraintProject(25), undefined).unsupported(), "the calendars note and one per task").toHaveLength(26);
  expect(LoadedSchedule.parse_bytes(constraintProject(520), undefined).unsupported()).toHaveLength(521);
});
