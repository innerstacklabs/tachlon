// The README says how `init` finds the package manager (lockfile, then `packageManager`) and that doctor's CSP_CONNECT_SRC covers the page's own origin (tl#1079).
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const sentences = readFileSync(new URL("../README.md", import.meta.url), "utf8").split(/(?<=\.)\s+(?=[A-Z])/);
const sentenceWith = (needle: string): string => sentences.find((s) => s.includes(needle)) ?? "";
test("the_readme_says_init_reads_the_package_manager_field_after_the_lockfile", () => {
  expect(sentenceWith("its lockfile"), "the sentence on how init finds the manager names the field").toContain("packageManager");
});
test("the_readme_says_csp_connect_src_covers_the_pages_own_origin", () => {
  expect(sentenceWith("`CSP_CONNECT_SRC`"), "the sentence on CSP_CONNECT_SRC names the page's own origin").toContain("own origin");
});
