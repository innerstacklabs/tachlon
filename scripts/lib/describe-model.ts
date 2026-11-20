// The model `describe` prints (to#23): the options, events, commands, methods, tokens, start errors and refusal reasons of the package, read as text from the declarations and the
// stylesheet, so nothing is listed by hand. Each item has its name, its type, its default (the declaration's `@defaultValue`, else none) and the first
// sentence of its doc comment (`summary` is how api-docs-check reads one). `scripts/emit-describe.ts` writes it into the package.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Item, Model } from "../../packages/gantt/src/cli/describe.ts";
import { summary } from "./api-docs-check.ts";

const oneLine = (text: string): string => text.replace(/\s+/g, " ").trim();
const firstSentence = (text: string): string => oneLine(text).match(/^.+?[.!?](?=\s+\S|$)/)?.[0] ?? oneLine(text);

type Member = { doc: string; text: string };

/** The members of `export interface <header>`, each with the doc comment before it and its text up to the `;`. */
function members(source: string, header: RegExp): Member[] {
  const start = header.exec(source);
  if (!start) throw new Error(`no ${header} in the source`);
  const out: Member[] = [];
  let doc = "";
  let text = "";
  let depth = 0;
  for (let i = start.index + start[0].length; i < source.length; ) {
    const c = source[i]!;
    if (source.startsWith("/*", i)) {
      const end = source.indexOf("*/", i) + 2;
      if (depth === 0 && text.trim() === "" && source.startsWith("/**", i)) doc = source.slice(i, end);
      i = end;
    } else if (source.startsWith("//", i)) {
      i = source.indexOf("\n", i) + 1 || source.length;
    } else if (c === "}" && depth === 0) {
      break;
    } else if (c === ";" && depth === 0) {
      if (text.trim()) out.push({ doc, text: text.trim() });
      doc = text = "";
      i++;
    } else {
      depth += "{([".includes(c) ? 1 : "})]".includes(c) ? -1 : 0;
      text += c;
      i++;
    }
  }
  return out;
}

const NAME = /^(?:readonly\s+)?(?:"([^"]+)"|([\w$]+))\??\s*(?=[(:])/;

function item({ doc, text }: Member): Item {
  const hit = NAME.exec(text);
  if (!hit) throw new Error(`cannot read the member "${text.slice(0, 40)}"`);
  const rest = oneLine(text.slice(hit[0].length));
  return { name: (hit[1] ?? hit[2])!, type: rest.startsWith(":") ? rest.slice(1).trim() : rest, default: /@defaultValue[ \t]+`?([^`\n]+)`?/.exec(doc)?.[1]?.trim() ?? null, description: firstSentence(summary(doc)) };
}

/** A method keeps its whole doc comment, on one line, for the refusals and limits that its later sentences state. */
const method = (member: Member): Item => ({ ...item(member), description: oneLine(summary(member.doc)) });

/** The reasons of `export type EditRefusal`: an item for each comment group, named by its reasons (`a, b`) and described by the whole comment, which states the fix where there is one. */
export const refusals = (source: string): Item[] =>
  [...(/export type EditRefusal =([\s\S]*?)\n\n/.exec(source)?.[1] ?? "").matchAll(/(\/\*\*[\s\S]*?\*\/)\s*((?:\|\s*"\w+"\s*)+)/g)].map((m) => ({ name: [...m[2]!.matchAll(/"(\w+)"/g)].map((r) => r[1]).join(", "), type: "refusal", default: null, description: oneLine(summary(m[1]!)) }));

/** The `--gantt-*` tokens the base block of `tokens.css` declares, with their value and the dark value of a colour; the meaning is the "Used for" cell of a Theming table in `docs/WASM_API.md` (the colours, then the layout), else the comment right above it. */
export function tokens(css: string, api: string): Item[] {
  const block = (open: string): string => css.slice(css.indexOf("{", css.indexOf(open)) + 1, css.indexOf("}", css.indexOf(open)));
  const dark = new Map([...block(':where(.gantt[data-theme="dark"])').matchAll(/(--gantt-[a-z-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1]!, oneLine(m[2]!)]));
  const used = new Map([...api.matchAll(/^\| `([\w-]+)` \|(?: `[^`]+` \|){1,2} (.+) \|$/gm)].map((m) => [`--gantt-${m[1]}`, firstSentence(m[2]!)]));
  const out: Item[] = [];
  let comment = "";
  for (const m of block(":where(.gantt) {").matchAll(/\/\*([\s\S]*?)\*\/|(--gantt-[a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    if (m[1] !== undefined) {
      comment = m[1];
      continue;
    }
    const value = oneLine(m[3]!);
    out.push({ name: m[2]!, type: /^#[0-9a-f]+$/i.test(value) ? "color" : "CSS value", default: dark.has(m[2]!) ? `${value} (light), ${dark.get(m[2]!)} (dark)` : value, description: used.get(m[2]!) ?? firstSentence(comment) });
    comment = "";
  }
  return out;
}

/** The start errors: the code (the product name in capitals, an underscore and the cause) and the first sentence of its fix. */
export function errors(source: string, product: string): Item[] {
  const body = /const FIXES = \{([\s\S]*?)\n\} as const;/.exec(source)?.[1] ?? "";
  return [...body.matchAll(/^\s*([A-Z_]+): ("(?:[^"\\]|\\.)*"),?$/gm)].map((m) => ({ name: `${product.toUpperCase()}_${m[1]}`, type: "error code", default: null, description: firstSentence(JSON.parse(m[2]!) as string) }));
}

/** The model of the package in `pkg` (its folder, two below the repository root). */
export function buildModel(pkg: string): Model {
  const read = (file: string): string => readFileSync(join(pkg, file), "utf8");
  const documented = (a: Item[], b: Item[]): Item[] => [...a, ...b.filter((x) => !a.some((y) => y.name === x.name))];
  const commands = documented(
    members(read("src/commands.ts"), /export interface Commands \{/).map(item),
    members(read("src/host-commands.ts"), /export interface HostCommands \{/).map(item),
  );
  const product = /PRODUCT_NAME = "([^"]+)"/.exec(read("src/product.ts"))?.[1] ?? "";
  return {
    options: members(read("src/gantt-types.ts"), /export interface MountOptions[^{]*\{/).map(item),
    events: members(read("src/events.ts"), /export interface GanttEvents \{/).map(item),
    commands,
    methods: [...members(read("src/events.ts"), /export interface HasOn \{/), ...members(read("src/gantt-types.ts"), /export interface Gantt<[^{]*\{/)].map(method),
    tokens: tokens(read("src/styles/tokens.css"), read("../../docs/WASM_API.md")),
    errors: errors(read("src/errors.ts"), product),
    refusals: refusals(read("src/edit-result.ts")),
  };
}
