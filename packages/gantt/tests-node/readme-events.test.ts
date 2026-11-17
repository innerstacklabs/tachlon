// The README's `## Events` table has one row for each event of `GanttEvents` (src/events.ts), and no other. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const pkg = (file: string): string => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");

test("the_readme_events_table_has_one_row_for_each_gantt_event", () => {
  const body = /^export interface GanttEvents \{([\s\S]*?)^\}/m.exec(pkg("src/events.ts"))?.[1] ?? "";
  const events = [...body.matchAll(/^ {2}"?([\w-]+)"?:/gm)].map((m) => m[1]!).sort();
  expect(events.length, "the interface's events are found").toBeGreaterThanOrEqual(11);
  const section = /^## Events$([\s\S]*?)(?=^## |(?![\s\S]))/m.exec(pkg("README.md"))?.[1] ?? "";
  const rows = [...section.matchAll(/^\| `([\w-]+)` \|/gm)].map((m) => m[1]!).sort();
  expect(rows, "events in the interface and rows in the README's Events section").toEqual(events);
});
