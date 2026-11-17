// `doctor`: reads a project that lists the package and says, check by check, whether the chart is installed right. It writes nothing.
// A `Finding` is a code, a status, what was seen and one sentence to fix it; `run.ts` prints them and picks the exit code (1 when one failed).
import { existsSync, readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { validate } from "../adapters/validate.ts";
import { codeFor, fixFor } from "../errors.ts";
import { allowsConnect, allowsWasm, configPolicies, wasmDirective } from "./csp.ts";
import { type Project, readJsonObject, Refusal } from "./detect.ts";
import { BROWSER_VARIABLE, type Gpu } from "./gpu.ts";
import type { Own } from "./init.ts";
import type { Probe } from "./probe.ts";

export interface Finding {
  /** Names the check and its result: `ENGINE_OK`, `ENGINE_SIZE`. */
  code: string;
  status: "pass" | "fail" | "skipped";
  what: string;
  /** One sentence that says what to do; empty on a pass. */
  fix: string;
}

/** What `doctor` is given beyond the project: `data` is a plain task data file to check, `probe` what `--url` read from the page and its engine address, `gpu` what a browser said about the graphics, or a function that is called only if the check is reached (none asked: the check is skipped). */
export interface Checks {
  data: string | undefined;
  probe?: Probe;
  gpu?: Gpu | (() => Gpu | undefined);
}

const pass = (code: string, what: string): Finding => ({ code, status: "pass", what, fix: "" });
const fail = (code: string, what: string, fix: string): Finding => ({ code, status: "fail", what, fix });
const MAGIC = [0x00, 0x61, 0x73, 0x6d];
const why = (e: unknown): string => (e instanceof Error ? e.message : String(e)).replace(/\s+/g, " ");

/** Every distinct copy of package `name` the project can load: the nearest `node_modules/<name>` first, then the ones nested one level down (inside another package, or in pnpm's store). */
export function copiesOf(dir: string, name: string): string[] {
  let modules: string | undefined;
  for (let d = dir; modules === undefined; d = dirname(d)) {
    if (existsSync(join(d, "node_modules", name, "package.json"))) modules = join(d, "node_modules");
    else if (dirname(d) === d) return [];
  }
  const found = new Set<string>([realpathSync(join(modules, name))]);
  const nested = (holder: string) => {
    const copy = join(holder, "node_modules", name);
    if (existsSync(join(copy, "package.json"))) found.add(realpathSync(copy));
  };
  for (const entry of readdirSync(modules)) {
    const sub = join(modules, entry);
    if (entry === ".pnpm" || entry.startsWith("@")) readdirSync(sub).forEach((s) => nested(join(sub, s)));
    else if (!entry.startsWith(".")) nested(sub);
  }
  return [...found];
}

/** The engine size the package's build recorded in `copy` (`dist-lib/engine-size.json`, written by `scripts/emit-engine-size.ts`); undefined when the file is missing or holds no whole number. */
function recordedSize(copy: string): number | undefined {
  try {
    const { bytes } = JSON.parse(readFileSync(join(copy, "dist-lib", "engine-size.json"), "utf8")) as { bytes?: unknown };
    return typeof bytes === "number" && Number.isInteger(bytes) && bytes > 0 ? bytes : undefined;
  } catch {
    return undefined;
  }
}

/** The engine in the installed copy: there, starts `\0asm`, and the size the copy's own build recorded (never a file read from the running install, which may be the copy itself). */
function engineFinding(project: Project, copy: string): Finding {
  const file = join(copy, "dist-lib", "app_bg.wasm");
  const shown = relative(project.dir, file);
  const again = `Delete the package from node_modules and install it again with \`${project.manager} install\``;
  if (!existsSync(file)) return fail("ENGINE_MISSING", `${shown} is missing`, `${again}, and stop any cleanup step that deletes .wasm files.`);
  const bytes = readFileSync(file);
  if (!MAGIC.every((b, i) => bytes[i] === b)) return fail("ENGINE_NOT_WASM", `${shown} does not start with "\\0asm"`, `${again}: the file is not the engine, which a half-finished install or a tool that rewrites .wasm files leaves behind.`);
  const want = recordedSize(copy);
  if (want === undefined) return fail("ENGINE_UNRECORDED", `${relative(project.dir, join(copy, "dist-lib", "engine-size.json"))} is missing or unreadable, so the engine's size cannot be checked`, `${again}: the package's build records the engine's size there, and a copy without it is not a whole package.`);
  if (bytes.length !== want) return fail("ENGINE_SIZE", `${shown} is ${bytes.length} bytes; the size recorded when the package was built is ${want} bytes`, `${again}: a file of another size is cut short or comes from another version of the package.`);
  return pass("ENGINE_OK", `${shown} starts with "\\0asm" and is ${want} bytes, the size recorded when the package was built`);
}

/** A Next app serves the engine from its own copy, `public/app_bg.wasm` (`init` writes it and never overwrites it): there, starts `\0asm`, and the size the installed package recorded, so a copy left from an older version is named. None when the package recorded no size (`engineFinding` fails that). */
function engineCopyFindings(project: Project, copy: string): Finding[] {
  const file = join(project.dir, "public", "app_bg.wasm");
  const shown = relative(project.dir, file);
  const from = relative(project.dir, join(copy, "dist-lib", "app_bg.wasm"));
  const refresh = `Copy the installed engine over it with \`cp ${from} ${shown}\``;
  if (!existsSync(file)) return [fail("ENGINE_COPY_MISSING", `${shown} is missing`, `${refresh}, or run init again to write it.`)];
  const want = recordedSize(copy);
  if (want === undefined) return [];
  const bytes = readFileSync(file);
  const wasm = MAGIC.every((b, i) => bytes[i] === b);
  const source = join(copy, "dist-lib", "app_bg.wasm");
  // The installed engine itself when it is there (a build of the same size can hold other code); `engineFinding` fails a missing or cut-short one.
  const other = existsSync(source) && !bytes.equals(readFileSync(source));
  if (bytes.length !== want || !wasm || other) {
    const how = !wasm ? `${bytes.length} bytes and does not start with "\\0asm"` : bytes.length === want ? `${want} bytes like the installed engine, but its content differs` : `${bytes.length} bytes`;
    return [fail("ENGINE_COPY_STALE", `${shown} is ${how}; the engine in the installed package is ${want} bytes`, `${refresh}: a copy that differs comes from another version of the package, and the page would run that engine.`)];
  }
  return [pass("ENGINE_COPY_OK", `${shown} starts with "\\0asm" and is the same ${want} bytes as the engine in the installed package`)];
}

/** The page's Content-Security-Policy: the project's config and, with `--url`, the served page. The engine needs `'wasm-unsafe-eval'`, and `connect-src` must allow the engine, even one on the page's own origin (`'self'` matches it; checked only against a served policy and a known engine address, `engineKnown`). Every fault of a policy is reported in one run. No policy found, no finding. */
function cspFindings(project: Project, probe: Probe | undefined, engineKnown: boolean): Finding[] {
  const out: Finding[] = [];
  const policies = [...configPolicies(project.dir), ...(probe?.policies ?? [])];
  const [page, engine] = probe && engineKnown ? [new URL(probe.page), new URL(probe.engine.url)] : [];
  for (const p of policies) {
    const directive = wasmDirective(p.text);
    if (!allowsWasm(p.text)) out.push(fail("CSP_WASM_EVAL", `${p.source} sets a policy whose ${directive} does not allow 'wasm-unsafe-eval', so the browser refuses to compile the engine`, `Add 'wasm-unsafe-eval' to the ${directive} directive in ${p.source}.`));
    if (page && engine && probe?.policies.includes(p) && !allowsConnect(p.text, engine, page)) out.push(fail("CSP_CONNECT_SRC", `${p.source} sets a policy whose connect-src (or default-src) does not allow the engine at ${engine.origin}`, engine.origin === page.origin ? `Add 'self' to the connect-src (or default-src) directive in the page's policy.` : `Add ${engine.origin} to the connect-src directive in the page's policy, or serve the engine from the page's own origin.`));
  }
  return policies.length > 0 && out.length === 0 ? [pass("CSP_OK", `the policy in ${[...new Set(policies.map((p) => p.source))].join(", ")} lets the engine compile${page && probe?.policies.length ? " and connect" : ""}`)] : out;
}

/** The engine at the address the page names, as served: status 200, starts `\0asm`, type `application/wasm`; in a Next app also byte-equal to the installed engine (`installed`). A page that names none has no address to check outside a Next app, whose engine `init` puts at `/app_bg.wasm`: there is nothing to fail, so the check is skipped. */
function servedFindings({ engine: e }: Probe, installed: string | undefined, next: boolean): Finding[] {
  if (!e.named && !next) return [{ code: "ENGINE_URL_SKIPPED", status: "skipped", what: "not checked: the page names no .wasm address, in its HTML or in the module scripts it loads (a Vite build can name its hashed engine file in a script it loads later)", fix: "Open the page in a browser, take the address of the .wasm file from its network panel and request it: it must answer 200 as application/wasm." }];
  const serve = `Serve the engine file at ${e.url}`;
  if (e.status === null) return [fail("ENGINE_FETCH", `${e.url} could not be requested: ${e.error}`, `${serve} and check that the address is reachable.`)];
  if (e.status !== 200) return [fail("ENGINE_FETCH", `${e.url} answered with status ${e.status}`, `${serve}; a missing file or a blocked path answers with another status.`)];
  if (!MAGIC.every((b, i) => e.bytes[i] === b)) return [fail("ENGINE_NOT_WASM", `${e.url} does not start with "\\0asm": it starts with "${e.bytes.subarray(0, 8).toString("latin1").replace(/[^\x20-\x7e]/g, "?")}"`, `${serve}: a fallback that answers every unknown path with the page hides a missing file.`)];
  if (!/^application\/wasm\s*(?:;|$)/i.test(e.type)) return [fail("ENGINE_CONTENT_TYPE", `${e.url} answers with the type "${e.type}", not "application/wasm"`, `Serve .wasm files as application/wasm, which the browser needs to compile the engine as it downloads.`)];
  if (installed !== undefined && existsSync(installed) && !e.bytes.equals(readFileSync(installed))) return [fail("ENGINE_URL_STALE", `${e.url} serves ${e.bytes.length} bytes that differ from the engine in the installed package`, `Copy the installed engine to public/app_bg.wasm and deploy again: the site serves an engine of another version.`)];
  return [pass("ENGINE_URL_OK", `${e.url} answers 200 with ${e.bytes.length} bytes of WebAssembly as application/wasm`)];
}

const SOURCE = /\.(?:[cm]?[jt]sx?|vue|svelte|astro|s?css|html)$/;
const SKIPPED_FOLDERS = new Set(["node_modules", "dist", "build", "out", "coverage"]);

const COMMENT = /^(?:\/\/|\/\*|\*|#|<!--)/;
const SPECIFIER = /\b(?:import|require)\b[^'"`(]*\(?\s*['"`]?~?([^'"`)\s;]+)/g; // the quotes are optional inside `url(`; a leading `~` is the webpack prefix
/** Whether `text` has a line that is not a comment and names `needle` in quotes after `import`, `@import`, `require(` or `import(`. A mention inside a multi-line template string is not checked. */
function imports(text: string, needle: string): boolean {
  return text.split("\n").some((l) => !COMMENT.test(l.trim()) && [...l.matchAll(SPECIFIER)].some((m) => m[1] === needle));
}

/** Whether some source file under `dir` imports `needle`; node_modules, build output and dot folders are not source. */
function mentions(dir: string, needle: string): boolean {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.name.startsWith(".")) continue;
    const path = join(dir, e.name);
    if (e.isDirectory()) {
      if (!SKIPPED_FOLDERS.has(e.name) && mentions(path, needle)) return true;
    } else if (e.isFile() && SOURCE.test(e.name) && statSync(path).size < 1e6 && imports(readFileSync(path, "utf8"), needle)) return true;
  }
  return false;
}

/** `validate()` on the JSON in `file`: one finding for each issue. A file that cannot be read is a refusal (exit 2); one that is not JSON is a finding. */
function dataFindings(file: string): Finding[] {
  let text: string;
  try {
    text = readFileSync(file, "utf8");
  } catch (e) {
    throw new Refusal(`${file} cannot be read: ${why(e)}`);
  }
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch (e) {
    return [fail("DATA_NOT_JSON", `${file} is not valid JSON: ${why(e)}`, "Fix the JSON syntax the message names, then run doctor again.")];
  }
  const result = validate(data);
  if (result.ok) return [pass("DATA_OK", `${file} passes validate()`)];
  return result.issues.map((i) => fail("DATA_INVALID", `${i.path || "data"}: expected ${i.expected}, got ${i.got}`, i.fix));
}

/** The findings for `project`: its package listed and installed once, the engine, a stylesheet import, the data file when given, and the GPU the browser reported (skipped when none answered). */
export function diagnose(project: Project, own: Own, c: Checks): Finding[] {
  const pkg = readJsonObject(join(project.dir, "package.json")) as { dependencies?: Record<string, string>; devDependencies?: Record<string, string> };
  if (!Object.hasOwn({ ...pkg.dependencies, ...pkg.devDependencies }, own.name)) {
    return [fail("NOT_LISTED", `package.json lists no ${own.name}`, `Run \`npx ${own.name} init\` here, or add ${own.name} with \`${project.manager} ${project.manager === "npm" ? "install" : "add"} ${own.name}\`.`)];
  }
  const copies = copiesOf(project.dir, own.name);
  const version = (copy: string) => String(readJsonObject(join(copy, "package.json")).version);
  const at = (copy: string) => `${version(copy)} at ${relative(project.dir, copy)}`;
  const out: Finding[] = [];
  if (copies.length === 0) out.push(fail("NOT_INSTALLED", `package.json lists ${own.name} but node_modules holds no copy`, `Run \`${project.manager} install\` in ${project.dir}.`));
  else {
    out.push(pass("INSTALLED", `${own.name} ${at(copies[0])}`));
    out.push(copies.length === 1 ? pass("ONE_COPY", "one copy is installed") : fail("MANY_COPIES", `${copies.length} copies are installed: ${copies.map(at).join(", ")}`, "Give every package that depends on it the same version range and install again, so the page loads one copy."));
    out.push(engineFinding(project, copies[0]));
    if (project.bundler === "next") out.push(...engineCopyFindings(project, copies[0]));
  }
  const next = project.bundler === "next";
  out.push(...cspFindings(project, c.probe, c.probe !== undefined && (c.probe.engine.named || next)));
  if (c.probe) out.push(...servedFindings(c.probe, next && copies.length > 0 ? join(copies[0], "dist-lib", "app_bg.wasm") : undefined, next));
  const css = `${own.name}/gantt.css`;
  out.push(mentions(project.dir, css) ? pass("STYLESHEET_OK", `a source file imports ${css}`) : fail("STYLESHEET_MISSING", `no source file imports ${css}`, `Add \`import "${css}";\` to the module that mounts the chart, or the chart draws unstyled.`));
  if (c.data !== undefined) out.push(...dataFindings(c.data));
  out.push(gpuFinding(typeof c.gpu === "function" ? c.gpu() : c.gpu));
  return out;
}

/** The graphics path a browser gave the chart: WebGPU or WebGL2 with the adapter's name, or neither (the fault `mount` rejects with). With no browser answer, the check is skipped and says why: a Node process has no WebGPU or WebGL2 to ask. */
function gpuFinding(gpu: Gpu | undefined): Finding {
  const none = codeFor("GPU_NONE");
  if (gpu && "api" in gpu) {
    // doctor retried the browser without its sandbox, which does not start in some environments (AppArmor blocks user namespaces on Ubuntu).
    const note = gpu.noSandbox ? "; doctor retried the browser without the sandbox, because it would not start with it" : "";
    if (gpu.api === "webgpu") return pass("GPU_WEBGPU", `the browser has a WebGPU adapter (${gpu.adapter}), so the chart draws with WebGPU${note}`);
    if (gpu.api === "webgl2") return pass("GPU_WEBGL2", `the browser has no WebGPU adapter but has WebGL2 (${gpu.adapter}), so the chart draws with WebGL2${note}`);
    return fail("GPU_NONE", `the browser has neither WebGPU nor WebGL2, so \`mount\` rejects with ${none}${note}`, fixFor(none));
  }
  return { code: "GPU_SKIPPED", status: "skipped", what: `not checked: ${gpu ? gpu.unavailable : "no browser was asked"}; a Node process has no WebGPU or WebGL2 itself, and \`mount\` reports ${none} when the browser has neither`, fix: `Install Chrome, Edge or Chromium (or name one with ${BROWSER_VARIABLE}) and run doctor again, or open the page in a browser and read the error \`mount\` rejects with.` };
}

/** The findings as lines a person reads. */
export function describeFindings(findings: Finding[], where: string): string {
  const mark = { pass: "ok  ", fail: "FAIL", skipped: "skip" };
  const lines = [`doctor in ${where}`];
  for (const f of findings) lines.push(`  ${mark[f.status]} ${f.code}: ${f.what}`, ...(f.fix ? [`       fix: ${f.fix}`] : []));
  const n = findings.filter((f) => f.status === "fail").length;
  // A clean run with the GPU unchecked is not "all fine": the one check that says whether the chart can draw at all did not run.
  const unchecked = findings.some((f) => f.code === "GPU_SKIPPED");
  return [...lines, n === 0 ? `No problems found${unchecked ? ", GPU not checked" : ""}.` : `${n} problem${n === 1 ? "" : "s"} found.`, ""].join("\n");
}
