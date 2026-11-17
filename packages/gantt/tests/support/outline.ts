import type { Page } from "@playwright/test";

/** The first top-level expanded summary oracle used by both outline.spec.ts and find.spec.ts. */
export interface FirstSummary {
  hasSummary: boolean;
  r0?: number;
  task?: number;
  expected?: number;
  before?: number;
  next?: number | null;
  nextRow?: number;
  firstChild?: number | null;
}

export async function firstTopLevelSummary(page: Page): Promise<FirstSummary> {
  return page.evaluate(() => {
    const app = window.gantt!.app;
    const before = window.gantt!.status().rowCount;
    const rows = JSON.parse(app.table_rows(0, before)) as number[][];
    const hasSummary = rows.some((r) => r[5] === 1);
    const r0 = rows.findIndex((r) => r[5] === 1 && app.task_outline_level(r[0]) === 1);
    if (r0 < 0) return { hasSummary };
    const task = rows[r0][0];
    const level = app.task_outline_level(task);
    let nextRow = r0 + 1;
    while (nextRow < rows.length && app.task_outline_level(rows[nextRow][0]) > level) nextRow++;
    const expected = nextRow - r0 - 1;
    return {
      hasSummary,
      r0,
      task,
      expected,
      before,
      next: nextRow < rows.length ? rows[nextRow][0] : null,
      nextRow,
      firstChild: rows[r0 + 1]?.[0] ?? null,
    };
  });
}

/** A `FirstSummary` known to hold a top-level expanded summary: every field the oracle fills in is present. */
export type ExpandedFirstSummary = Required<Omit<FirstSummary, "next" | "firstChild">> & {
  next: number | null;
  firstChild: number | null;
};

export function assertExpandedSummary(summary: FirstSummary): asserts summary is ExpandedFirstSummary {
  if (summary.r0 === undefined) throw new Error("top-level expanded summary row missing");
}
