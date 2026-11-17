// The typed-model experiment's answer key (gantt#1250) and what the specs need to read it: the menu states the Filters menu can
// reach on the office fit-out, and a way to apply each one through the chart and read back the activities it keeps. Nothing here
// calls a model; the key is data (docs/research/typed-search-questions.json).
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { CREWS, ROWS, toMspdi } from "../../samples/office-fitout";

/** A row filter as `Commands.setFilter` takes it, or `"off"` for none. */
export type RowChoice = string | { status: string } | { check: string } | { resource: string };
export interface Span {
  start: string;
  weeks: number;
}
/** One state of the Filters menu: the row filter, then the Lookahead window (null for none). */
export interface MenuState {
  row: RowChoice;
  lookahead: Span | null;
}
export interface Question {
  id: string;
  question: string;
  /** The menu state that answers the question, or "cannot" when no menu state does (then `reason` says why and `keeps` and `sameSet` are empty). */
  state: MenuState | "cannot";
  reason?: string;
  /** The uids of the activities (leaf tasks) the chart keeps in `state`, ascending. */
  keeps: number[];
  /** The other menu states that keep exactly `keeps`, in menu order: a set is never read as a state. */
  sameSet: MenuState[];
}
export type MenuQuestion = Question & { state: MenuState };
export const isMenuQuestion = (question: Question): question is MenuQuestion => question.state !== "cannot";
export interface Key {
  windows: { starts: string[]; weeks: number[] };
  questions: Question[];
}

export const KEY: Key = JSON.parse(readFileSync(new URL("../../../../docs/research/typed-search-questions.json", import.meta.url), "utf8"));

/**
 * The row filters the menu can turn on for this sample, with "off": 14 of its 17, then the Resource choice for each crew in `CREWS`. Float lost needs a
 * baseline with float and Show moved needs an edit, so the chart refuses those; the sample has no activity codes, so the menu hides Activity code.
 */
export const ROW_CHOICES: RowChoice[] = [
  "off", "critical", "longestPath", "nearCritical", "slipped", "joinedCritical", "leftCritical", "startConstraint",
  { status: "in-progress" }, { status: "not-started" }, { status: "complete" },
  { check: "missing-logic" }, { check: "open-start" }, { check: "open-end" },
  ...Object.keys(CREWS).map((resource) => ({ resource })),
];

/** Every menu state: each row filter with no window and with each window the key names (`ROW_CHOICES.length` times 16). */
export function menuStates(): MenuState[] {
  const windows: (Span | null)[] = [null, ...KEY.windows.starts.flatMap((start) => KEY.windows.weeks.map((weeks) => ({ start, weeks })))];
  return ROW_CHOICES.flatMap((row) => windows.map((lookahead) => ({ row, lookahead })));
}

/** The page starts on the engine's own copy of the fit-out, which names no crew, so the Resource filter refuses every name: open the fit-out file, which does. */
export async function openFitout(page: Page): Promise<void> {
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: Buffer.from(toMspdi()) });
  await page.waitForFunction(() => window.gantt!.app.resource_names() !== "[]", undefined, { timeout: 60_000 });
}

export interface Applied {
  /** What `setFilter` answered for every part of the state. */
  taken: boolean;
  /** The chart's own reading of the state afterwards, in the shape of `MenuState`. */
  readback: MenuState;
  /** The uids of the activities the chart keeps, ascending. */
  kept: number[];
}

/** Apply each state through the chart (clear, row filter, then window) and read what it keeps; a row the chart has no uid for fails the run. */
export async function applyAll(page: Page, states: MenuState[]): Promise<Applied[]> {
  const uids = ROWS.map((row) => (row.summary === true ? null : row.uid));
  const names = ROWS.map((row) => row.name);
  return page.evaluate(
    ({ states, uids, names }) => {
      const { api, app } = window.gantt!;
      const count = api!.state().taskCount;
      if (count !== uids.length) throw new Error(`the chart holds ${count} tasks, the table ${uids.length}`);
      return states.map((state) => {
        api!.commands.clearFilters();
        let taken = true;
        if (state.row !== "off") taken = api!.commands.setFilter(state.row as never) === true;
        if (state.lookahead) taken = api!.commands.setFilter({ lookahead: state.lookahead }) === true && taken;
        const now = api!.state();
        const row = now.filter === "status" ? { status: now.statusFilter! } : now.filter === "check" ? { check: now.checkFilter! } : now.filter === "resource" ? { resource: now.resourceFilter! } : now.filter;
        const kept: number[] = [];
        for (let id = 0; id < count; id += 1) {
          if (!app.task_name(id).endsWith(names[id]!)) throw new Error(`task ${id} is not row ${id} of the table`);
          if (uids[id] !== null && app.test_task_row(id) != null) kept.push(uids[id]!);
        }
        const { on, start, weeks } = now.lookahead;
        return { taken, readback: { row, lookahead: on ? { start, weeks } : null }, kept: kept.sort((a, b) => a - b) };
      });
    },
    { states, uids, names },
  );
}
