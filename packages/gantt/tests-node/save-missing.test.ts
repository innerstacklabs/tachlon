import type { Handlers, Save } from "../src/gantt-types.ts";
import { guardBeforeEdit } from "../src/before-edit.ts";
import { profileFor, refuseMissing, warnMissing } from "../src/save-handlers.ts";
import { handlersOf, liveSave } from "../src/save-queue.ts";
import { MESSAGES_EN } from "../src/texts-en.ts";
import { mock, test } from "node:test";
import { expect } from "@playwright/test";

const done = async (): Promise<void> => undefined;
const all: Required<Omit<Handlers, "classify">> = { createTask: done, updateTask: done, deleteTask: done, createLink: done, updateLink: done, deleteLink: done, updateSchedule: done };
const without = (name: keyof typeof all): Handlers => Object.fromEntries(Object.entries(all).filter(([n]) => n !== name));
const say = (what: string, handler: string): string => MESSAGES_EN["cannot-save"]!.replace("{what}", what).replace("{handler}", handler);
const kinds: [keyof typeof all, string][] = [["createTask", "new tasks"], ["updateTask", "changes to tasks"], ["deleteTask", "deleted tasks"], ["createLink", "new links"], ["updateLink", "changes to links"], ["deleteLink", "deleted links"], ["updateSchedule", "changes to the schedule"]];

test("each missing handler refuses in the page's words before asking the host; otherwise the host's answer stands", () => {
  for (const [name, what] of kinds) {
    const before = mock.fn(() => "no, not now"), change = { needs: [name] };
    expect(refuseMissing(before, () => without(name), say)(change)).toBe(`This page cannot save ${what}: it gave no ${name}.`);
    expect(before.mock.callCount()).toBe(0);
    expect(refuseMissing(undefined, () => all, say)(change)).toBeUndefined();
    expect(refuseMissing(before, () => all, say)(change)).toBe("no, not now");
    expect(before.mock.callCount()).toBe(1);
  }
});
test("a gate constructed before a wrapper has save reads its later handlers; a function or no save lacks nothing", () => {
  let held: Save | Handlers | undefined;
  const before = mock.fn(() => "host answer"), change = { needs: ["createLink"] };
  const gate = refuseMissing(before, () => handlersOf(liveSave(() => held)), say);
  expect(gate(change)).toBe("host answer");
  held = without("createLink");
  expect(gate(change)).toBe("This page cannot save new links: it gave no createLink.");
  expect(before.mock.callCount()).toBe(1);
  held = done;
  expect(gate(change)).toBe("host answer");
  expect(refuseMissing(before, () => handlersOf(undefined), say)(change)).toBe("host answer");
  expect(before.mock.callCount()).toBe(3);
});
test("the host's before-edit function never sees needs", () => {
  const before = mock.fn((_: unknown) => undefined);
  guardBeforeEdit(before, () => undefined, () => [])({ kind: "group", source: "typed", tasks: [0], changes: [], needs: ["createLink"] });
  expect(before.mock.calls[0]!.arguments[0]).toStrictEqual({ kind: "group", source: "typed", tasks: [0], changes: [] });
});
test("one warning names every missing handler; full handlers, a function and no save warn nothing", () => {
  const warn = mock.method(console, "warn", () => undefined);
  try {
    warnMissing({ ...without("createLink"), deleteLink: undefined });
    expect(warn.mock.calls.map((c) => c.arguments[0])).toEqual(["the save handlers have no createLink (new links), deleteLink (deleted links); the chart refuses those edits."]);
    for (const save of [all, done, undefined]) warnMissing(handlersOf(save));
    expect(warn.mock.callCount()).toBe(1);
  } finally { warn.mock.restore(); }
});
test("no createLink turns link drawing off and preserves the host's other profile members", () => {
  const host = { gestures: { barMove: false }, editable: { names: false } };
  expect(profileFor(host, without("createLink"))).toEqual({ gestures: { barMove: false, link: false }, editable: { names: false } });
  expect(profileFor(undefined, without("createLink"))).toEqual({ gestures: { link: false } });
  expect(profileFor(host, all)).toBe(host);
  expect(profileFor(host, undefined)).toBe(host);
});
