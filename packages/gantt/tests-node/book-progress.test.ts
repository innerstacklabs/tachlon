// The progress page says a typed % complete below 100 gives a task with no actual start its early start; the compare section says how tasks match. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const page = (name: string): string => readFileSync(new URL(`../../../book/src/${name}`, import.meta.url), "utf8").replace(/\s+/g, " ");
test("a_typed_percent_below_100_gives_a_task_with_no_actual_start_its_early_start_says_the_page", () => {
  expect(page("j8-progress.md")).toContain("with no actual start also gives it one, its early start");
});
test("tasks_match_by_activity_id_for_p6_and_by_task_uid_for_ms_project_says_the_compare_section", () => {
  expect(page("baseline.md")).toContain("by activity ID for a P6 file and by task UID for an MS Project file");
});
