// The root README's front page: the link row comes before the first code block, the first code block is the assistant prompt
// the package README carries, and the prompt comes before the manual install.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { agentPrompt } from "../../../scripts/lib/agent-prompt.ts";
import { PRODUCT_NAME } from "../src/product.ts";

const readme = readFileSync(new URL("../../../README.md", import.meta.url), "utf8");
const packageReadme = readFileSync(new URL("../README.md", import.meta.url), "utf8");
const site = `https://${PRODUCT_NAME.toLowerCase()}.com`;
const firstFence = readme.indexOf("```");

test("the website and the live demo are linked before the first code block", () => {
  const head = readme.slice(0, firstFence);
  expect(firstFence, "the README has a code block").toBeGreaterThan(0);
  expect(head, "the website link").toContain(`[Website](${site})`);
  expect(head, "the live demo link").toContain(`[Live demo](https://app.${PRODUCT_NAME.toLowerCase()}.com)`);
});

test("the first code block is the assistant prompt the package README carries, and it comes before npm install", () => {
  const block = /^```\w*\n([\s\S]*?)\n```/m.exec(readme.slice(firstFence));
  expect(block, "the first fence closes").not.toBeNull();
  expect(block![1], "the first code block equals the package README's prompt").toBe(agentPrompt(packageReadme));
  expect(readme.indexOf("npm install"), "the manual install follows the prompt").toBeGreaterThan(firstFence + block![0].length);
});
