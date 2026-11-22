// `init`: plans how to add the chart to a project (`planInit`, also `--dry-run`), then does it (`applyInit`): install the package, write the chart module.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, posix, relative } from "node:path";
import type { Manager, Project } from "./detect.ts";
import { readJsonObject, Refusal } from "./detect.ts";
import { copiesOf } from "./doctor.ts";
import { skillFile, skillPath } from "./skill.ts";
import { chartModule, type Kind } from "./template.ts";

export interface Own {
  name: string;
  version: string;
}

/** What the caller asked of `init`, beyond the folder: `spec` replaces the installed `<name>@<version>`, `plain` writes the module that needs no framework. */
export interface Options {
  explicitDir: boolean;
  dryRun: boolean;
  install: boolean;
  spec: string | undefined;
  plain: boolean;
  /** Also write the assistant skill, `.claude/skills/<name>/SKILL.md`. */
  skill: boolean;
}

export interface Plan {
  dir: string;
  manager: Manager;
  framework: Project["framework"];
  bundler: string;
  /** The install command, as one line; `--no-install` leaves it unrun. */
  install: string;
  /** New files, relative to the project folder; none is overwritten. */
  files: string[];
  /** The chart module is already there as `init` writes it and, when `init` installs, package.json lists the package (a second run): nothing to install or write. */
  done: boolean;
  dryRun: boolean;
}

/** Why `init` cannot run in `project`, or undefined. */
export function refusal(project: Project, o: Pick<Options, "explicitDir" | "plain">): string | undefined {
  if (project.workspace && !o.explicitDir) return `${project.dir} is a workspace root; pass --dir <the package to add it to>`;
  if (project.bundler === undefined && !o.plain) return `init supports Vite and found ${project.otherBundler ?? "no bundler"} instead; pass --plain to write a chart module that needs no framework (it imports the package by name and its stylesheet, which a page resolves only through a bundler or an import map and a stylesheet link)`;
  if (project.bundler === "next" && project.routerRoot === undefined) return "this is a Next.js project with neither an app/ nor a pages/ folder (looked in the project folder and src/); create an app/ or pages/ folder, then run init again";
  return undefined;
}

/** A file `init` writes: its path relative to the project folder, and its text (none for Next's engine copy, which only the plan names). */
export interface Output {
  path: string;
  text: string | undefined;
  /** A file that is left as it is, whatever it holds, when it is already there (the skill: a person may have edited it). */
  keep?: boolean;
}

function newFiles(project: Project, own: Own, plain: boolean, skill: boolean): Output[] {
  const extra: Output[] = skill ? [{ path: skillPath(own.name), text: skillFile(own.name), keep: true }] : [];
  return [...chartFiles(project, own, plain), ...extra];
}

function chartFiles(project: Project, own: Own, plain: boolean): Output[] {
  const kind: Kind = project.framework === "none" || plain ? "plain" : "react";
  const ext = project.typescript ? (kind === "plain" ? "ts" : "tsx") : kind === "plain" ? "js" : "jsx";
  const file = `${own.name}-chart.${ext}`;
  const text = chartModule(kind, project.typescript, own.name, project.bundler === "next");
  if (project.bundler === "next") return [{ path: posix.join(project.routerRoot ?? "", "components", file), text }, { path: "public/app_bg.wasm", text: undefined }];
  return [{ path: existsSync(join(project.dir, "src")) ? posix.join("src", file) : file, text }];
}

const installCommand = (project: Project, own: Own, o: Options): [string, string[]] => [project.manager, [project.manager === "npm" ? "install" : "add", o.spec ?? `${own.name}@${own.version}`]];

/** The plan for `project` and the files it writes, or the sentence that says why it cannot be made. */
export function planInit(project: Project, own: Own, o: Options): { plan: Plan; outputs: Output[] } | { error: string } {
  const why = refusal(project, o);
  if (why !== undefined) return { error: why };
  const outputs = newFiles(project, own, o.plain, o.skill);
  const taken = outputs.filter((f) => existsSync(join(project.dir, f.path)));
  // The engine copy (no text) is left as it is whatever it holds: `doctor` names a copy that is out of date.
  const same = (f: Output) => f.text === undefined || f.keep === true || readFileSync(join(project.dir, f.path), "utf8") === f.text;
  const blocking = taken.find((f) => !same(f));
  if (blocking !== undefined) return { error: `${blocking.path} already exists, and init never overwrites a file; move it aside or delete it, then run init again` };
  const [cmd, args] = installCommand(project, own, o);
  // Done when every file is there and, when init installs, package.json lists the package (an earlier `--no-install` run leaves it unlisted); a file that is missing from a partly set up project is still written.
  const pkg = readJsonObject(join(project.dir, "package.json")) as { dependencies?: Record<string, string>; devDependencies?: Record<string, string> };
  const done = taken.length === outputs.length && (!o.install || Object.hasOwn({ ...pkg.dependencies, ...pkg.devDependencies }, own.name));
  const files = (done ? outputs : outputs.filter((f) => !taken.includes(f))).map((f) => f.path);
  const plan: Plan = { dir: project.dir, manager: project.manager, framework: o.plain ? "none" : project.framework, bundler: project.bundler ?? "none", install: [cmd, ...args].join(" "), files, done, dryRun: o.dryRun };
  return { plan, outputs };
}

/** The engine file Next serves from `public/`: the installed package's copy, or a refusal when the package is not there to copy from. */
function engineBytes(project: Project, own: Own, installed: boolean): Buffer {
  const copy = copiesOf(project.dir, own.name)[0];
  const file = copy === undefined ? undefined : join(copy, "dist-lib", "app_bg.wasm");
  if (file === undefined || !existsSync(file)) throw new Refusal(`${own.name}/dist-lib/app_bg.wasm is not in node_modules, so there is no engine to copy to public/; ${installed ? "the package was installed and no file was written; install a version that ships dist-lib/app_bg.wasm" : "no file was written; install the package in this project"}, then run init again`);
  return readFileSync(file);
}

/** Installs the package (unless `o.install` is false), then writes each file; a failed install writes no file. Returns the lines to print. */
export function applyInit(project: Project, own: Own, o: Options, plan: Plan, outputs: Output[]): string[] {
  if (plan.done) return [`already set up, nothing changed: ${plan.files.join(", ")} is already here`];
  const lines: string[] = [];
  if (o.install) {
    const [cmd, args] = installCommand(project, own, o);
    const ran = spawnSync(cmd, args, { cwd: project.dir, encoding: "utf8", shell: process.platform === "win32" });
    if (ran.error || ran.status !== 0) throw new Refusal(`${plan.install} failed${ran.error ? `: ${ran.error.message}` : ` (exit ${ran.status})`}; init wrote no files; fix the install error${ran.error ? "" : " below"}, then run init again\n${(ran.stderr ?? "").trim()}`.trim());
    lines.push(`  ran: ${plan.install}`);
  } else lines.push("  install skipped (--no-install)");
  const todo = outputs.filter((f) => plan.files.includes(f.path));
  const bodies = todo.map((f) => f.text ?? engineBytes(project, own, o.install)); // read first: a missing engine writes no file
  const wrote: string[] = [];
  for (const [i, f] of todo.entries()) {
    try {
      mkdirSync(dirname(join(project.dir, f.path)), { recursive: true });
      writeFileSync(join(project.dir, f.path), bodies[i], { flag: "wx" });
    } catch (e) {
      const why = (e instanceof Error ? e.message : String(e)).replace(/\s+/g, " ");
      const code = (e as NodeJS.ErrnoException).code ?? "";
      throw new Refusal(`${f.path} cannot be written (${why}); ${o.install ? "the package was installed and " : ""}${wrote.length === 0 ? "no file was written" : `only ${wrote.join(", ")} was written`}; ${/^(EEXIST|ENOTDIR)$/.test(code) ? "move aside or delete the file the error names" : /^(EACCES|EPERM|EROFS)$/.test(code) ? "make the folder writable" : "fix what the error names"}, then run init again`);
    }
    wrote.push(f.path);
    lines.push(`  wrote: ${f.path}`);
  }
  return lines;
}

/** What to add to a page and run next, as lines. */
export function nextSteps(plan: Plan, outputs: Output[]): string[] {
  const module = `${plan.bundler === "next" ? "../components" : "."}/${posix.basename(outputs[0].path).replace(/\.[jt]sx?$/, "")}`; // a Next page sits in app/ or pages/, one folder below components/
  const line = /\.[jt]sx$/.test(outputs[0].path) ? `import Chart from "${module}"; // then render <Chart />` : `import { mountChart } from "${module}"; // then mountChart(<a div with the class "gantt" and a height>)`;
  return ["Add this line to a page:", `  ${line}`, `Then run: ${plan.manager} run dev`];
}

/** The plan as lines a person reads (`--dry-run`). */
export function describe(plan: Plan, cwd: string): string {
  const where = relative(cwd, plan.dir) || ".";
  return [
    `init (dry run) in ${where}`,
    `  package manager: ${plan.manager}`,
    `  framework: ${plan.framework} (${plan.bundler})`,
    ...(plan.done ? ["  already set up: nothing to install or write"] : [`  would run: ${plan.install}`, ...plan.files.map((f) => `  would write: ${f}`)]),
    "Nothing was written (--dry-run).",
    "",
  ].join("\n");
}
