// README "Try it" (gantt#1868): the first sentence is the no-install route; the build requirements follow it and come before the first code block.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const readme = readFileSync(new URL("../../../README.md", import.meta.url), "utf8");
test("try_it_opens_with_the_hosted_app_and_the_build_requirements_follow", () => {
  const section = readme.split("\n## Try it\n")[1]!.split("\n## ")[0]!, at = (text: string) => section.indexOf(text);
  expect(section.trim().split("\n\n")[0]!.split(". ")[0], "the first sentence names the hosted app and Open…").toMatch(/hosted app.*Open…/);
  expect(at("developers who build from source"), "the developer sentence follows the hosted route").toBeGreaterThan(at("Open…"));
  expect(at("wasm-bindgen-cli"), "the requirements follow the developer sentence").toBeGreaterThan(at("developers who build from source"));
  expect(at("```"), "the first code block follows the requirements").toBeGreaterThan(at("wasm-bindgen-cli"));
});
