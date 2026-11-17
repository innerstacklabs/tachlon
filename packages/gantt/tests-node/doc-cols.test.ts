// The Theming section of docs/WASM_API.md gives --gantt-cols twice, in the `cols` row and in the layout paragraph; both equal tokens.css. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const read = (path: string): string => readFileSync(new URL(path, import.meta.url), "utf8");
test("the_doc_gives_the_cols_template_the_stylesheet_has_twice", () => {
  const doc = read("../../../docs/WASM_API.md");
  const css = read("../src/styles/tokens.css").match(/--gantt-cols:\s*([^;]+);/)?.[1];
  expect(css, "tokens.css sets --gantt-cols").toBeTruthy();
  expect(doc.match(/^\| `cols` \| `([^`]+)` \|/m)?.[1], "the cols row").toBe(css);
  expect(doc.match(/`--gantt-cols` \(the one grid template[^`]*`([^`]+)`/)?.[1], "the layout paragraph").toBe(css);
});
