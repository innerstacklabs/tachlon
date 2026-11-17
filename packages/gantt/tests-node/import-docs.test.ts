import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const read = (path: string): string => readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8");
const doc = read("docs/IMPORT.md");
test("the_import_guide_quotes_the_base_calendar_note_the_xer_reader_writes", () => {
  const quote = /the notes list says "([^"]*)"/.exec(doc)![1]!;
  expect(read("crates/import/src/xer_calendars.rs")).toContain(quote.split(";")[0]!.replace("Calendar N", "Calendar {clndr_id}"));
});
test("the_import_guide_says_the_export_dialog_calls_the_ms_project_export", () => {
  expect(doc).not.toContain("No page control");
});
test("the_export_button_title_names_both_formats", () => {
  expect(/data-part="export-button"[^>]*title="([^"]*)"/.exec(read("apps/chrome/src/file.html"))?.[1]).toMatch(/MS Project XML.*XER/);
});
