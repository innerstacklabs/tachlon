// The "What works today" table (docs/JOURNEYS.md) must not call a journey "Not started" once the journey has a
// spec: `packages/gantt/tests/journey-jN.spec.ts` walks what main does for it, so the row says what that is. Read as text.
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const root = (path: string): URL => new URL(`../../../${path}`, import.meta.url);

/** The rows of the "What works today" table: the journey number and the row's text. */
function journeyRows(readme: string): { n: number; row: string }[] {
  const table = /^# What works today$([\s\S]*)/m.exec(readme)?.[1] ?? "";
  return [...table.matchAll(/^\| J(\d+)\. .*$/gm)].map((m) => ({ n: Number(m[1]), row: m[0] }));
}

test("no_readme_journey_row_says_not_started_when_its_journey_spec_exists", () => {
  const rows = journeyRows(readFileSync(root("docs/JOURNEYS.md"), "utf8"));
  expect(rows.length, "the table has a row per journey").toBeGreaterThanOrEqual(9);
  const withSpec = rows.filter(({ n }) => existsSync(root(`packages/gantt/tests/journey-j${n}.spec.ts`)));
  expect(withSpec.length, "the journey specs are found").toBeGreaterThan(0);
  // The row is `| journey | what works today | chapter |`: only the second cell is the status, not the name or the chapter link.
  const stale = withSpec.filter(({ row }) => /^not started\b/i.test(row.split("|")[2]!.trim())).map(({ n }) => `J${n}`);
  expect(stale, "journeys with a spec whose README row still says Not started").toEqual([]);
});
