import { expect, test } from "@playwright/test";
import { installBaselineSummary } from "../src/baseline-summary";
import { createBaselineSummaryText } from "../src/status-line";
import type { Status } from "../src/api";
import type { Shell } from "../src/shell";

// An idle frame builds no baseline summary string (gs#864): each chart's clause builder hands back one array while the counts
// stand, and the chip and the developer line join it only when the array is a new one. The spec runs in Node on the modules
// themselves and counts the `join` calls the frames make. Each chart keeps its own builder (gs#1195), so two charts with
// different counts never rebuild each other's.

const fmt = new Intl.NumberFormat("en-US");
const summary = { slipped: 46, finish_days: 26, joined: 0, left: 3, added: 0, removed: 0 };

/** A chip on a fake root, and the number of times its text was written. */
function chip(): { sync: (s: Status) => void; writes: () => number } {
  let writes = 0;
  const element = { hidden: true, set textContent(_: string) { writes++; }, get textContent() { return ""; } };
  const root = { querySelector: () => element } as unknown as HTMLElement;
  const installed = installBaselineSummary({ root, fmt } as unknown as Shell);
  return { sync: installed.sync, writes: () => writes };
}

const status = (baseline: typeof summary | null): Status => ({ baseline }) as Status;

/** The `join` calls `run` makes on any array. */
function joinsIn(run: () => void): number {
  const join = Array.prototype.join;
  let joins = 0;
  Array.prototype.join = function (this: unknown[], separator?: string): string {
    joins++;
    return join.call(this, separator);
  };
  try {
    run();
  } finally {
    Array.prototype.join = join;
  }
  return joins;
}

test("an idle frame joins no string for the chip or the developer line", () => {
  const a = chip();
  const line = createBaselineSummaryText();
  a.sync(status({ ...summary }));
  expect(a.writes(), "the first frame writes the chip").toBe(1);
  expect(line({ ...summary }, fmt)).toBe("vs baseline: 46 slipped, finish +26 days, critical 0 joined, 3 left");
  const joins = joinsIn(() => {
    for (let frame = 0; frame < 5; frame++) {
      a.sync(status({ ...summary }));
      line({ ...summary }, fmt);
    }
  });
  expect(joins, "five idle frames join nothing").toBe(0);
  expect(a.writes(), "and write nothing").toBe(1);

  // A change of count is a new array: it is joined, and written.
  a.sync(status({ ...summary, slipped: 47 }));
  expect(a.writes()).toBe(2);
});

test("two charts with different counts keep their own clauses, so idle frames in both build nothing", () => {
  const a = chip();
  const b = chip();
  const other = { ...summary, slipped: 3, finish_days: -2 };
  a.sync(status({ ...summary }));
  b.sync(status({ ...other }));
  const joins = joinsIn(() => {
    for (let frame = 0; frame < 5; frame++) {
      a.sync(status({ ...summary }));
      b.sync(status({ ...other }));
    }
  });
  expect(joins, "neither chart rebuilds the other's clauses").toBe(0);
  expect([a.writes(), b.writes()]).toEqual([1, 1]);
});
