// Check 8, the page lookups: no file under packages/gantt/src but the developer page's entry calls a page method on the
// document, its body or root element, or an element's ownerDocument. It reads each file's syntax tree (`parseAst`,
// the parser Vite ships), so comments, strings, templates and line breaks are the parser's, not a text scanner's.
// The window size and the key listeners stay in crates/app/tests/architecture/embedding.rs.
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { parseAst } from "vite";

/** The one file that may look parts up on the page or listen on it: the developer page's entry. */
const PAGE_ENTRY = "packages/gantt/src/main.ts";

/** The document's methods that find parts on the page or listen on it. */
const PAGE_METHODS = new Set([
  "getElementById",
  "querySelector",
  "querySelectorAll",
  "getElementsByClassName",
  "getElementsByTagName",
  "getElementsByName",
  "addEventListener",
]);

type Node = { type: string; start: number; end: number; [key: string]: unknown };

const isNode = (v: unknown): v is Node => typeof v === "object" && v !== null && typeof (v as Node).type === "string";

/** `node` without the wrappers that do not change what it names: `a?.b` (a chain), `a!` and `(a)`. */
function unwrap(node: Node): Node {
  while (["ChainExpression", "TSNonNullExpression", "ParenthesizedExpression"].includes(node.type)) {
    node = node.expression as Node;
  }
  return node;
}

/** The name of a member's property when it is written `a.name`, not `a[name]`. */
function memberName(node: Node): string | undefined {
  if (node.type !== "MemberExpression" || node.computed) return undefined;
  return (node.property as Node & { name?: string }).name;
}

const isIdentifier = (node: Node, names: string[]): boolean =>
  node.type === "Identifier" && names.includes((node as Node & { name: string }).name);

/** `document`, `window.document` (or on `globalThis` or `self`), or any `X.ownerDocument`. */
function isDocument(node: Node): boolean {
  node = unwrap(node);
  if (isIdentifier(node, ["document"])) return true;
  const name = memberName(node);
  if (name === "ownerDocument") return true;
  return name === "document" && isIdentifier(unwrap(node.object as Node), ["window", "globalThis", "self"]);
}

/** The document, or its body or root element. */
function isPage(node: Node): boolean {
  node = unwrap(node);
  const name = memberName(node);
  return isDocument(node) || ((name === "body" || name === "documentElement") && isDocument(node.object as Node));
}

/** Each page lookup in `text`, as `path:line has receiver.method`. */
export function pageLookups(path: string, text: string): string[] {
  const found: string[] = [];
  const visit = (node: Node): void => {
    if (node.type === "CallExpression") {
      const callee = unwrap(node.callee as Node);
      const method = memberName(callee);
      if (method && PAGE_METHODS.has(method) && isPage(callee.object as Node)) {
        const receiver = text.slice(callee.start, callee.end).replace(/[\s?!()]/g, "");
        const line = text.slice(0, callee.start).split("\n").length;
        found.push(`${path}:${line} has ${receiver}`);
      }
    }
    for (const value of Object.values(node)) {
      for (const child of Array.isArray(value) ? value : [value]) if (isNode(child)) visit(child);
    }
  };
  visit(parseAst(text, { lang: "ts" }) as unknown as Node);
  return found;
}

/** Whether `path` under packages/gantt/src is read: a `.ts` source, not a declaration and not the generated packages/gantt/src/wasm/. */
const isWebSource = (path: string): boolean =>
  path.endsWith(".ts") && !path.endsWith(".d.ts") && !path.startsWith("wasm/");

/** Every `.ts` source under packages/gantt/src, as [path from the repository root, text]. */
function webSources(): [string, string][] {
  const src = new URL("../src/", import.meta.url);
  return (readdirSync(src, { recursive: true }) as string[])
    .map((name) => name.split("\\").join("/"))
    .filter(isWebSource)
    .map((name) => [`packages/gantt/src/${name}`, readFileSync(new URL(name, src), "utf8")]);
}

const copy = (text: string): string[] => pageLookups("packages/gantt/src/copy.ts", text);

test("no file under packages/gantt/src but the page entry looks parts up on the page", () => {
  const files = webSources();
  expect(files.length, "the walk read the sources").toBeGreaterThanOrEqual(50);
  expect(files.map(([path]) => path), "the page entry is where this check thinks").toContain(PAGE_ENTRY);
  const failures = files.filter(([path]) => path !== PAGE_ENTRY).flatMap(([path, text]) => pageLookups(path, text));
  expect(
    failures,
    "a lookup or a listener on the page finds parts of another chart, so look inside the chart root " +
      "(root.querySelector) and listen on the root; only packages/gantt/src/main.ts, the developer page's entry, may use the document",
  ).toEqual([]);
});

test("the generated packages/gantt/src/wasm/ and declaration files are not read, as the Rust read skips them", () => {
  expect(["main.ts", "adapters/bryntum.ts"].every(isWebSource)).toBe(true);
  expect(["wasm/app.js", "wasm/app.d.ts", "wasm/app_bg.wasm.d.ts", "wasm/helper.ts", "env.d.ts"].some(isWebSource)).toBe(false);
});

test("each page method on the document fails", () => {
  for (const method of PAGE_METHODS) expect(copy(`a = document.${method}(x);\n`)).toEqual([`packages/gantt/src/copy.ts:1 has document.${method}`]);
});

test("the document through the window, globalThis or self fails", () => {
  const src = 'a = window.document.getElementById("y");\nglobalThis.document.addEventListener("pointerdown", f);\nself.document.querySelector(x);\n';
  expect(copy(src)).toEqual([
    "packages/gantt/src/copy.ts:1 has window.document.getElementById",
    "packages/gantt/src/copy.ts:2 has globalThis.document.addEventListener",
    "packages/gantt/src/copy.ts:3 has self.document.querySelector",
  ]);
});

test("a lookup on the body, the root element or an element's document fails", () => {
  const src = "a = document.body.querySelector(x);\nb = document.documentElement.querySelectorAll(x);\nc = el.ownerDocument.getElementsByName(x);\n";
  expect(copy(src)).toEqual([
    "packages/gantt/src/copy.ts:1 has document.body.querySelector",
    "packages/gantt/src/copy.ts:2 has document.documentElement.querySelectorAll",
    "packages/gantt/src/copy.ts:3 has el.ownerDocument.getElementsByName",
  ]);
});

test("an optional chain, a non-null or a bracketed receiver fails", () => {
  const src = "a = (document?.body).querySelector(x);\nb = (el?.ownerDocument)!.querySelector(x);\nc = document?.getElementById(x);\n";
  expect(copy(src)).toEqual([
    "packages/gantt/src/copy.ts:1 has document.body.querySelector",
    "packages/gantt/src/copy.ts:2 has el.ownerDocument.querySelector",
    "packages/gantt/src/copy.ts:3 has document.getElementById",
  ]);
});

test("a lookup on the root, another document method, or a lookup in a comment passes", () => {
  const src = 'root.querySelector("#x");\nroot.addEventListener("copy", f);\nel.getElementById("x");\ndocument.createElement("div");\n' +
    'document.getElementByIdx("x");\ndocument["querySelector"](x);\n// document.getElementById("x");\n/* document.body.querySelector(x) */\n';
  expect(copy(src)).toEqual([]);
});

test("a lookup inside a string passes and one inside a template expression fails", () => {
  const src = "a = \"document.querySelector(x)\";\nb = 'it\\'s document.body.querySelector(x)';\n" +
    "c = `document.getElementById(x) ${document.body.querySelector(s)} ${{ k: 1 }.k}`;\n";
  expect(copy(src)).toEqual(["packages/gantt/src/copy.ts:3 has document.body.querySelector"]);
});

test("TypeScript syntax around a lookup is read", () => {
  const src = "const a: HTMLElement | null = document.querySelector<HTMLElement>(x) as HTMLElement;\ntype T = { q: string };\n";
  expect(copy(src)).toEqual(["packages/gantt/src/copy.ts:1 has document.querySelector"]);
});

test("the page entry may use the document", () => {
  const files = webSources();
  const entry = files.find(([path]) => path === PAGE_ENTRY);
  expect(entry && pageLookups(...entry).length, "the entry's own lookups are found, then allowed").toBeGreaterThan(0);
});

test("escape: a brace in a comment inside a template expression", () => {
  expect(copy("a = `${ /* } */ document.querySelector(x) }`;\n")).toEqual(["packages/gantt/src/copy.ts:1 has document.querySelector"]);
});

test("escape: a dot then a newline", () => {
  expect(copy("a = document.\n  querySelector(x);\n")).toEqual(["packages/gantt/src/copy.ts:1 has document.querySelector"]);
});

test("escape: a chain split over lines", () => {
  expect(copy("a = document.\n  body\n  .querySelector(x);\n")).toEqual(["packages/gantt/src/copy.ts:1 has document.body.querySelector"]);
});
