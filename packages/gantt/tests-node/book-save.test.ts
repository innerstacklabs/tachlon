// The save guide file says what a saved file keeps and what an export keeps. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const page = readFileSync(new URL("../../../packages/gantt/guide/save.md", import.meta.url), "utf8").replace(/\s+/g, " ");

test("the_save_page_says_what_a_saved_file_and_an_export_keep", () => {
  expect(page).not.toContain("keeps everything");
  expect(page).toContain("deadlines");
  expect(page).not.toContain("data date that an export drops");
});
