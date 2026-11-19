// The progress guide file says a typed % complete from 1 to 99 gives a task with no actual start its early start and a typed 0 leaves it unstarted; the compare section says how tasks match. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const page = (path: string): string => readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8").replace(/\s+/g, " ");
test("a_typed_percent_from_1_to_99_gives_a_task_with_no_actual_start_its_early_start_and_0_leaves_it_unstarted_says_the_page", () => {
  expect(page("packages/gantt/guide/progress.md")).toContain("with no actual start also gives it one, its early start");
  expect(page("packages/gantt/guide/progress.md")).toContain("A `0` on a task with no actual start leaves it unstarted");
});
test("tasks_match_by_activity_id_for_p6_and_by_task_uid_for_ms_project_says_the_compare_section", () => {
  expect(page("book/src/baseline.md")).toContain("by activity ID for a P6 file and by task UID for an MS Project file");
});
