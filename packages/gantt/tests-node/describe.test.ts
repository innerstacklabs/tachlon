// `describe` (to#23, to#24): each topic prints every exported option, event, command, token and start error, found by reading the declarations as text; --json is the same
// data; an unknown topic is exit 2; and every option, event, command and token the `fit` steps name is in the model, so a rename fails here and not in the field.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { buildModel } from "../../../scripts/lib/describe-model.ts";
import type { Item, Model } from "../src/cli/describe.ts";
import { run } from "../src/cli/run.ts";
import { PRODUCT_NAME } from "../src/product.ts";

const read = (file: string): string => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
const body = (text: string, header: string): string => text.slice(text.indexOf(header)).split(/^\}/m)[0]!;
const all = (text: string, re: RegExp): string[] => [...text.matchAll(re)].map((m) => m[1]!);

// What the declarations hold, read here and not from the model's own reader.
const declared = {
  options: all(body(read("src/gantt-types.ts"), "export interface MountOptions {"), /^ {2}(\w+)\??:/gm),
  events: all(body(read("src/events.ts"), "export interface GanttEvents {"), /^ {2}"?([\w-]+)"?:/gm),
  commands: [...new Set([...all(body(read("src/commands.ts"), "export interface Commands {"), /^ {2}(\w+)\(/gm), ...all(body(read("src/host-commands.ts"), "export interface HostCommands {"), /^ {2}(\w+)\(/gm)])],
  tokens: all(body(read("src/styles/tokens.css").replace(/\/\*[\s\S]*?\*\//g, ""), ":where(.gantt) {"), /(--gantt-[a-z0-9-]+)\s*:/g),
  errors: all(body(read("src/errors.ts"), "const FIXES = {"), /^ {2}([A-Z_]+): "/gm).map((cause) => `${PRODUCT_NAME.toUpperCase()}_${cause}`),
};
const bundle = { model: buildModel(new URL("../", import.meta.url).pathname), fit: read("src/cli/fit.md") };
const describe = (...argv: string[]) => run(["describe", ...argv], ".", undefined, undefined, bundle);
const names = (markdown: string): string[] => all(markdown, /^- `([\w$-]+)/gm);

for (const topic of Object.keys(declared) as (keyof typeof declared)[]) {
  test(`describe ${topic} prints every ${topic} the declarations hold, as markdown and as the same data with --json`, () => {
    assert.ok(declared[topic].length >= 8, `the ${topic} are found in the declarations`);
    const text = describe(topic);
    assert.equal(text.code, 0);
    assert.deepEqual(names(text.out).sort(), [...declared[topic]].sort());
    const json = JSON.parse(describe(topic, "--json").out) as { topic: string; items: { name: string; type: string; description: string }[] };
    assert.deepEqual(json.items.map((i) => i.name), names(text.out));
    for (const i of json.items as { name: string; type: string; default: string | null; description: string }[]) {
      assert.ok(i.type && i.description, `${i.name} has a type and a meaning`);
      assert.ok(text.out.includes(i.type) && text.out.includes(i.description), `${i.name}'s type and meaning are printed`);
      if (i.default !== null) assert.ok(text.out.includes(`Default: \`${i.default}\`.`), `${i.name}'s default is printed`);
    }
  });
}

test("an option's meaning starts its doc comment, and its default is the declaration's @defaultValue", () => {
  const source = body(read("src/gantt-types.ts"), "export interface MountOptions {");
  const tagged = all(source, /@defaultValue\s+`([^`]+)`/g);
  assert.ok(tagged.length >= 4, "the declarations carry defaults");
  const { items } = JSON.parse(describe("options", "--json").out) as { items: Item[] };
  assert.deepEqual(items.map((i) => i.default).filter((d) => d !== null), tagged);
  const comments = [...source.matchAll(/^ {2}\/\*\* (.*?)(?: \*\/|\n)[^]*?^ {2}(\w+)\??:/gm)];
  assert.ok(comments.length >= 8, "the comments are found");
  for (const [, text, name] of comments) assert.ok(text!.startsWith(items.find((i) => i.name === name)!.description), `${name}'s meaning starts its comment`);
});

test("describe with no topic lists the topics, and an unknown topic is exit 2 with the topics listed", () => {
  const list = describe();
  assert.equal(list.code, 0);
  for (const topic of [...Object.keys(declared), "fit"]) assert.match(list.out, new RegExp(` describe ${topic}\``));
  const bad = describe("nope");
  assert.equal(bad.code, 2);
  assert.match(bad.err, /unknown topic nope[^]* describe options`/);
});

// The lone words in backticks of `fit` that are not options, events, commands or tokens: record and payload members, HTML and CSS words, values, and the two functions it calls.
const OTHER = ["id", "key", "parent", "from", "to", "task", "link", "set", "was", "kind", "source", "changes", "commits", "snapshot", "seq", "data-theme", "class", "style", "lang", "dir", "div", "gantt", "script-src", "default-src", "inherit", "fail", "null", "undefined", "true", "false", "YYYY-MM-DD", "mount", "validate"];

// The names `fit` writes as a lone word in backticks that the model does not have, and the tokens it names (in a span or a code block) that the model does not have.
function unknown(fit: string, model: Model): string[] {
  const known = new Set([...model.options, ...model.events, ...model.commands, ...model.tokens].map((i) => i.name));
  const prose = fit.replace(/^```[\s\S]*?^```/gm, "");
  const words = all(prose, /`([\w-]+)`/g).filter((w) => !known.has(w) && !OTHER.includes(w));
  const tokens = all(fit, /(--gantt-[a-z0-9-]+)/g).filter((t) => !known.has(t));
  return [...new Set([...words, ...tokens])];
}

test("every option, event, command and token the fit steps name is in the model, and a renamed one is found", () => {
  assert.deepEqual(unknown(bundle.fit, bundle.model), []);
  for (const used of OTHER) assert.match(bundle.fit, new RegExp(`\\b${used}\\b`), `${used} is named in fit, so OTHER lists no stale word`);
  for (const [was, now] of [["`beforeEdit`", "`beforeEditor`"], ["`locale`", "`locales`"], ["`edit`", "`edits`"], ["`assignKey`", "`assignKeys`"], ["--gantt-bar-h", "--gantt-bar-height"]] as const) {
    assert.ok(bundle.fit.includes(was), `${was} is in fit`);
    assert.deepEqual(unknown(bundle.fit.replace(was, now), bundle.model), [now.replaceAll("`", "")], `renaming ${was}`);
  }
});

test("describe fit prints the steps with the package's own name in the commands, as markdown and as data", () => {
  const text = describe("fit");
  assert.match(text.out, /^# Make the chart fit your product/);
  assert.ok(!text.out.includes("{name}"), "the placeholder is filled");
  assert.match(text.out, new RegExp(`npx ${PRODUCT_NAME.toLowerCase()} describe tokens`));
  assert.equal((JSON.parse(describe("fit", "--json").out) as { markdown: string }).markdown, text.out);
});
