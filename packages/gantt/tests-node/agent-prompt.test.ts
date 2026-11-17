// The prompt on the package README's first screen (gantt#1560): short, tied to no assistant, above the install section.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { agentPrompt } from "../../../scripts/lib/agent-prompt.ts";
import { chartModule } from "../src/cli/template.ts";
import { PRODUCT_NAME } from "../src/product.ts";

const readme = readFileSync(new URL("../README.md", import.meta.url), "utf8");

test("the README's prompt is under 60 words, names the product and its AGENTS.md, and names no assistant", () => {
  const prompt = agentPrompt(readme);
  assert.ok(prompt.split(/\s+/).length < 60, `${prompt.split(/\s+/).length} words`);
  assert.ok(prompt.toLowerCase().includes(PRODUCT_NAME.toLowerCase()) && prompt.includes("AGENTS.md"), prompt);
  assert.doesNotMatch(prompt, /claude|codex|cursor|copilot|gemini|chatgpt/i);
});

test("AGENTS.md names the two shapes `init` writes: the function and the component", () => {
  const guide = readFileSync(new URL("../AGENTS.md", import.meta.url), "utf8");
  const plain = /export async function (\w+)/.exec(chartModule("plain", false, PRODUCT_NAME))?.[1];
  const react = /export default function (\w+)/.exec(chartModule("react", false, PRODUCT_NAME))?.[1];
  for (const name of [plain, react]) assert.ok(guide.includes(`\`${name}\``), `AGENTS.md does not name \`${name}\``);
});

test("AGENTS.md says to run `git init` when the app has no git repository", () => {
  const guide = readFileSync(new URL("../AGENTS.md", import.meta.url), "utf8");
  assert.ok(guide.includes("`git init`"), "AGENTS.md has no no-repository case");
});

test("AGENTS.md allows a Content-Security-Policy change for CSP_CONNECT_SRC as well as CSP_WASM_EVAL", () => {
  const guide = readFileSync(new URL("../AGENTS.md", import.meta.url), "utf8");
  assert.ok(guide.includes("`CSP_CONNECT_SRC`"), "AGENTS.md does not name `CSP_CONNECT_SRC`");
});

test("the prompt sits above the Install section, and a README without one is an error, not an empty prompt", () => {
  assert.ok(readme.indexOf(agentPrompt(readme)) < readme.indexOf("\n## Install"));
  assert.throws(() => agentPrompt("# title\n\n## Install\n"), /no "Using an AI assistant\? Paste this:" line/);
  assert.throws(() => agentPrompt("Using an AI assistant? Paste this:\n\nno block\n"), /fenced block/);
});
