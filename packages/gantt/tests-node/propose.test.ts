// A proposal about a link (gantt#1250): `propose` asks the link exports and `acceptKey` / `rejectKey` settle by pair. No page: the app is a recording stand-in.
import { acceptKey, propose, type ProposedChange, type ProposedKey, rejectKey } from "../src/propose.ts";
import type { PublicApp } from "../src/api.js";
import { test } from "node:test";
import { expect } from "@playwright/test";

const calls: unknown[][] = [];
const moved = JSON.stringify({ ok: true, finish_was: "2026-03-02 17:00", finish_now: "2026-03-09 17:00", moved: [] });
const call = (name: string, ...args: unknown[]) => (calls.push([name, ...args]), name.startsWith("reject") ? true : moved);
const app = new Proxy({}, { get: (_, name: string) => (...args: unknown[]) => call(name, ...args) }) as unknown as PublicApp;
const asked = (change: unknown) => (calls.length = 0, propose(app, change as ProposedChange));

test("a_link_or_unlink_asks_the_link_export_with_its_pair_and_answers_what_moves", () => {
  expect(asked({ kind: "link", pred: 3, succ: 5, text: "SS+2d" })).toMatchObject({ ok: true, finishNow: "2026-03-09 17:00", moved: [] });
  expect(calls).toEqual([["propose_link", 3, 5, "SS+2d"]]);
  expect(asked({ kind: "unlink", pred: 3, succ: 5 })).toMatchObject({ ok: true });
  expect(calls).toEqual([["propose_unlink", 3, 5]]);
});

test("a_link_ask_that_names_no_pair_or_no_text_is_a_refusal_and_never_a_call", () => {
  expect(asked({ kind: "link", pred: 3, succ: -1, text: "FS" })).toMatchObject({ ok: false, reason: "unknownTask" });
  expect(asked({ kind: "unlink", pred: "3", succ: 5 })).toMatchObject({ ok: false, reason: "unknownTask" });
  expect(asked({ kind: "link", pred: 3, succ: 5 })).toMatchObject({ ok: false, reason: "badLinkText" });
  expect(calls).toEqual([]);
});

test("a_key_with_a_pair_settles_the_link_and_a_key_with_a_task_settles_the_task", () => {
  calls.length = 0;
  acceptKey(app, { kind: "link", pred: 3, succ: 5 });
  rejectKey(app, { kind: "unlink", pred: 5, succ: 3 });
  acceptKey(app, { kind: "duration", task: 4 });
  rejectKey(app, { kind: "move", task: 4 });
  expect(calls).toEqual([["accept_proposed_link", 3, 5], ["reject_proposed_link", 5, 3], ["accept_proposed", "duration", 4], ["reject_proposed", "move", 4]]);
});

test("a_progress_change_asks_the_progress_export_with_its_one_field_and_is_settled_by_its_task", () => {
  const sent = (change: unknown) => (asked({ kind: "progress", task: 4, ...(change as object) }), calls.map((c) => c.slice(1)));
  expect(sent({ actualFinish: "2026-03-06" })).toEqual([[4, "actual_finish", "2026-03-06"]]);
  expect(sent({ actualStart: "2026-03-02 08:00" })).toEqual([[4, "actual_start", "2026-03-02 08:00"]]);
  expect(sent({ remaining: 2.5 })).toEqual([[4, "remaining", "2.5d"]]);
  expect(sent({ progress: 40 })).toEqual([[4, "percent", "40"]]);
  for (const bad of [{}, { progress: 40, remaining: 1 }, { progress: null }]) expect(asked({ kind: "progress", task: 4, ...bad })).toMatchObject({ ok: false, reason: "other" });
  expect(calls).toEqual([]);
  acceptKey(app, { kind: "progress", task: 4 });
  rejectKey(app, { kind: "progress", task: 4 });
  expect(calls).toEqual([["accept_proposed", "progress", 4], ["reject_proposed", "progress", 4]]);
});

test("a_key_that_names_nothing_is_a_key_no_change_holds", () => {
  calls.length = 0;
  acceptKey(app, { kind: "link", pred: 3 } as unknown as ProposedKey);
  rejectKey(app, undefined as unknown as ProposedKey);
  expect(calls).toEqual([["accept_proposed", "", 0], ["reject_proposed", "", 0]]);
});
