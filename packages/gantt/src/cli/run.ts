// The command line: arguments in, exit code and the two streams out. `main.ts` only prints them and exits, so tests call this.
//   exit 0: done, the plan or a `describe` page printed, or `doctor` found nothing wrong. 1: `doctor` found problems. 2: could not run (bad arguments, a refusal).
import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { PRODUCT_NAME } from "../product.ts";
import { parseArgs } from "./args.ts";
import { type Bundle, describeTopic, readBundle, TOPICS } from "./describe.ts";
import { detect, findProjectDir, readJsonObject, Refusal } from "./detect.ts";
import { type Checks, describeFindings, diagnose } from "./doctor.ts";
import { applyInit, describe, nextSteps, type Own, planInit } from "./init.ts";
import { askBrowser, BROWSER_VARIABLE, type Gpu } from "./gpu.ts";
import { type Probe, probePage } from "./probe.ts";

export interface Result {
  code: 0 | 1 | 2;
  out: string;
  err: string;
}

/** This package's own name and version: `../../package.json` is the package's from `src/cli/` and from `dist-lib/cli/` alike. */
function own(): Own {
  const { name, version } = readJsonObject(fileURLToPath(new URL("../../package.json", import.meta.url)));
  return { name: String(name), version: String(version) };
}

const usage = (name: string): string =>
  `${PRODUCT_NAME} command line\n\nusage: ${name} init [--dry-run] [--dir <folder>] [--spec <package spec>] [--no-install] [--plain] [--skill] [--json]\n       ${name} doctor [--dir <folder>] [--data <file>] [--url <page>] [--json]\n       ${name} describe [${Object.keys(TOPICS).join("|")}] [--json]\n\n  init   install the package into a Vite or Next.js project and write the chart module (src/<name>-chart; for Next, components/<name>-chart and the engine file public/app_bg.wasm);\n         --dry-run only prints the plan;\n         --plain writes the module that needs no framework; --skill also writes the assistant skill .claude/skills/<name>/SKILL.md (never over a file that is there); a second run changes nothing\n  doctor check an installed chart and say how to fix each problem: the package listed and installed once, the engine file (in Next, its copy in public/), a stylesheet import, the page's Content-Security-Policy in the project's config (it must allow 'wasm-unsafe-eval');\n         --data checks a task data file; --url fetches the served page and checks its policy and that its engine address (the .wasm it names; in a Next app, else /app_bg.wasm) answers with the engine;\n         it writes nothing in the project, and asks a Chrome, Edge or Chromium on this machine (${BROWSER_VARIABLE} names one) whether the chart gets WebGPU or WebGL2\n\n  describe print what this installed version offers, as markdown (--json: as data): no topic lists the topics; fit is the steps that make the chart fit your product;\n         it needs no project\n\nexit codes: 0 done, 1 problems found, 2 could not run\n`;

const refuse = (err: string): Result => ({ code: 2, out: "", err: `${err}\n` });

/** Runs the command line `argv` (without `node` and the script) from the folder `cwd`; a file it cannot read is exit 2 and one sentence. `bundle` is what `describe` reads: a test passes it, and the installed CLI reads the two files beside it. `doctor --url` needs the page read first, and `doctor` the browser asked about the GPU: `runAsync` does both, and passes them as `probe` and `gpu`. */
export function run(argv: string[], cwd: string, probe?: Probe, gpu?: Checks["gpu"], bundle?: Bundle): Result {
  try {
    return runCommand(argv, cwd, probe, gpu, bundle);
  } catch (e) {
    if (e instanceof Refusal) return refuse(e.message);
    throw e;
  }
}

/** `run`, reading the page first when `doctor` is given `--url` and asking a browser (`askGpu`) about the GPU, only when `doctor` gets as far as that check; the entry `main.ts` calls. A page it cannot fetch is exit 2. */
export async function runAsync(argv: string[], cwd: string, askGpu: () => Promise<Gpu> = askBrowser): Promise<Result> {
  const args = parseArgs(argv);
  if ("error" in args || args.command !== "doctor" || args.help || args.version) return run(argv, cwd);
  try {
    const probe = args.url === undefined ? undefined : await probePage(args.url);
    let reached = false;
    const first = run(argv, cwd, probe, () => {
      reached = true;
      return undefined;
    });
    return reached ? run(argv, cwd, probe, await askGpu()) : first;
  } catch (e) {
    if (e instanceof Refusal) return refuse(e.message);
    throw e;
  }
}

function runCommand(argv: string[], cwd: string, probe: Probe | undefined, gpu: Checks["gpu"], bundle: Bundle | undefined): Result {
  const me = own();
  const args = parseArgs(argv);
  if ("error" in args) return refuse(`${args.error}\n\n${usage(me.name)}`);
  if (args.version) return { code: 0, out: `${me.version}\n`, err: "" };
  if (args.help) return { code: 0, out: usage(me.name), err: "" };
  if (args.command === "describe") {
    const said = describeTopic(bundle ?? readBundle(), args.topic, args.json, me.name);
    return "error" in said ? refuse(said.error) : { code: 0, out: said.out, err: "" };
  }
  if (args.command !== "init" && args.command !== "doctor") return refuse(`${args.command === undefined ? "no command" : `unknown command ${args.command}`}\n\n${usage(me.name)}`);
  const dir = findProjectDir(args.dir === undefined ? cwd : resolve(cwd, args.dir), args.dir !== undefined);
  if (dir === undefined) return refuse("no package.json here or in a folder above it; pass --dir <project folder>");
  if (args.url !== undefined && args.command !== "doctor") return refuse(`--url is for doctor\n\n${usage(me.name)}`);
  if (args.url !== undefined && probe === undefined) throw new Error("run cannot read a page: call runAsync for doctor --url");
  const project = detect(dir);
  if (args.command === "doctor") {
    if (project.workspace && args.dir === undefined) return refuse(`${dir} is a workspace root; pass --dir <the package to check>`);
    let findings;
    try {
      findings = diagnose(project, me, { data: args.data === undefined ? undefined : resolve(cwd, args.data), probe, gpu });
    } catch (e) {
      // A folder or file the check must read but cannot (permission, a vanished file) means it could not run: exit 2, never the exit 1 of a finding, and never a pass.
      const fs = e as NodeJS.ErrnoException;
      if (e instanceof Error && typeof fs.code === "string" && typeof fs.syscall === "string") throw new Refusal(`doctor could not read ${fs.path ?? "a file"} (${fs.code}); fix its permissions or remove it, then run doctor again`);
      throw e;
    }
    const code = findings.some((f) => f.status === "fail") ? 1 : 0;
    return { code, out: args.json ? `${JSON.stringify({ dir, findings }, null, 2)}\n` : describeFindings(findings, relative(cwd, dir) || "."), err: "" };
  }
  const options = { explicitDir: args.dir !== undefined, dryRun: args.dryRun, install: args.install, spec: args.spec, plain: args.plain, skill: args.skill };
  const made = planInit(project, me, options);
  if ("error" in made) return refuse(made.error);
  const { plan, outputs } = made;
  if (args.dryRun) return { code: 0, out: args.json ? `${JSON.stringify(plan, null, 2)}\n` : describe(plan, cwd), err: "" };
  const wrote = applyInit(project, me, options, plan, outputs);
  if (args.json) return { code: 0, out: `${JSON.stringify(plan, null, 2)}\n`, err: "" };
  return { code: 0, out: [`init in ${relative(cwd, dir) || "."}`, ...wrote, ...(plan.done ? [] : nextSteps(plan, outputs)), ""].join("\n"), err: "" };
}
