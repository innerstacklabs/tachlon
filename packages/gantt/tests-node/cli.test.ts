// The command-line tool (tl#1249 plan row 1): what `init --dry-run` detects in a project and plans, when it refuses, and its exit codes (0 done, 2 could not run).
// `run` takes the arguments and the working folder and returns the exit code and both streams, so no process is spawned.
import assert from "node:assert/strict";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import { after, test } from "node:test";
import type { Gpu } from "../src/cli/gpu.ts";
import { run, runAsync } from "../src/cli/run.ts";
import { skillFile } from "../src/cli/skill.ts";

const root = mkdtempSync(join(tmpdir(), "cli-"));
after(() => rmSync(root, { recursive: true, force: true }));
const NAME = ["tach", "lon"].join(""); // the package's own name, built at run time (the name check)
let n = 0;
// A project folder: `pkg` as its package.json (none: no file), and `files` as path -> text.
function project(pkg: object | null, files: Record<string, string | Buffer> = {}): string {
  const dir = join(root, `p${n++}`);
  mkdirSync(dir);
  if (pkg) writeFileSync(join(dir, "package.json"), JSON.stringify(pkg));
  for (const [f, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, f)), { recursive: true });
    writeFileSync(join(dir, f), text);
  }
  return dir;
}
const vite = { devDependencies: { vite: "8.0.0", react: "19.0.0" } };
const plan = (dir: string) => run(["init", "--dry-run", "--json", "--dir", dir], dir);
const refused = (dir: string, why: RegExp, search = false) => {
  const r = search ? run(["init", "--dry-run"], dir) : plan(dir);
  assert.equal(r.code, 2);
  assert.match(r.err, why);
  assert.equal(r.out, "");
};

test("a Vite and React project gets a plan with its manager, install command and one new file, and nothing is written", () => {
  const dir = project({ ...vite, devDependencies: { ...vite.devDependencies, typescript: "5.0.0" } }, { "pnpm-lock.yaml": "", "src/main.tsx": "" });
  const r = plan(dir);
  assert.equal(r.code, 0);
  const got = JSON.parse(r.out);
  assert.deepEqual([got.manager, got.framework, got.bundler, got.dryRun], ["pnpm", "react", "vite", true]);
  assert.match(got.install, new RegExp(`^pnpm add ${NAME}@\\d+\\.\\d+\\.\\d+`));
  assert.deepEqual(got.files, [`src/${NAME}-chart.tsx`]);
  assert.deepEqual(readdirSync(dir).sort(), ["package.json", "pnpm-lock.yaml", "src"]);
  assert.match(run(["init", "--dry-run", "--dir", dir], dir).out, /Nothing was written/);
});

test("the package manager comes from the lockfile, then the packageManager field, else npm", () => {
  const manager = (pkg: object, files: Record<string, string>) => JSON.parse(plan(project({ ...vite, ...pkg }, files)).out).manager;
  assert.equal(manager({ packageManager: "bun@1.2.0" }, { "yarn.lock": "" }), "yarn");
  assert.equal(manager({ packageManager: "bun@1.2.0" }, {}), "bun");
  assert.equal(manager({}, {}), "npm");
});

test("Next gets a client component and the engine copy; the router folder decides where, and none refuses", () => {
  const next = { dependencies: { next: "16.0.0", react: "19.0.0" } };
  assert.deepEqual(JSON.parse(plan(project(next, { "app/page.tsx": "" })).out).files, [`components/${NAME}-chart.jsx`, "public/app_bg.wasm"]);
  assert.deepEqual(JSON.parse(plan(project(next, { "src/pages/index.js": "", "tsconfig.json": "{}" })).out).files, [`src/components/${NAME}-chart.tsx`, "public/app_bg.wasm"]);
  refused(project(next), /neither an app\/.*create an app\/ or pages\/ folder, then run init again/);
});

test("the refusal that offers --plain says what the module needs, not that it needs no bundler", () => {
  const r = plan(project({ dependencies: { react: "19.0.0" } }));
  assert.equal(r.code, 2);
  assert.match(r.err, /found no bundler instead; pass --plain/);
  assert.doesNotMatch(r.err, /needs no bundler/);
});

test("it refuses, exit 2 and nothing on stdout, when it cannot tell what to write or would overwrite", () => {
  refused(project(null), /no package\.json/);
  refused(project({ workspaces: ["a"], ...vite }), /workspace root/, true);
  refused(project({ dependencies: { react: "19.0.0" } }), /found no bundler instead; pass --plain/);
  refused(project(vite, { [`src/${NAME}-chart.jsx`]: "x" }), new RegExp(`src/${NAME}-chart.jsx already exists.*move it aside or delete it, then run init again`));
  // A package.json that is not JSON, not an object, or not a file is a refusal in one sentence, never a stack and exit 1.
  refused(project(null, { "package.json": "{bad" }), /package\.json is not valid JSON/);
  refused(project(null, { "package.json": "null" }), /package\.json is not a JSON object/);
  refused(project(null, { "package.json/x": "" }), /package\.json cannot be read/);
  // A flag that takes a value does not take the next flag for it.
  const dir = project(vite);
  const r = run(["init", "--dry-run", "--dir", "--json"], dir);
  assert.deepEqual([r.code, r.out], [2, ""]);
  assert.match(r.err, /--dir needs a folder/);
});

test("the project is the nearest package.json up from the working folder; --dir names one and does not search", () => {
  const dir = project(vite, { "src/a/b.js": "" });
  assert.equal(JSON.parse(run(["init", "--dry-run", "--json"], join(dir, "src/a")).out).dir, dir);
  assert.equal(run(["init", "--dry-run", "--dir", join(dir, "src")], dir).code, 2);
});

test("usage: help and version exit 0; no command, an unknown flag or command, a flag missing its value, and a flag given a value it does not take exit 2", () => {
  assert.equal(run(["--help"], root).code, 0);
  assert.match(run(["--version"], root).out, /^\d+\.\d+\.\d+/);
  const dir = project(vite);
  for (const argv of [[], ["init", "--nope"], ["frob"], ["init", "--spec"], ["init", "--dry-run", "--skill=false"], ["init", "--dry-run=false"]]) assert.equal(run(argv, dir).code, 2, argv.join(" "));
  assert.deepEqual(readdirSync(dir), ["package.json"]);
});

// Row 2: `init` writes (tl#1249 plan steps 1 to 6). A fake package manager on PATH records what it was asked to install.
const fakeBin = join(root, "bin");
mkdirSync(fakeBin);
// Like npm, a successful install lists the package in package.json (the fake lists it under NAME whatever the spec).
writeFileSync(join(fakeBin, "npm"), `#!/usr/bin/env node\nconst fs = require("node:fs");\nfs.appendFileSync(${JSON.stringify(join(root, "npm.log"))}, process.argv.slice(2).join(" ") + "\\n");\nif (process.env.FAKE_NPM_FAIL) process.exit(1);\nconst pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));\nfs.writeFileSync("package.json", JSON.stringify({ ...pkg, dependencies: { ...pkg.dependencies, [${JSON.stringify(NAME)}]: "1.2.3" } }));\n`, { mode: 0o755 });
writeFileSync(join(fakeBin, "pnpm"), `#!/usr/bin/env node\nrequire("node:fs").appendFileSync(${JSON.stringify(join(root, "pnpm.log"))}, process.argv.slice(2).join(" ") + "\\n");\n`, { mode: 0o755 });
const installs = () => (existsSync(join(root, "npm.log")) ? readFileSync(join(root, "npm.log"), "utf8").trim().split("\n") : []);
function withFakeManager<T>(env: Record<string, string>, f: () => T): T {
  const path = process.env.PATH;
  Object.assign(process.env, env, { PATH: `${fakeBin}${delimiter}${process.env.PATH}` });
  try {
    return f();
  } finally {
    for (const k of Object.keys(env)) delete process.env[k];
    process.env.PATH = path;
  }
}
// Every file under `dir` with its text: two equal snapshots mean nothing changed.
const snapshot = (dir: string) => Object.fromEntries(readdirSync(dir, { recursive: true, withFileTypes: true }).filter((e) => e.isFile()).map((e) => [join(e.parentPath, e.name), readFileSync(join(e.parentPath, e.name), "utf8")]));
const write = (dir: string, ...flags: string[]) => run(["init", "--dir", dir, ...flags], dir);
const vitePlain = { devDependencies: { vite: "8.0.0", typescript: "5.0.0" } };
const viteReact = { devDependencies: { ...vite.devDependencies, typescript: "5.0.0" } };

test("init in a plain Vite project installs its own version, writes the chart module, names the line to add, and a second run changes nothing and says so", () => {
  const dir = project(vitePlain, { "src/main.ts": "" });
  const before = installs().length;
  const first = withFakeManager({}, () => write(dir));
  assert.equal(first.code, 0, first.err);
  assert.equal(installs().length, before + 1);
  assert.match(installs().at(-1) as string, new RegExp(`^install ${NAME}@\\d+\\.\\d+\\.\\d+`));
  const file = join(dir, `src/${NAME}-chart.ts`);
  const text = readFileSync(file, "utf8");
  assert.match(text, new RegExp(`from "${NAME}"`));
  assert.match(text, new RegExp(`"${NAME}/gantt.css"`));
  assert.equal(text.match(/\bstart: "\d{4}-\d\d-\d\d"/g)?.length, 3);
  assert.match(text, /\.destroy\(\)/);
  assert.match(first.out, new RegExp(`wrote: src/${NAME}-chart.ts`));
  assert.match(first.out, new RegExp(`from "\\./${NAME}-chart"`));
  assert.match(first.out, /npm run dev/);
  const done = snapshot(dir);
  const second = withFakeManager({}, () => write(dir));
  assert.equal(second.code, 0, second.err);
  assert.match(second.out, /already set up, nothing changed/);
  assert.deepEqual(snapshot(dir), done);
  assert.equal(installs().length, before + 1, "the second run installs nothing");
});

test("init in a Vite and React project writes a component that destroys the chart on unmount: tsx with TypeScript, jsx without; --no-install installs nothing", () => {
  const before = installs().length;
  for (const [pkg, ext] of [[viteReact, "tsx"], [vite, "jsx"]] as const) {
    const dir = project(pkg, { "src/main.js": "" });
    const first = write(dir, "--no-install");
    assert.equal(first.code, 0, first.err);
    const text = readFileSync(join(dir, `src/${NAME}-chart.${ext}`), "utf8");
    assert.match(text, /useEffect/);
    assert.match(text, /\.destroy\(\)/);
    assert.match(text, /className="gantt"/);
    assert.equal(ext === "tsx", /useRef<HTMLDivElement>/.test(text));
    assert.match(text, /function Chart\(\{ onReady \}/, "the component takes onReady (gantt#1522)");
    assert.match(text, /onReady\?\.\(made\)/, "and calls it with the chart `mount` resolved to");
    assert.equal(ext === "tsx", /onReady\?: \(chart: Gantt\) => void/.test(text));
    const done = snapshot(dir);
    assert.match(write(dir, "--no-install").out, /already set up, nothing changed/);
    assert.deepEqual(snapshot(dir), done);
  }
  assert.equal(installs().length, before);
});

const SKILL = `.claude/skills/${NAME}/SKILL.md`;

test("init --skill is listed by --dry-run and --json, writes the skill once, and plain init writes none", () => {
  const dir = project(vitePlain, { "src/main.ts": "" });
  const before = snapshot(dir);
  const planned = JSON.parse(run(["init", "--dry-run", "--json", "--skill", "--dir", dir], dir).out);
  assert.deepEqual(planned.files, [`src/${NAME}-chart.ts`, SKILL]);
  assert.match(run(["init", "--dry-run", "--skill", "--dir", dir], dir).out, new RegExp(`would write: ${SKILL.replace(/\./g, "\\.")}`));
  assert.deepEqual(snapshot(dir), before, "a dry run writes nothing");
  assert.deepEqual(JSON.parse(plan(dir).out).files, [`src/${NAME}-chart.ts`], "plain init does not name the skill");
  const first = withFakeManager({}, () => write(dir, "--skill", "--json"));
  assert.equal(first.code, 0, first.err);
  assert.deepEqual(JSON.parse(first.out).files, planned.files);
  const text = readFileSync(join(dir, SKILL), "utf8");
  assert.match(text, new RegExp(`^---\\nname: ${NAME}\\n`));
  assert.match(text, new RegExp(`node_modules/${NAME}/AGENTS\\.md`));
  for (const never of [/replace or delete a page/, /lockfile/, /CSP_WASM_EVAL/, /CSP_CONNECT_SRC/, /push, open a pull request/, /globally/, /dev server/]) assert.match(text, never);
  const done = snapshot(dir);
  const second = withFakeManager({}, () => write(dir, "--skill"));
  assert.match(second.out, /already set up, nothing changed/);
  assert.deepEqual(snapshot(dir), done);
  const bare = project(vitePlain, { "src/main.ts": "" });
  assert.equal(withFakeManager({}, () => write(bare)).code, 0);
  assert.equal(existsSync(join(bare, ".claude")), false, "init without --skill writes no skill");
});

test("the skill and AGENTS.md say `describe` exists and when to use it", () => {
  const guide = readFileSync(new URL("../AGENTS.md", import.meta.url), "utf8");
  for (const [where, text] of [["the skill", skillFile(NAME)], ["AGENTS.md", guide]] as const) {
    for (const phrase of ["before you write code against an option, event, command or token", `npx ${NAME} describe`, `npx ${NAME} describe fit`, `npx ${NAME} describe methods`, `npx ${NAME} describe refusals`, "before you theme the chart or wire saves"]) assert.ok(text.toLowerCase().includes(phrase.toLowerCase()), `${where} says: ${phrase}`);
  }
});

test("the skill says when to run `doctor` and names AGENTS.md", () => {
  const text = skillFile(NAME);
  for (const phrase of ["does not draw or does not load", `npx ${NAME} doctor`, `node_modules/${NAME}/AGENTS.md`]) assert.ok(text.includes(phrase), `the skill says: ${phrase}`);
});

test("init --skill never overwrites a skill that is there: it stays as it is and the chart module is still written", () => {
  const dir = project(vitePlain, { "src/main.ts": "", [SKILL]: "my own skill\n" });
  const r = withFakeManager({}, () => write(dir, "--skill"));
  assert.equal(r.code, 0, r.err);
  assert.equal(readFileSync(join(dir, SKILL), "utf8"), "my own skill\n");
  assert.ok(existsSync(join(dir, `src/${NAME}-chart.ts`)));
  assert.doesNotMatch(r.out, /wrote: \.claude/);
  const done = snapshot(dir);
  assert.match(withFakeManager({}, () => write(dir, "--skill")).out, /already set up, nothing changed/);
  assert.deepEqual(snapshot(dir), done);
});

test("--spec replaces the installed spec, the lockfile picks the manager, and a failed install writes nothing and exits 2", () => {
  const dir = project(vitePlain, { "package-lock.json": "" });
  const before = snapshot(dir);
  const failed = withFakeManager({ FAKE_NPM_FAIL: "1" }, () => write(dir, "--spec", "file:/x/pack.tgz"));
  assert.equal(failed.code, 2);
  assert.match(failed.err, /install file:\/x\/pack\.tgz failed.*init wrote no files; fix the install error below, then run init again/);
  assert.deepEqual(snapshot(dir), before);
  assert.equal(installs().at(-1), "install file:/x/pack.tgz");
  assert.equal(withFakeManager({}, () => write(dir, "--spec=file:/y.tgz")).code, 0);
  assert.equal(installs().at(-1), "install file:/y.tgz");
});

test("init refuses with exit 2 and writes nothing: no package.json, a workspace root without --dir, an unknown bundler (named, --plain offered), a file it would write", () => {
  const refusal = (dir: string, why: RegExp, argv: string[] = ["--dir", dir]) => {
    const before = snapshot(dir);
    const r = run(["init", "--no-install", ...argv], dir);
    assert.equal(r.code, 2, r.out);
    assert.match(r.err, why);
    assert.equal(r.out, "");
    assert.deepEqual(snapshot(dir), before);
  };
  refusal(project(null), /no package\.json/);
  refusal(project({ workspaces: ["a"], ...vitePlain }), /workspace root/, []);
  refusal(project({ devDependencies: { webpack: "5.0.0" } }), /webpack.*--plain/);
  refusal(project(vitePlain, { [`src/${NAME}-chart.ts`]: "mine" }), new RegExp(`src/${NAME}-chart.ts already exists`));
  // A write that fails names the step that works for its cause: a file where the folder must go is moved aside; making a folder writable would not help.
  refusal(project(vitePlain, { src: "mine" }), /cannot be written.*move aside or delete the file the error names, then run init again/);
  // A folder that cannot be written (not as root, which writes anywhere) is made writable.
  if (process.getuid?.() !== 0) {
    const locked = project(vitePlain, { "src/main.ts": "" });
    chmodSync(join(locked, "src"), 0o555);
    try {
      refusal(locked, /cannot be written.*EACCES.*make the folder writable, then run init again/);
    } finally {
      chmodSync(join(locked, "src"), 0o755);
    }
  }
});

// Row 4: `init` in a Next app writes the component and copies the engine to public/ (tl#1249 plan, `init`, Next). The package is already in node_modules, so `--no-install` has an engine to copy.
const WASM_NEXT = Buffer.concat([Buffer.from([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]), Buffer.alloc(64, 7)]);
const next = { dependencies: { next: "16.0.0", react: "19.0.0" }, devDependencies: { typescript: "5.0.0" } };
const withEngine = (pkg: object, files: Record<string, string | Buffer>): string =>
  project(pkg, { "package-lock.json": "{}", [`node_modules/${NAME}/package.json`]: JSON.stringify({ name: NAME, version: "1.2.3" }), [`node_modules/${NAME}/dist-lib/app_bg.wasm`]: WASM_NEXT, [`node_modules/${NAME}/dist-lib/engine-size.json`]: JSON.stringify({ bytes: WASM_NEXT.length }), ...files });
const bytesOf = (dir: string) => Object.fromEntries(readdirSync(dir, { recursive: true, withFileTypes: true }).filter((e) => e.isFile()).map((e) => [join(e.parentPath, e.name), readFileSync(join(e.parentPath, e.name)).toString("hex")]));

test("init in a Next app writes a client component that points at /app_bg.wasm and copies the installed engine to public/; a second run changes nothing", () => {
  const dir = withEngine(next, { "app/page.tsx": "" });
  const first = write(dir, "--no-install");
  assert.equal(first.code, 0, first.err);
  const text = readFileSync(join(dir, `components/${NAME}-chart.tsx`), "utf8");
  assert.match(text, /^(\/\/[^\n]*\n)*"use client";/);
  assert.match(text, /wasmUrl: "\/app_bg\.wasm"/);
  assert.match(text, /\.destroy\(\)/);
  // A mount that rejects (a wasm served as text/html, say) is logged with its fix, never an unhandled rejection.
  assert.match(text, /\.catch\(\(error\) => console\.error\(error\.message, error\.fix\)\)/);
  assert.deepEqual(readFileSync(join(dir, "public/app_bg.wasm")), WASM_NEXT);
  assert.match(first.out, new RegExp(`wrote: components/${NAME}-chart.tsx`));
  assert.match(first.out, new RegExp(`from "\\.\\./components/${NAME}-chart"`));
  // Nothing else is touched: the lockfile and the dependency list stay as they were, and no tsconfig or .env appears.
  assert.equal(readFileSync(join(dir, "package-lock.json"), "utf8"), "{}");
  assert.equal(existsSync(join(dir, "tsconfig.json")) || existsSync(join(dir, ".env")), false);
  const done = bytesOf(dir);
  const second = write(dir, "--no-install");
  assert.equal(second.code, 0, second.err);
  assert.match(second.out, /already set up, nothing changed/);
  assert.deepEqual(bytesOf(dir), done);
});

test("init in a Next app with src/pages and no TypeScript writes src/components/<name>-chart.jsx, and the install runs once", () => {
  const dir = withEngine({ dependencies: next.dependencies }, { "src/pages/index.js": "" });
  const before = installs().length;
  const r = withFakeManager({}, () => write(dir));
  assert.equal(r.code, 0, r.err);
  assert.equal(installs().length, before + 1);
  assert.match(readFileSync(join(dir, `src/components/${NAME}-chart.jsx`), "utf8"), /"use client"/);
  assert.deepEqual(readFileSync(join(dir, "public/app_bg.wasm")), WASM_NEXT);
});

test("init in a Next app never overwrites: a different component refuses, a copy already in public/ stays as it is, and only a missing file is written", () => {
  const mine = withEngine(next, { "app/page.tsx": "", [`components/${NAME}-chart.tsx`]: "mine" });
  const before = bytesOf(mine);
  const refused = write(mine, "--no-install");
  assert.equal(refused.code, 2);
  assert.match(refused.err, /already exists, and init never overwrites/);
  assert.deepEqual(bytesOf(mine), before);
  const old = withEngine(next, { "app/page.tsx": "", "public/app_bg.wasm": Buffer.from([0, 0x61, 0x73, 0x6d, 9]) });
  assert.equal(write(old, "--no-install").code, 0);
  assert.deepEqual(readFileSync(join(old, "public/app_bg.wasm")), Buffer.from([0, 0x61, 0x73, 0x6d, 9]));
  assert.equal(existsSync(join(old, `components/${NAME}-chart.tsx`)), true);
  const again = bytesOf(old);
  assert.match(write(old, "--no-install").out, /already set up, nothing changed/);
  assert.deepEqual(bytesOf(old), again);
});

test("init in a Next app refuses with exit 2 and writes nothing: no app/ or pages/ folder, or no installed engine to copy", () => {
  const nothing = project(next, { "package-lock.json": "{}" });
  const r = write(nothing, "--no-install");
  assert.deepEqual([r.code, r.out], [2, ""]);
  assert.match(r.err, /neither an app\//);
  assert.deepEqual(Object.keys(snapshot(nothing)).map((f) => f.slice(nothing.length)), ["/package-lock.json", "/package.json"]);
  const bare = project(next, { "app/page.tsx": "" });
  const before = snapshot(bare);
  const none = write(bare, "--no-install");
  assert.deepEqual([none.code, none.out], [2, ""]);
  assert.match(none.err, /dist-lib\/app_bg\.wasm.*no file was written/);
  assert.match(none.err, /install the package in this project, then run init again/);
  assert.deepEqual(snapshot(bare), before);
  // After a manager run that left no engine, the sentence says the package was installed, as the write failure does.
  const after = withFakeManager({}, () => write(bare));
  assert.equal(after.code, 2);
  assert.match(after.err, /the package was installed and no file was written; install a version that ships dist-lib\/app_bg\.wasm, then run init again/);
  assert.doesNotMatch(after.err, /install the package in this project/); // the install already ran, so asking for it again is a step that cannot work
  assert.deepEqual({ ...snapshot(bare), [join(bare, "package.json")]: "" }, { ...before, [join(bare, "package.json")]: "" }); // the fake manager lists the package in package.json, so every other file is compared
});

test("init after init --no-install installs the package instead of saying it is already set up", () => {
  const dir = project(vitePlain, { "src/main.ts": "" });
  const before = installs().length;
  assert.equal(write(dir, "--no-install").code, 0);
  const second = withFakeManager({}, () => write(dir));
  assert.equal(second.code, 0, second.err);
  assert.match(second.out, new RegExp(`ran: npm install ${NAME}@`));
  assert.doesNotMatch(second.out, /already set up/);
  assert.equal(installs().length, before + 1);
  assert.match(withFakeManager({}, () => write(dir)).out, /already set up, nothing changed/);
  assert.equal(installs().length, before + 1, "the third run installs nothing");
});

test("--plain writes the framework-free module, for an unknown bundler or a React project alike", () => {
  const other = project({ devDependencies: { webpack: "5.0.0" } });
  assert.equal(write(other, "--no-install", "--plain").code, 0);
  assert.match(readFileSync(join(other, `${NAME}-chart.js`), "utf8"), /export async function mountChart/);
  assert.match(readFileSync(join(other, `${NAME}-chart.js`), "utf8"), /bundler or an import map/);
  const react = project(viteReact, { "src/main.tsx": "" });
  assert.equal(write(react, "--no-install", "--plain").code, 0);
  assert.deepEqual(readdirSync(join(react, "src")).sort(), ["main.tsx", `${NAME}-chart.ts`]);
});

test("a --dir member of a workspace has no lockfile of its own: the nearest workspace root's lockfile or packageManager picks the manager, and pnpm runs", () => {
  const ws = project({ private: true }, { "pnpm-lock.yaml": "", "pnpm-workspace.yaml": "packages:\n  - 'apps/*'\n", "apps/web/package.json": JSON.stringify(vitePlain) });
  const member = join(ws, "apps/web");
  const planned = JSON.parse(plan(member).out);
  assert.equal(planned.manager, "pnpm");
  assert.equal(withFakeManager({}, () => write(member)).code, 0);
  assert.equal(readFileSync(join(root, "pnpm.log"), "utf8").trim(), planned.install.replace(/^pnpm /, ""));
  assert.deepEqual(readdirSync(member).sort(), ["package.json", `${NAME}-chart.ts`]);
  const yarn = project({ private: true, workspaces: ["pkgs/*"], packageManager: "yarn@4.1.0" }, { "pkgs/a/package.json": JSON.stringify(vitePlain) });
  assert.equal(JSON.parse(plan(join(yarn, "pkgs/a")).out).manager, "yarn");
  assert.equal(JSON.parse(plan(project(vitePlain)).out).manager, "npm");
});

test("a write that fails after the install exits 2 with one sentence saying the package was installed and no file was written", () => {
  const dir = project(vitePlain, { "pnpm-lock.yaml": "", src: "not a folder" });
  const r = withFakeManager({}, () => write(dir));
  assert.equal(r.code, 2);
  assert.match(r.err, /the package was installed and no file was written/);
  assert.equal(r.err.trim().split("\n").length, 1);
});

// Row 3: `doctor` reads an installed project and names each fault with one fix sentence (tl#1249 plan, `doctor`). The engine's size is the one the package's build recorded in `dist-lib/engine-size.json`.
const WASM = Buffer.concat([Buffer.from([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]), Buffer.alloc(64, 7)]);
// An installed project: the package listed and in node_modules with its engine and the size recorded for it (`record`: the file's text, null for none), and a source file that imports the stylesheet.
function installed(o: { engine?: Buffer | null; css?: boolean; record?: string | null; files?: Record<string, string> } = {}): string {
  const files: Record<string, string | Buffer> = { [`node_modules/${NAME}/package.json`]: JSON.stringify({ name: NAME, version: "1.2.3" }), "src/main.ts": o.css === false ? "" : `import "${NAME}/gantt.css";\n`, ...o.files };
  if (o.engine !== null) files[`node_modules/${NAME}/dist-lib/app_bg.wasm`] = o.engine ?? WASM;
  if (o.record !== null) files[`node_modules/${NAME}/dist-lib/engine-size.json`] = o.record ?? JSON.stringify({ bytes: WASM.length });
  return project({ devDependencies: { ...vite.devDependencies, [NAME]: "1.2.3" } }, files);
}
const doctor = (dir: string, ...flags: string[]) => run(["doctor", "--dir", dir, "--json", ...flags], dir);
type Finding = { code: string; status: string; what: string; fix: string };
const found = (r: ReturnType<typeof run>) => (JSON.parse(r.out).findings as Finding[]).filter((f) => f.status === "fail");
// The fail findings of a run that exits 1, each with a one-sentence fix.
function problems(dir: string, ...flags: string[]): Finding[] {
  const r = doctor(dir, ...flags);
  assert.equal(r.code, 1, r.out + r.err);
  const fails = found(r);
  for (const f of fails) assert.match(f.fix, /^[^\n]+\.$/, f.code);
  return fails;
}

test("doctor exits 0 on a good install, lists each check, skips the GPU one and says mount reports GPU_NONE, and changes nothing", () => {
  const dir = installed();
  const before = snapshot(dir);
  const r = doctor(dir);
  assert.equal(r.code, 0, r.out + r.err);
  const all = JSON.parse(r.out).findings as Finding[];
  assert.deepEqual(all.map((f) => f.code), ["INSTALLED", "ONE_COPY", "ENGINE_OK", "STYLESHEET_OK", "GPU_SKIPPED"]);
  assert.deepEqual(all.map((f) => f.status), ["pass", "pass", "pass", "pass", "skipped"]);
  assert.match(all[0].what, /1\.2\.3/);
  assert.match(all[4].what, new RegExp(`${NAME.toUpperCase()}_GPU_NONE`));
  assert.deepEqual(snapshot(dir), before);
  assert.match(run(["doctor", "--dir", dir], dir).out, /ok .*ENGINE_OK/);
});

test("doctor exits 1 with the code of each engine fault: deleted, cut short, not WebAssembly", () => {
  assert.deepEqual(problems(installed({ engine: null })).map((f) => f.code), ["ENGINE_MISSING"]);
  const cut = problems(installed({ engine: WASM.subarray(0, 30) }));
  assert.deepEqual(cut.map((f) => f.code), ["ENGINE_SIZE"]);
  assert.match(cut[0].what, new RegExp(`30 bytes.*recorded.*${WASM.length}`));
  assert.deepEqual(problems(installed({ engine: Buffer.alloc(WASM.length, "<") })).map((f) => f.code), ["ENGINE_NOT_WASM"]);
});

test("doctor exits 1 when no source file imports the stylesheet, and ignores a stylesheet import that sits in node_modules", () => {
  const dir = installed({ css: false });
  writeFileSync(join(dir, "node_modules", NAME, "x.js"), `import "${NAME}/gantt.css";`);
  assert.deepEqual(problems(dir).map((f) => f.code), ["STYLESHEET_MISSING"]);
});

// A page with no bundler: its module script imports the package by name, which a browser resolves only through an import map.
function bare(map = "", bundler = false): string {
  const dir = installed({ files: { "index.html": `<div class="gantt"></div>${map}<script type="module" src="/src/chart.js"></script>`, "src/chart.js": `import { mount } from "${NAME}";\n` } });
  if (!bundler) writeFileSync(join(dir, "package.json"), JSON.stringify({ dependencies: { [NAME]: "1.2.3" } }));
  return dir;
}

test("doctor exits 1 for a no-bundler page whose module imports the package by name and has no import map, and passes with an import map or a bundler", () => {
  const [f, ...rest] = problems(bare());
  assert.deepEqual([f.code, rest.length], ["MODULE_NOT_RESOLVABLE", 0]);
  assert.match(f.what, new RegExp(`"${NAME}".*import map`));
  assert.equal(doctor(bare(`<script type="importmap">{"imports":{"${NAME}":"/x.js"}}</script>`)).code, 0);
  assert.equal(doctor(bare("", true)).code, 0);
});

test("doctor exits 1 when the stylesheet is only named in a comment, and accepts each import form", () => {
  assert.deepEqual(problems(installed({ css: false, files: { "src/note.ts": `// see ${NAME}/gantt.css\n` } })).map((f) => f.code), ["STYLESHEET_MISSING"]);
  for (const line of [`@import url(${NAME}/gantt.css);`, `@import "~${NAME}/gantt.css";`, `@import "${NAME}/gantt.css";`,`@import url("${NAME}/gantt.css");`, `require("${NAME}/gantt.css");`, `import("${NAME}/gantt.css");`, `import "./a.css"; import '${NAME}/gantt.css'`])
    assert.equal(doctor(installed({ css: false, files: { "src/a.ts": line } })).code, 0, line);
});

test("doctor exits 1 for a package it does not find listed, or not installed, or installed twice", () => {
  assert.deepEqual(problems(project(vite, { "src/a.ts": "" })).map((f) => f.code), ["NOT_LISTED"]);
  assert.ok(problems(project(vite, { "src/a.ts": "" }))[0].fix.startsWith(`Run \`npx ${NAME} init\` here`));
  const bare = project({ devDependencies: { [NAME]: "1.2.3" } }, { "src/a.ts": `import "${NAME}/gantt.css";` });
  assert.deepEqual(problems(bare).map((f) => f.code), ["NOT_INSTALLED"]);
  const dir = installed();
  mkdirSync(join(dir, "node_modules/other/node_modules", NAME), { recursive: true });
  writeFileSync(join(dir, "node_modules/other/node_modules", NAME, "package.json"), JSON.stringify({ name: NAME, version: "1.0.0" }));
  const two = problems(dir);
  assert.deepEqual(two.map((f) => f.code), ["MANY_COPIES"]);
  assert.match(two[0].what, /1\.0\.0.*1\.2\.3|1\.2\.3.*1\.0\.0/);
});

test("doctor's engine fix names `<manager> install`, which restores the package for pnpm as for npm", () => {
  assert.match(problems(installed({ engine: null, files: { "pnpm-lock.yaml": "" } }))[0].fix, /with `pnpm install`/);
});

test("doctor counts a pnpm layout's symlink and its store entry as one copy, and a second version in the store as two", () => {
  const dir = project({ devDependencies: { ...vite.devDependencies, [NAME]: "1.2.3" } }, { "src/main.ts": `import "${NAME}/gantt.css";`, [`node_modules/.pnpm/${NAME}@1.2.3/node_modules/${NAME}/package.json`]: JSON.stringify({ version: "1.2.3" }), [`node_modules/.pnpm/${NAME}@1.2.3/node_modules/${NAME}/dist-lib/app_bg.wasm`]: WASM, [`node_modules/.pnpm/${NAME}@1.2.3/node_modules/${NAME}/dist-lib/engine-size.json`]: JSON.stringify({ bytes: WASM.length }) });
  symlinkSync(join(dir, `node_modules/.pnpm/${NAME}@1.2.3/node_modules/${NAME}`), join(dir, "node_modules", NAME));
  assert.equal(doctor(dir).code, 0);
  mkdirSync(join(dir, `node_modules/.pnpm/${NAME}@1.0.0/node_modules/${NAME}`), { recursive: true });
  writeFileSync(join(dir, `node_modules/.pnpm/${NAME}@1.0.0/node_modules/${NAME}/package.json`), JSON.stringify({ version: "1.0.0" }));
  assert.deepEqual(problems(dir).map((f) => f.code), ["MANY_COPIES"]);
});

test("doctor --data prints every issue validate() finds, one finding each; a good file passes, a file that is not JSON fails, an unreadable one exits 2", () => {
  const dir = installed();
  const data = join(dir, "data.json");
  writeFileSync(data, JSON.stringify({ tasks: [{ id: 1, name: "A", start: "soon" }, { id: 1, name: 2, start: "2026-03-02" }] }));
  const bad = problems(dir, "--data", data);
  assert.ok(bad.length >= 3, bad.map((f) => f.what).join("\n"));
  assert.ok(bad.every((f) => f.code === "DATA_INVALID"));
  assert.match(bad.map((f) => f.what).join("\n"), /tasks\[0\]\.start/);
  writeFileSync(data, JSON.stringify({ tasks: [{ id: 1, name: "A", start: "2026-03-02" }] }));
  assert.equal(doctor(dir, "--data", data).code, 0);
  writeFileSync(data, "{bad");
  assert.deepEqual(problems(dir, "--data", data).map((f) => f.code), ["DATA_NOT_JSON"]);
  const missing = doctor(dir, "--data", join(dir, "none.json"));
  assert.deepEqual([missing.code, missing.out], [2, ""]);
  assert.match(missing.err, /none\.json cannot be read/);
  assert.equal(run(["doctor", "--data"], dir).code, 2);
});

test("doctor exits 2 when it cannot run: no package.json, or a workspace root without --dir", () => {
  assert.equal(run(["doctor", "--dir", project(null)], root).code, 2);
  const ws = run(["doctor"], project({ workspaces: ["a"], ...vite }));
  assert.equal(ws.code, 2);
  assert.match(ws.err, /workspace root/);
});

// The size is the copy's own record, so the check holds when the command that runs is that installed copy (`pnpm exec <bin> doctor`): the reference never comes from a file the same install supplies.
test("doctor never says ENGINE_OK for a copy whose recorded size is missing, unreadable or not a number", () => {
  for (const record of [null, "not json", JSON.stringify({ bytes: "64" })]) {
    const r = problems(installed({ record }));
    assert.deepEqual(r.map((f) => f.code), ["ENGINE_UNRECORDED"], String(record));
  }
});

// A file or folder doctor must read but cannot is "could not run" (exit 2, one sentence, no stack trace), never exit 1, which means findings.
test("doctor exits 2, not 1, when a folder or the engine cannot be read", { skip: process.getuid?.() === 0 && "root reads every file" }, () => {
  const unreadable = (dir: string, target: string, mode: number) => {
    chmodSync(target, 0);
    try {
      return run(["doctor", "--dir", dir], dir);
    } finally {
      chmodSync(target, mode);
    }
  };
  const folder = installed();
  mkdirSync(join(folder, "pgdata"));
  const a = unreadable(folder, join(folder, "pgdata"), 0o755);
  assert.deepEqual([a.code, a.out, /pgdata.*EACCES/.test(a.err), a.err.trim().split("\n").length], [2, "", true, 1]);
  const engine = installed();
  const b = unreadable(engine, join(engine, "node_modules", NAME, "dist-lib", "app_bg.wasm"), 0o644);
  assert.deepEqual([b.code, b.out, /app_bg\.wasm.*EACCES/.test(b.err)], [2, "", true]);
});

// Row 4: `doctor` in a Next app also compares the engine copy in public/ with the size the installed package recorded, so a copy left from an older version is named.
const nextInstalled = (copy?: Buffer | null) => withEngine({ ...next, dependencies: { ...next.dependencies, [NAME]: "1.2.3" } }, { "app/page.tsx": `import "${NAME}/gantt.css";\n`, ...(copy === null ? {} : { "public/app_bg.wasm": copy ?? WASM_NEXT }) });

test("doctor in a Next app passes the engine copy in public/ when it is the recorded size, and names a missing, stale or non-engine copy", () => {
  const ok = doctor(nextInstalled());
  assert.equal(ok.code, 0, ok.out + ok.err);
  assert.deepEqual((JSON.parse(ok.out).findings as Finding[]).map((f) => f.code), ["INSTALLED", "ONE_COPY", "ENGINE_OK", "ENGINE_COPY_OK", "STYLESHEET_OK", "GPU_SKIPPED"]);
  assert.deepEqual(problems(nextInstalled(null)).map((f) => f.code), ["ENGINE_COPY_MISSING"]);
  const stale = problems(nextInstalled(WASM_NEXT.subarray(0, 30)));
  assert.deepEqual(stale.map((f) => f.code), ["ENGINE_COPY_STALE"]);
  assert.match(stale[0].what, new RegExp(`public/app_bg\\.wasm is 30 bytes.*${WASM_NEXT.length} bytes`));
  assert.match(stale[0].fix, /cp .*dist-lib\/app_bg\.wasm public\/app_bg\.wasm/);
  assert.deepEqual(problems(nextInstalled(Buffer.alloc(WASM_NEXT.length, "<"))).map((f) => f.code), ["ENGINE_COPY_STALE"]);
  // The same size and the same first bytes, one byte of the engine different: another build, not the installed one.
  const other = Buffer.from(WASM_NEXT);
  other[other.length - 1] ^= 1;
  const same = problems(nextInstalled(other));
  assert.deepEqual(same.map((f) => f.code), ["ENGINE_COPY_STALE"]);
  assert.match(same[0].what, /its content differs/);
});

test("doctor exits 2, not 1, when the engine copy in public/ cannot be read", { skip: process.getuid?.() === 0 && "root reads every file" }, () => {
  const dir = nextInstalled();
  const copy = join(dir, "public/app_bg.wasm");
  chmodSync(copy, 0);
  try {
    const r = run(["doctor", "--dir", dir], dir);
    assert.deepEqual([r.code, r.out, /public\/app_bg\.wasm.*EACCES/.test(r.err)], [2, "", true]);
  } finally {
    chmodSync(copy, 0o644);
  }
});

test("doctor in a Vite project does not look for a copy in public/", () => {
  assert.equal((JSON.parse(doctor(installed()).out).findings as Finding[]).some((f) => f.code.startsWith("ENGINE_COPY")), false);
});

// Row 5: `doctor` also reads the page's Content-Security-Policy (the project's config, and with `--url` the served page) and, with `--url`, the engine address (a local server plays the faults).
type Reply = { status?: number; type?: string; csp?: string; body: string | Buffer };
const servers: Server[] = [];
after(() => servers.forEach((s) => (s.closeAllConnections(), s.close())));
async function serve(routes: Record<string, Reply>): Promise<string> {
  const server = createServer((req, res) => {
    const r = routes[req.url ?? ""] ?? { status: 404, body: "not found" };
    res.writeHead(r.status ?? 200, { "content-type": r.type ?? "text/html", connection: "close", ...(r.csp ? { "content-security-policy": r.csp } : {}) });
    res.end(r.body);
  });
  servers.push(server);
  await new Promise<void>((ok) => server.listen(0, "127.0.0.1", ok));
  return `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
}
const GOOD: Reply = { type: "application/wasm", body: WASM };
const WASM_OK = "script-src 'self' 'wasm-unsafe-eval'";
// A page at `/` that names the engine at /app_bg.wasm (a preload link) and serves `engine` there, or nothing. `BARE` is a page that names none: only a Next app has an address for it.
const BARE = `<div class="gantt"></div>`;
const site = (page: Partial<Reply>, engine: Reply | null = GOOD) => serve({ "/": { body: `<link rel="preload" href="/app_bg.wasm" as="fetch">${BARE}`, ...page }, ...(engine ? { "/app_bg.wasm": engine } : {}) });
// No browser is asked here: the GPU line is the skip.
const NO_BROWSER = async (): Promise<Gpu> => ({ unavailable: "no browser in this test" });
const at = (dir: string, url: string) => runAsync(["doctor", "--dir", dir, "--json", "--url", url], dir, NO_BROWSER);
const codes = (r: Awaited<ReturnType<typeof at>>) => (JSON.parse(r.out).findings as Finding[]).map((f) => f.code);
// The fail codes of a run that exits 1, each with a one-sentence fix.
async function faults(dir: string, url: string): Promise<string[]> {
  const r = await at(dir, url);
  assert.equal(r.code, 1, r.out + r.err);
  for (const f of found(r)) assert.match(f.fix, /^[^\n]+\.$/, f.code);
  return found(r).map((f) => f.code);
}

test("doctor --url passes a page whose engine address answers with the engine and whose policy allows it", async () => {
  const r = await at(installed(), await site({ csp: `default-src 'self'; ${WASM_OK}` }));
  assert.equal(r.code, 0, r.out + r.err);
  assert.deepEqual(codes(r), ["INSTALLED", "ONE_COPY", "ENGINE_OK", "CSP_OK", "ENGINE_URL_OK", "STYLESHEET_OK", "GPU_SKIPPED"]);
  assert.deepEqual(codes(await at(installed(), await site({}))), ["INSTALLED", "ONE_COPY", "ENGINE_OK", "ENGINE_URL_OK", "STYLESHEET_OK", "GPU_SKIPPED"]);
});

test("doctor --url exits 1 with CSP_WASM_EVAL when the response header's script-src, or default-src without one, lacks 'wasm-unsafe-eval'", async () => {
  for (const csp of ["script-src 'self'", "default-src 'self'", "default-src 'self'; script-src 'self' https://cdn.example", `${WASM_OK}, default-src 'self'`]) {
    assert.deepEqual(await faults(installed(), await site({ csp })), ["CSP_WASM_EVAL"], csp);
  }
  for (const csp of ["default-src 'self' 'wasm-unsafe-eval'", "script-src 'self' 'unsafe-eval'", "img-src 'self'", "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'"]) {
    assert.equal((await at(installed(), await site({ csp }))).code, 0, csp);
  }
});

test("doctor --url exits 1 with CSP_CONNECT_SRC when the page names an engine on another origin that connect-src (or default-src without one) does not allow", async () => {
  const cdn = await site({}, GOOD);
  const page = (csp: string) => site({ csp, body: `<link rel="preload" href="${cdn}/app_bg.wasm" as="fetch">` });
  for (const csp of [`${WASM_OK}; connect-src 'self'`, `default-src 'self' 'wasm-unsafe-eval'`, `${WASM_OK}; connect-src https://other.example`, `${WASM_OK}; connect-src http://127.0.0.1:1`]) {
    assert.deepEqual(await faults(installed(), await page(csp)), ["CSP_CONNECT_SRC"], csp);
  }
  for (const csp of [`${WASM_OK}; connect-src ${cdn}`, `${WASM_OK}; connect-src *`, `${WASM_OK}; connect-src 'self' http://*.0.0.1:${new URL(cdn).port}`, WASM_OK, `${WASM_OK}; connect-src http:`]) {
    assert.equal((await at(installed(), await page(csp))).code, 0, csp);
  }
});

test("doctor --url names both CSP_WASM_EVAL and CSP_CONNECT_SRC in one run when one policy has both faults", async () => {
  const cdn = await site({}, GOOD);
  const page = site({ csp: "script-src 'self'; connect-src 'self'", body: `<link rel="preload" href="${cdn}/app_bg.wasm" as="fetch">` });
  assert.deepEqual(await faults(installed(), await page), ["CSP_WASM_EVAL", "CSP_CONNECT_SRC"]);
});

test("doctor --url checks connect-src for an engine on the page's own origin too: 'self' allows it, a list without 'self' does not", async () => {
  for (const csp of [`${WASM_OK}; connect-src https://other.example`, `${WASM_OK}; connect-src 'none'`, `default-src https://other.example; ${WASM_OK}`]) {
    assert.deepEqual(await faults(installed(), await site({ csp })), ["CSP_CONNECT_SRC"], csp);
  }
  for (const csp of [`${WASM_OK}; connect-src 'self'`, `${WASM_OK}; connect-src https://other.example 'self'`, `default-src 'self'; ${WASM_OK}`]) {
    assert.equal((await at(installed(), await site({ csp }))).code, 0, csp);
  }
});

test("doctor --url exits 1 for an engine address that answers with another status, the page instead of the engine, or the wrong type", async () => {
  assert.deepEqual(await faults(installed(), await site({}, null)), ["ENGINE_FETCH"]);
  assert.deepEqual(await faults(installed(), await site({}, { status: 500, body: "boom" })), ["ENGINE_FETCH"]);
  const fallback = await site({}, { body: "<!doctype html><div></div>" });
  assert.deepEqual(await faults(installed(), fallback), ["ENGINE_NOT_WASM"]);
  const [what] = found(await at(installed(), fallback)).map((f) => f.what);
  assert.match(what, /\/app_bg\.wasm .*"<!do/);
  assert.deepEqual(await faults(installed(), await site({}, { type: "text/plain", body: WASM })), ["ENGINE_CONTENT_TYPE"]);
  assert.equal((await at(installed(), await site({}, { type: "application/wasm; charset=binary", body: WASM }))).code, 0);
});

test("doctor --url in a Next app exits 1 with ENGINE_URL_STALE when the served engine is not byte-equal to the installed one", async () => {
  const other = Buffer.from(WASM_NEXT);
  other[other.length - 1] ^= 1;
  assert.deepEqual(await faults(nextInstalled(), await site({ body: BARE }, { type: "application/wasm", body: other })), ["ENGINE_URL_STALE"]);
  assert.equal((await at(nextInstalled(), await site({ body: BARE }, { type: "application/wasm", body: WASM_NEXT }))).code, 0);
  assert.deepEqual(await faults(nextInstalled(), await site({ body: BARE }, null)), ["ENGINE_FETCH"]);
  // A Vite project has no copy to compare: the served bytes are checked as the engine only.
  assert.equal((await at(installed(), await site({}, { type: "application/wasm", body: other }))).code, 0);
});

test("doctor --url skips the engine address, and does not fail the page, when a Vite page names none (its engine has a hashed address only the script holds)", async () => {
  for (const engine of [null, { body: "<!doctype html><div></div>" }]) {
    const r = await at(installed(), await site({ body: BARE }, engine));
    assert.equal(r.code, 0, r.out + r.err);
    const skipped = (JSON.parse(r.out).findings as Finding[]).filter((f) => f.status === "skipped");
    assert.deepEqual(skipped.map((f) => f.code), ["ENGINE_URL_SKIPPED", "GPU_SKIPPED"]);
    assert.match(skipped[0].what, /names no \.wasm address/);
  }
});

test("doctor --url finds a Vite build's hashed engine file in a module script of the page, and checks it", async () => {
  for (const named of ["/assets/app_bg-1a2b3c.wasm", "app_bg-1a2b3c.wasm"]) {
    const script = { type: "text/javascript", body: `export const engine = new URL("${named}", import.meta.url);` };
    const base = await serve({ "/": { body: `<script type="module" crossorigin src="/assets/index-9f8e.js"></script>${BARE}` }, "/assets/index-9f8e.js": script, "/assets/app_bg-1a2b3c.wasm": GOOD });
    const r = await at(installed(), base);
    assert.equal(r.code, 0, r.out + r.err);
    assert.deepEqual(codes(r).filter((c) => c.startsWith("ENGINE_URL")), ["ENGINE_URL_OK"]);
    assert.match((JSON.parse(r.out).findings as Finding[]).find((f) => f.code === "ENGINE_URL_OK")?.what ?? "", /\/assets\/app_bg-1a2b3c\.wasm answers 200/);
  }
  // The engine the script names is checked like any other: a missing file fails, it is not skipped.
  const gone = await serve({ "/": { body: `<script type="module" src="/a.js"></script>${BARE}` }, "/a.js": { type: "text/javascript", body: `fetch("/assets/gone-1.wasm")` } });
  assert.deepEqual(await faults(installed(), gone), ["ENGINE_FETCH"]);
});

test("doctor --url exits 2 for a page that does not answer 200, so an error page's policy is never read", async () => {
  for (const status of [404, 500]) {
    const r = await at(installed(), await site({ status, csp: "script-src 'self'" }));
    assert.equal(r.code, 2, r.out);
    assert.match(r.err, new RegExp(`answered with status ${status}`));
  }
});

test("CSP_OK says the engine can compile, and connect only when connect-src was checked against a known engine address", async () => {
  const okText = (r: Awaited<ReturnType<typeof at>>) => (JSON.parse(r.out).findings as Finding[]).find((f) => f.code === "CSP_OK")?.what ?? "";
  const config = { "vercel.json": JSON.stringify({ headers: [{ source: "/", headers: [{ key: "Content-Security-Policy", value: WASM_OK }] }] }) };
  assert.doesNotMatch(okText(doctor(installed({ files: config }))), /connect/);
  assert.doesNotMatch(okText(await at(installed({ files: config }), await site({ body: BARE }))), /connect/);
  assert.match(okText(await at(installed(), await site({ csp: WASM_OK }))), /compile and connect/);
  assert.doesNotMatch(okText(await at(installed(), await site({ csp: WASM_OK, body: BARE }))), /connect/);
  assert.match(okText(await at(nextInstalled(), await site({ csp: WASM_OK, body: BARE }))), /compile and connect/);
});

test("doctor --url exits 2 when the page cannot be fetched, and the flag needs a value", async () => {
  const r = await at(installed(), "http://127.0.0.1:1/");
  assert.equal(r.code, 2);
  assert.match(r.err, /http:\/\/127\.0\.0\.1:1\/ could not be fetched/);
  assert.equal(r.out, "");
  assert.equal(run(["doctor", "--url"], root).code, 2);
});

test("doctor reads the policy in next.config, vercel.json, public/_headers, netlify.toml and a meta tag, ignores a report-only one, and names the file", () => {
  const lacks = "default-src 'self'; script-src 'self'";
  const files: Record<string, string> = {
    "next.config.mjs": `const csp = \`${lacks}\n\`;\nexport default { async headers() { return [{ source: "/(.*)", headers: [{ key: "Content-Security-Policy", value: csp.replace(/\\n/g, "") }] }]; } };\n`,
    "vercel.json": JSON.stringify({ headers: [{ source: "/(.*)", headers: [{ key: "Content-Security-Policy", value: lacks }] }] }),
    "public/_headers": `/*\n  Content-Security-Policy: ${lacks}\n`,
    "netlify.toml": `[[headers]]\n  for = "/*"\n  [headers.values]\n    Content-Security-Policy = "${lacks}"\n`,
    "index.html": `<meta http-equiv="Content-Security-Policy" content="${lacks}">\n`,
  };
  for (const [file, text] of Object.entries(files)) {
    const [f] = problems(installed({ files: { [file]: text } }));
    assert.equal(f.code, "CSP_WASM_EVAL", file);
    assert.match(f.what, new RegExp(file.replace(".", "\\.")));
    assert.match(f.fix, /wasm-unsafe-eval/);
    const fixed = text.replaceAll("script-src 'self'", "script-src 'self' 'wasm-unsafe-eval'");
    assert.deepEqual((JSON.parse(doctor(installed({ files: { [file]: fixed } })).out).findings as Finding[]).map((x) => x.code), ["INSTALLED", "ONE_COPY", "ENGINE_OK", "CSP_OK", "STYLESHEET_OK", "GPU_SKIPPED"], file);
  }
  const reportOnly = JSON.stringify({ headers: [{ source: "/", headers: [{ key: "Content-Security-Policy-Report-Only", value: lacks }] }] });
  assert.equal(doctor(installed({ files: { "vercel.json": reportOnly } })).code, 0);
});

test("doctor names the directive to change: default-src when the policy has no script-src, script-src when it has one", () => {
  for (const [csp, names, not] of [["default-src 'self'", "default-src", "script-src"], ["script-src 'self'", "script-src", "default-src"]]) {
    const [f] = problems(installed({ files: { "index.html": `<meta http-equiv="Content-Security-Policy" content="${csp}">\n` } }));
    assert.equal(f.code, "CSP_WASM_EVAL", csp);
    for (const text of [f.what, f.fix]) assert.ok(text.includes(names) && !text.includes(not), `${csp}: ${text}`);
  }
});

test("doctor reads a next.config policy built from several strings (an array joined, or strings added) as one policy", () => {
  const wrap = (csp: string) => `const csp = ${csp};\nexport default { async headers() { return [{ source: "/(.*)", headers: [{ key: "Content-Security-Policy", value: csp }] }]; } };\n`;
  const shapes: Record<string, [good: string, bad: string]> = {
    "an array joined": [`["default-src 'self'", "script-src 'self' 'wasm-unsafe-eval'", "img-src 'self' data:"].join("; ")`, `["default-src 'self'", "script-src 'self'", "img-src 'self' data:"].join("; ")`],
    "strings added": [`"default-src 'self'; " +\n  "script-src 'self' 'wasm-unsafe-eval'; " +\n  "connect-src 'self'"`, `"default-src 'self'; " +\n  "script-src 'self'; " +\n  "connect-src 'self'"`],
  };
  for (const [shape, [good, bad]] of Object.entries(shapes)) {
    const ok = JSON.parse(doctor(installed({ files: { "next.config.mjs": wrap(good) } })).out).findings as Finding[];
    assert.deepEqual(ok.map((f) => f.code), ["INSTALLED", "ONE_COPY", "ENGINE_OK", "CSP_OK", "STYLESHEET_OK", "GPU_SKIPPED"], shape);
    assert.deepEqual(problems(installed({ files: { "next.config.mjs": wrap(bad) } })).map((f) => f.code), ["CSP_WASM_EVAL"], shape);
  }
});

test("doctor reads each header key of a next.config on its own: a route whose policy lacks 'wasm-unsafe-eval' fails even when another route's policy has it", () => {
  const route = (source: string, value: string) => `{ source: "${source}", headers: [{ key: "Content-Security-Policy", value: "${value}" }] }`;
  const wrap = (...routes: string[]) => `export default { async headers() { return [${routes.join(", ")}]; } };\n`;
  const good = route("/app", "script-src 'self' 'wasm-unsafe-eval'");
  const bad = route("/docs", "script-src 'self'");
  for (const routes of [[good, bad], [bad, good]]) assert.deepEqual(problems(installed({ files: { "next.config.mjs": wrap(...routes) } })).map((f) => f.code), ["CSP_WASM_EVAL"]);
  assert.equal(doctor(installed({ files: { "next.config.mjs": wrap(good, good) } })).code, 0);
});

test("doctor skips a report-only policy by the key that owns its strings, however far they sit from the key", () => {
  const pieces = `["default-src 'self'", "img-src 'self' data: https://images.example.com", "font-src 'self' https://fonts.example.com", "script-src 'self'"].join("; ")`;
  const wrap = (key: string) => `const csp = ${pieces};\nexport default { async headers() { return [{ source: "/(.*)", headers: [{ key: "${key}", value: csp }] }]; } };\n`;
  assert.equal(doctor(installed({ files: { "next.config.mjs": wrap("Content-Security-Policy-Report-Only") } })).code, 0);
  assert.deepEqual(problems(installed({ files: { "next.config.mjs": wrap("Content-Security-Policy") } })).map((f) => f.code), ["CSP_WASM_EVAL"]);
});

test("doctor --url checks the engine the page preloads, not the first .wasm its HTML names", async () => {
  const r = await at(installed(), await site({ body: `<a href="/sample.wasm">a sample</a><link rel="preload" href="/app_bg.wasm" as="fetch">${BARE}` }));
  assert.equal(r.code, 0, r.out + r.err);
  assert.ok(codes(r).includes("ENGINE_URL_OK"), r.out);
});
