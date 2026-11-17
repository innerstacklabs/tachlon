// Two sentences on the public page (apps/site/index.html) say what the chart and the file readers do. Read as text, no page.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const page = readFileSync(new URL("../../../apps/site/index.html", import.meta.url), "utf8");

test("a_change_note_quoted_on_the_home_page_has_no_comma_before_finish", () => {
  // The chart writes "N tasks moved · finish +D days" (crates/app/tests/moved.rs), so a quote keeps the dot.
  expect(page.match(/tasks moved [^\s,]+ finish/g), "the page quotes the note with the chart's separator").not.toBeNull();
  expect(page.match(/tasks moved,\s*finish/g), "a comma before finish").toBeNull();
});

test("activity_codes_appear_on_the_home_page_only_beside_p6", () => {
  const found = [...page.matchAll(/(\S+\s+)?activity codes/gi)];
  expect(found.length, "the page names activity codes").toBeGreaterThan(0);
  expect(found.filter((m) => m[1]?.trim() !== "P6").map((m) => m[0]), "activity codes not beside P6").toEqual([]);
});
