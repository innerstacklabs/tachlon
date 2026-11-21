// A handlers object with no handler for a kind of change (decision 0022, rule 3.1): one warning at mount, link drawing off with no `createLink`, and the edits that need a missing handler refused ahead of `beforeEdit`.
// The browser case, undo included, is scenario 26 of `examples/save/spec.ts`.
import type { FieldChange } from "../src/change-set.ts";
import type { Handlers } from "../src/gantt-types.ts";
import { guardBeforeEdit } from "../src/before-edit.ts";
import { profileFor, refuseMissing, warnMissing } from "../src/save-handlers.ts";
import { MESSAGES_EN } from "../src/texts-en.ts";
import { mock, test } from "node:test";
import { expect } from "@playwright/test";

const done = async (): Promise<void> => undefined;
const all: Required<Omit<Handlers, "classify">> = { createTask: done, updateTask: done, deleteTask: done, createLink: done, updateLink: done, deleteLink: done, updateSchedule: done };
const without = (name: keyof typeof all): Handlers => Object.fromEntries(Object.entries(all).filter(([n]) => n !== name));
const say = (what: string, handler: string): string => MESSAGES_EN["cannot-save"]!.replace("{what}", what).replace("{handler}", handler);
const KINDS: [FieldChange["record"], FieldChange["op"], keyof typeof all, string][] = [
  ["task", "create", "createTask", "new tasks"], ["task", "update", "updateTask", "changes to tasks"], ["task", "delete", "deleteTask", "deleted tasks"],
  ["link", "create", "createLink", "new links"], ["link", "update", "updateLink", "changes to links"], ["link", "delete", "deleteLink", "deleted links"],
  ["schedule", "update", "updateSchedule", "changes to the schedule"],
];
const change = (record: FieldChange["record"], op: FieldChange["op"]): { changes: FieldChange[] } => ({ changes: [{ record, op, key: "k1", task: 0, field: "name", old: "a", new: "b", fields: { name: "a" } }] });

test("an edit that needs a handler the host did not give is refused in the words of the page, and one it did give is not", () => {
  for (const [record, op, name, what] of KINDS) {
    expect(refuseMissing(undefined, () => without(name), say)(change(record, op)), name).toBe(`This page cannot save ${what}: it gave no ${name}.`);
    expect(refuseMissing(undefined, () => all, say)(change(record, op)), name).toBeUndefined();
  }
});

test("the host's before-edit function is asked only about an edit the handlers can save, and its answer stands", () => {
  const before = mock.fn(() => "no, not now");
  const gate = refuseMissing(before, () => without("createLink"), say);
  expect(gate(change("link", "create"))).toBe("This page cannot save new links: it gave no createLink.");
  expect(before.mock.callCount()).toBe(0);
  expect(gate(change("task", "update"))).toBe("no, not now");
  expect(before.mock.callCount()).toBe(1);
});

test("a plan of several edits that carries only `needs` is refused for the handler of any of them: typed links, an undo of links, an undo of rows", () => {
  const row = (record: FieldChange["record"], op: FieldChange["op"]): FieldChange => ({ record, op, key: null, link: 0 });
  const plans: [string, FieldChange[], keyof typeof all, string][] = [
    ["group", [row("link", "create"), row("link", "create")], "createLink", "new links"],
    ["undo", [row("link", "delete"), row("link", "delete")], "deleteLink", "deleted links"],
    ["undo", [row("task", "delete"), row("task", "delete")], "deleteTask", "deleted tasks"],
  ];
  for (const [kind, needs, name, what] of plans) {
    const before = mock.fn(() => undefined);
    expect(refuseMissing(before, () => without(name), say)({ changes: [], needs }), kind + name).toBe(`This page cannot save ${what}: it gave no ${name}.`);
    expect(before.mock.callCount()).toBe(0);
    expect(refuseMissing(before, () => all, say)({ changes: [], needs }), name).toBeUndefined();
    expect(before.mock.callCount()).toBe(1);
  }
});

test("the host's before-edit function is shown the change it always was, never `needs`", () => {
  const before = mock.fn((_: unknown) => undefined);
  const needs: FieldChange[] = [{ record: "link", op: "create" }];
  guardBeforeEdit(before as never, () => undefined, (changes) => changes as never)({ kind: "group", source: "typed", tasks: [0], changes: [], needs });
  expect(before.mock.calls[0]!.arguments[0]).toStrictEqual({ kind: "group", source: "typed", tasks: [0], changes: [] });
});

test("one warning at mount names every missing handler and what it turns off; a full set, a function and none warn nothing", () => {
  const warn = mock.method(console, "warn", () => undefined);
  try {
    warnMissing({ ...without("createLink"), deleteLink: undefined });
    expect(warn.mock.callCount()).toBe(1);
    expect(String(warn.mock.calls[0]!.arguments[0])).toBe("the save handlers have no createLink (new links), deleteLink (deleted links); the chart refuses those edits.");
    warnMissing(all);
    warnMissing(undefined);
    expect(warn.mock.callCount()).toBe(1);
  } finally {
    warn.mock.restore();
  }
});

test("no createLink turns link drawing off and keeps the host's own members; with it, or with no handlers object, the profile is the host's", () => {
  const host = { gestures: { barMove: false }, editable: { names: false } };
  expect(profileFor(host, without("createLink"))).toEqual({ gestures: { barMove: false, link: false }, editable: { names: false } });
  expect(profileFor(undefined, without("createLink"))).toEqual({ gestures: { link: false } });
  expect(profileFor(host, all)).toBe(host);
  expect(profileFor(host, undefined)).toBe(host);
});
