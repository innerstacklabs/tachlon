// Claims the site's pages (apps/site/*.html, tl#1048) must not make: Cursor rules shipped in the package, and the bar shape, row height or type called "documented".
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const dir = new URL("../../../apps/site/", import.meta.url);
const text = (page: string): string => readFileSync(new URL(page, dir), "utf8").replace(/<[^>]+>/g, "");
test("the_launch_page_line_on_what_the_package_ships_names_no_cursor_rules", () => {
  const line = text("launch.html").split(".").find((sentence) => sentence.includes("inside the package"));
  expect(line, "the launch page has the line on what ships in the package").toBeDefined();
  expect(line, "the package ships no Cursor rules").not.toContain("Cursor");
});
test("no_page_calls_the_bar_shape_row_height_or_type_documented", () => {
  const pages = readdirSync(dir).filter((name) => name.endsWith(".html"));
  expect(pages, "the pages are found").toContain("launch.html");
  const clauses = pages.flatMap((page) => text(page).split(/[.;]/).map((clause) => [page, clause]));
  for (const [page, clause] of clauses) if (/\bdocumented\b/.test(clause!)) expect(clause, `${page} calls something documented`).not.toMatch(/\b(bar shape|row height|type)\b/);
});
