// The sentences the chart writes (gantt#1875): templates with `{name}` holes, and for a count the plural form of the chart's locale (`Intl.PluralRules`).
import { test } from "node:test";
import { expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { keepMessages, say } from "../src/messages.ts";
import { MESSAGES_EN } from "../src/texts-en.ts";

// Polish has three forms for a count that English has two for: 1 is `one`, 22 is `few`, 5 is `many`.
const PL = { "bytes.one": "{count} bajt", "bytes.few": "{count} bajty", "bytes.many": "{count} bajtów", "bytes.other": "{count} bajta" };

test("pl gives one, few and many for 1, 22 and 5", () => {
  const root = {};
  keepMessages(root, PL, "pl");
  expect([1, 22, 5].map((count) => say(root, "bytes", { count }))).toEqual(["1 bajt", "22 bajty", "5 bajtów"]);
});

test("the holes are filled from the vars, and a number is written in the chart's format", () => {
  const root = {};
  keepMessages(root, { "row-position": "Zeile {row} von {total}", "rows-selected": "{count} Zeilen markiert" }, "de");
  expect(say(root, "row-position", { row: "1", total: "2" })).toBe("Zeile 1 von 2");
  expect(say(root, "rows-selected", { count: 1234 }), "German writes a point for thousands").toBe("1.234 Zeilen markiert");
});

test("a key the host leaves out stays English, and English has its own plural forms", () => {
  const root = {};
  keepMessages(root, { saved: "Gespeichert: {file}" }, "de");
  expect(say(root, "saved", { file: "a.gantt" })).toBe("Gespeichert: a.gantt");
  expect(say(root, "empty", { name: "a.xer" }), "no German text: English").toBe("a.xer is empty.");
  expect(say(root, "baseline-set", { count: 1 })).toBe("Baseline set: 1 row");
  expect(say(root, "baseline-set", { count: 59 })).toBe("Baseline set: 59 rows");
});

test("a host's `.other` beats the English form for a count the host has no form for", () => {
  const root = {};
  keepMessages(root, { "bytes.other": "{count} Bytes" }, "de");
  expect(say(root, "bytes", { count: 1 }), "German `one` falls to the host's `.other`, not to English `one`").toBe("1 Bytes");
});

test("a chart that kept nothing writes English, and an unknown key is its own text", () => {
  expect(say({}, "rows-selected", { count: 5 })).toBe("5 rows selected");
  expect(say({}, "no-such-key")).toBe("no-such-key");
});

test("the English fallback keeps English's plural forms under a locale whose categories differ", () => {
  const russian = {};
  keepMessages(russian, undefined, "ru");
  expect(say(russian, "baseline-set", { count: 21 }), "Russian says `one` for 21; English says rows").toBe("Baseline set: 21 rows");
  expect(say(russian, "bytes", { count: 1001 }), "Russian says `one` for 1001; English says bytes").toMatch(/ bytes$/);
  const japanese = {};
  keepMessages(japanese, undefined, "ja");
  expect(say(japanese, "baseline-set", { count: 1 }), "Japanese has only `other`; English says row for 1").toBe("Baseline set: 1 row");
  const french = {};
  keepMessages(french, undefined, "fr");
  expect(say(french, "baseline-set", { count: 0 }), "French says `one` for 0; English says rows").toBe("Baseline set: 0 rows");
});

// A host gives one `texts` object, and the keys of the app's own sentences (`crates/app/src/texts.rs`) go on to `App.set_locale`: a key in both would word two sentences at once.
test("no sentence of the chart has the key of one the app writes", () => {
  const rust = readFileSync(new URL("../../../crates/app/src/texts.rs", import.meta.url), "utf8");
  const app = new Set([...rust.matchAll(/^\s+[A-Z_0-9]+ "([a-zA-Z]+)"/gm)].map((m) => m[1]));
  expect(app.size, "the scan finds the app's keys").toBeGreaterThan(10);
  expect(Object.keys(MESSAGES_EN).map((key) => key.split(".")[0]!).filter((key) => app.has(key))).toEqual([]);
});
