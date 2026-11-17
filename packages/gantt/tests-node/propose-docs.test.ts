import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const source = (path: string): string => readFileSync(new URL(path, import.meta.url), "utf8");
test("the comment on propose names every kind of ProposedChange and every reason of ProposeResult", () => {
  const types = source("../src/propose.ts");
  const kinds = [...(/export type ProposedChange =([^]*?)\n\n/.exec(types)?.[1] ?? "").matchAll(/kind: "(\w+)"/g)].map((m) => m[1]);
  const reasons = [...(/export type ProposeResult = .*?reason: ([^;]*?); message/.exec(types)?.[1] ?? "").matchAll(/"(\w+)"/g)].map((m) => m[1]);
  const comment = /\/\*\*((?:(?!\*\/)[^])*)\*\/\s*propose\(/.exec(source("../src/commands.ts"))?.[1] ?? "";
  expect([kinds, reasons]).toEqual([["move", "duration", "progress", "link", "unlink"], ["badDate", "finishBeforeStart", "actualAfterDataDate", "duration", "badPercent", "badLinkText", "loop", "alreadyLinked", "noLink"]]);
  for (const word of [...kinds, ...reasons]) expect(comment, word).toContain(`"${word}"`);
});
