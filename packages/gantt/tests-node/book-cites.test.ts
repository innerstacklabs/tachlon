// Every test file the book names (`.spec.ts`, `.test.ts` or `.rs` under a `tests/` or `tests-node/` folder) is a file in the repo. Read as text.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const root = new URL("../../../", import.meta.url);
const cites = /[\w./-]+\/tests(?:-node)?\/[\w./-]+\.(?:spec\.ts|test\.ts|rs)\b/g;

test("every_test_file_the_book_names_exists", () => {
  const pages = readdirSync(new URL("book/src/", root)).filter((name) => name.endsWith(".md"));
  expect(pages.length, "the book pages are found").toBeGreaterThan(10);
  const missing = pages.flatMap((page) =>
    [...readFileSync(new URL(`book/src/${page}`, root), "utf8").matchAll(cites)].map((m) => m[0]).filter((path) => !existsSync(new URL(path, root))).map((path) => `${page}: ${path}`),
  );
  expect(missing, "test files the book names that are not in the repo").toEqual([]);
});

test("the_task_dialog_chapter_cites_both_dialog_specs_and_they_exist", () => {
  const cited = [...readFileSync(new URL("book/src/j10-task-dialog.md", root), "utf8").matchAll(cites)].map((m) => m[0]);
  expect(cited.filter((path) => !existsSync(new URL(path, root))), "dialog specs the chapter names that are not in the repo").toEqual([]);
  expect(cited, "the chapter names both dialog specs").toEqual(expect.arrayContaining(["packages/gantt/tests/task-dialog.spec.ts", "packages/gantt/tests/task-dialog-open.spec.ts"]));
});
