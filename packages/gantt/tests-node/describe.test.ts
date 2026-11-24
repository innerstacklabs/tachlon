// `describe` (to#23, to#24): each topic prints every exported option, event, command, token and start error, found by reading the declarations as text; --json is the same
// data; an unknown topic is exit 2; and every option, event, command and token the `fit` steps name is in the model, so a rename fails here and not in the field.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import { buildModel } from "../../../scripts/lib/describe-model.ts";
import { type Item, type Model, readBundle } from "../src/cli/describe.ts";
import { Refusal } from "../src/cli/detect.ts";
import { run } from "../src/cli/run.ts";
import { PRODUCT_NAME } from "../src/product.ts";

const read = (file: string): string => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
const body = (text: string, header: string): string => text.slice(text.indexOf(header)).split(/^\}/m)[0]!;
const all = (text: string, re: RegExp): string[] => [...text.matchAll(re)].map((m) => m[1]!);

// What the declarations hold, read here and not from the model's own reader.
const declared = {
  options: all(body(read("src/gantt-types.ts"), "export interface MountOptions<F extends Fields | undefined = undefined> {"), /^ {2}(\w+)\??:/gm),
  events: all(body(read("src/events.ts"), "export interface GanttEvents {"), /^ {2}"?([\w-]+)"?:/gm),
  commands: [...new Set([...all(body(read("src/commands.ts"), "export interface Commands {"), /^ {2}(\w+)\(/gm), ...all(body(read("src/host-commands.ts"), "export interface HostCommands {"), /^ {2}(\w+)\(/gm)])],
  methods: ["on", ...all(body(read("src/gantt-types.ts"), "export interface Gantt<F extends Fields | undefined = undefined> extends HasOn {"), /^ {2}(\w+)[(:]/gm)],
  tokens: all(body(read("src/styles/tokens.css").replace(/\/\*[\s\S]*?\*\//g, ""), ":where(.gantt) {"), /(--gantt-[a-z0-9-]+)\s*:/g),
  errors: all(body(read("src/errors.ts"), "const FIXES = {"), /^ {2}([A-Z_]+): "/gm).map((cause) => `${PRODUCT_NAME.toUpperCase()}_${cause}`),
};
const bundle = { model: buildModel(new URL("../", import.meta.url).pathname), fit: read("src/cli/fit.md"), guide: read("guide/PROMPT.md") };
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
  const source = body(read("src/gantt-types.ts"), "export interface MountOptions<F extends Fields | undefined = undefined> {");
  const tagged = all(source, /@defaultValue\s+`([^`]+)`/g);
  assert.ok(tagged.length >= 4, "the declarations carry defaults");
  const { items } = JSON.parse(describe("options", "--json").out) as { items: Item[] };
  assert.deepEqual(items.map((i) => i.default).filter((d) => d !== null), tagged);
  const comments = [...source.matchAll(/^ {2}\/\*\* (.*?)(?: \*\/|\n)[^]*?^ {2}(\w+)\??:/gm)];
  assert.ok(comments.length >= 8, "the comments are found");
  for (const [, text, name] of comments) assert.ok(text!.startsWith(items.find((i) => i.name === name)!.description), `${name}'s meaning starts its comment`);
});

test("describe options lists density with its three names and its default", () => {
  const { items } = JSON.parse(describe("options", "--json").out) as { items: Item[] };
  assert.equal(items.find((i) => i.name === "density")?.default, "\"normal\"");
  assert.match(describe("options").out, /- `density`[^]*?Default: `"normal"`\./);
});

test("describe with no topic lists the topics, and an unknown topic is exit 2 with the topics listed", () => {
  const list = describe();
  assert.equal(list.code, 0);
  for (const topic of [...Object.keys(declared), "refusals", "fit", "guide"]) assert.match(list.out, new RegExp(` describe ${topic}\``));
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

test("describe guide prints the prompt with the package's own name in the commands, as markdown and as data", () => {
  assert.ok(bundle.guide.includes("{name}"), "the prompt holds the placeholder");
  const text = describe("guide");
  assert.equal(text.code, 0);
  assert.ok(!text.out.includes("{name}"), "the placeholder is filled");
  assert.match(text.out, new RegExp(`npx ${PRODUCT_NAME.toLowerCase()} describe options`));
  assert.equal((JSON.parse(describe("guide", "--json").out) as { markdown: string }).markdown, text.out);
});

test("a pack without guide/PROMPT.md is refused with the reinstall sentence, and one with it gives the prompt", () => {
  const dir = mkdtempSync(join(tmpdir(), "describe-pack-"));
  try {
    writeFileSync(join(dir, "describe.json"), JSON.stringify(bundle.model));
    writeFileSync(join(dir, "fit.md"), bundle.fit);
    const pack = pathToFileURL(`${dir}/`);
    assert.throws(() => readBundle(pack), (e) => e instanceof Refusal && e.message === "guide/PROMPT.md is not in this install; reinstall the package");
    mkdirSync(join(dir, "guide"));
    writeFileSync(join(dir, "guide", "PROMPT.md"), bundle.guide);
    assert.equal(readBundle(pack).guide, bundle.guide);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("the hostEdit entry names its two added reasons and sends both to update; methods and commands point to refusals", () => {
  const hostEdit = bundle.model.methods.find((m) => m.name === "hostEdit")?.description ?? "";
  for (const word of ["hasActuals", "notAllowed", "For both, call `update`"]) assert.ok(hostEdit.includes(word), `hostEdit says ${word}`);
  for (const topic of ["methods", "commands"]) assert.match(describe(topic).out, / describe refusals`\.\n$/);
});

test("describe refusals prints every reason of EditRefusal, and every reason a method comment names is one of them", () => {
  const union = read("src/edit-result.ts").split("export type EditRefusal =")[1]!.split("\n\n")[0]!.replace(/\/\*\*.*\*\//g, "");
  const reasons = all(union, /"(\w+)"/g);
  const out = describe("refusals").out;
  assert.ok(reasons.length >= 30 && reasons.every((r) => out.includes(`\`${r}\``)), "every reason is printed");
  assert.deepEqual((JSON.parse(describe("refusals", "--json").out) as { items: Item[] }).items.flatMap((i) => i.name.split(", ")), reasons);
  const text = bundle.model.methods.map((m) => m.description).join("\n");
  const camel = "([a-z]+[A-Z]\\w*)";
  const named = [...all(text, new RegExp(`\`"${camel}"\``, "g")), ...all(text, new RegExp(`\\bis \`${camel}\``, "g")), ...all(text.match(/EditRefusal`: ([^)]*)\)/)![1]!, new RegExp(`\`${camel}\``, "g"))];
  assert.ok(named.length >= 10, "the reasons the comments name are found");
  for (const n of named) assert.ok(reasons.includes(n), `${n} is a reason of EditRefusal`);
});
