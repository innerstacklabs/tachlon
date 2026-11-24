// A code block the save and page chapters take from an example is verbatim in that example's file. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const root = new URL("../../../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), "utf8");

/** Each fenced block of a chapter, with the example file its lead-in paragraph names (`undefined` when it names none). */
function blocks(page: string): { file: string | undefined; code: string }[] {
  const found: { file: string | undefined; code: string }[] = [];
  let prose: string[] = [];
  let code: string[] | undefined;
  let file: string | undefined;
  for (const line of read(`book/src/${page}`).split("\n")) {
    if (line.startsWith("```")) {
      if (code === undefined) {
        const lead = prose.join("\n").trim().split(/\n\s*\n/).pop() ?? "";
        file = lead.match(/`(examples\/[\w./-]+\.\w+)`/)?.[1];
        code = [];
      } else {
        found.push({ file, code: code.join("\n") });
        code = undefined;
        prose = [];
      }
    } else if (code === undefined) prose.push(line);
    else code.push(line);
  }
  return found;
}

const cited = (page: string) => blocks(page).flatMap((b) => (b.file === undefined ? [] : [b.file]));

test("every_block_the_chapters_take_from_an_example_is_verbatim_in_its_file", () => {
  for (const page of ["dev-save.md", "dev-page.md"]) {
    for (const { file, code } of blocks(page)) {
      if (file !== undefined) expect(read(file), `${page}: a block that is not in ${file}`).toContain(code);
    }
  }
});

test("the_save_chapter_opens_with_the_save_handlers_of_the_save_example", () => {
  expect(cited("dev-save.md"), "the save chapter takes one block from the save example").toEqual(["examples/save/src/main.tsx"]);
  const [first] = blocks("dev-save.md").filter((b) => b.file !== undefined);
  for (const handler of ["createTask", "updateTask", "deleteTask", "createLink", "updateLink", "deleteLink", "updateSchedule"]) {
    expect(first?.code, `the block gives ${handler}`).toContain(`${handler}:`);
  }
});

test("the_page_chapter_shows_the_react_example", () => {
  expect(cited("dev-page.md"), "the page chapter takes its blocks from the plain and react examples").toEqual([
    "examples/plain/index.html",
    "examples/plain/main.js",
    "examples/react/src/main.tsx",
  ]);
});
