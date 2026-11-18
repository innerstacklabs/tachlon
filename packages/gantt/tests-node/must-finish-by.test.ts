// The must-finish-by date (gantt#2074): plain data carries it into the engine, a document carries it back out, and one outside the engine's range is refused.
// Needs `packages/gantt/src/wasm/` (scripts/build-web.sh).
import { test } from "node:test";
import { expect } from "@playwright/test";
import { openHeadless } from "../src/headless.ts";
import { fromTasks, type PlainLink, type PlainTask } from "../src/adapters/plain.ts";
import { toPlain } from "../src/adapters/to-plain.ts";
import { PlainDataError } from "../src/adapters/validate.ts";

// Three tasks of five working days in a row: from Monday 2026-03-02 to Friday 2026-03-20, 17:00 (the document counts minutes from that Monday).
const tasks: PlainTask[] = ["a", "b", "c"].map((id) => ({ id, name: id, start: "2026-03-02", duration: 5 }));
const links: PlainLink[] = [{ from: "a", to: "b" }, { from: "b", to: "c" }];
const floats = (mustFinishBy?: string) => openHeadless({ tasks, links, ...(mustFinishBy === undefined ? {} : { mustFinishBy }) }).results().map((t) => t.float);

test("a date two working days before the finish gives every task of the chain minus two days of float", () => {
  expect(floats()).toEqual([0, 0, 0]);
  expect(floats("2026-03-18T17:00")).toEqual([-2, -2, -2]);
  expect(floats("2026-03-18")).toEqual([-2, -2, -2]); // a date alone is the end of that day
});

test("the date goes into the document and comes back out of it, and is left out when none is given", () => {
  const starts = ["2026-03-02 08:00", "2026-03-09 08:00", "2026-03-16 08:00"];
  const { document } = fromTasks({ tasks, links, mustFinishBy: "2026-03-18T17:00" });
  expect(toPlain(document, starts).mustFinishBy).toBe("2026-03-18T17:00");
  expect(document.must_finish_by).toBe(16 * 24 * 60 + 17 * 60);
  expect(fromTasks({ tasks, links }).document).not.toHaveProperty("must_finish_by");
  expect(toPlain(fromTasks({ tasks, links }).document, starts)).not.toHaveProperty("mustFinishBy");
});

test("a date that is not a date, or is outside the engine's range, is refused by name", () => {
  expect(() => fromTasks({ tasks, mustFinishBy: "2026-02-30" })).toThrow(PlainDataError);
  expect(() => openHeadless({ tasks, mustFinishBy: "9999-12-31" })).toThrow(/must_finish_by: .* outside what the engine accepts/);
});
