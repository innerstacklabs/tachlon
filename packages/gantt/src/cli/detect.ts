// What the command line reads from a project folder: where the project is, its package manager and its framework.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

export type Manager = "npm" | "pnpm" | "yarn" | "bun";
export type Bundler = "next" | "vite";

export interface Project {
  dir: string;
  manager: Manager;
  /** `next` or `react` when the project depends on it, else `none`. */
  framework: "next" | "react" | "none";
  bundler: Bundler | undefined;
  /** The first other bundler or meta-framework the dependencies name (`webpack`, `parcel`), for a refusal to name; undefined if none. */
  otherBundler: string | undefined;
  /** For Next, the folder that holds `app/` or `pages/`: the project folder or its `src/`. */
  routerRoot: string | undefined;
  typescript: boolean;
  workspace: boolean;
}

const OTHER_BUNDLERS = ["webpack", "parcel", "rollup", "esbuild", "@angular/cli", "astro", "@sveltejs/kit", "nuxt", "gatsby", "@remix-run/dev", "react-scripts"];

const LOCKFILES: [string, Manager][] = [
  ["pnpm-lock.yaml", "pnpm"],
  ["yarn.lock", "yarn"],
  ["bun.lock", "bun"],
  ["bun.lockb", "bun"],
  ["package-lock.json", "npm"],
];

/** A reason the command cannot run, in one sentence; `run` turns it into exit 2. */
export class Refusal extends Error {}

/** The JSON object in `file`, or a `Refusal` saying it cannot be read, is not JSON, or is not an object. */
export function readJsonObject(file: string): Record<string, unknown> {
  const why = (e: unknown): string => (e instanceof Error ? e.message : String(e)).replace(/\s+/g, " ");
  let text: string;
  try {
    text = readFileSync(file, "utf8");
  } catch (e) {
    throw new Refusal(`${file} cannot be read: ${why(e)}`);
  }
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (e) {
    throw new Refusal(`${file} is not valid JSON: ${why(e)}`);
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Refusal(`${file} is not a JSON object`);
  return value as Record<string, unknown>;
}

interface PackageJson {
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
  packageManager?: string;
  workspaces?: unknown;
}

/** The nearest folder at or above `start` that holds a package.json, or undefined; with `exact`, only `start` itself. */
export function findProjectDir(start: string, exact = false): string | undefined {
  for (let dir = resolve(start); ; dir = dirname(dir)) {
    if (existsSync(join(dir, "package.json"))) return dir;
    if (exact || dirname(dir) === dir) return undefined;
  }
}

/** The manager `dir` names by a lockfile, else by its package.json's `packageManager`; undefined if neither. */
function managerHere(dir: string, pkg: PackageJson): Manager | undefined {
  const locked = LOCKFILES.find(([file]) => existsSync(join(dir, file)));
  if (locked) return locked[1];
  const named = pkg.packageManager?.split("@")[0];
  return LOCKFILES.find(([, m]) => m === named)?.[1];
}

/** The manager of the nearest workspace root above `dir` (a `pnpm-workspace.yaml`, or a package.json with `workspaces`): its members share its lockfile. */
function managerOfWorkspaceRoot(dir: string): Manager | undefined {
  for (let up = dirname(dir); up !== dir; dir = up, up = dirname(up)) {
    let pkg: PackageJson = {};
    try {
      pkg = readJsonObject(join(up, "package.json")) as PackageJson;
    } catch {
      // no package.json here, or one that is not JSON: it is not a workspace root we can read
    }
    if (existsSync(join(up, "pnpm-workspace.yaml")) || pkg.workspaces !== undefined) return managerHere(up, pkg);
  }
  return undefined;
}

function managerOf(dir: string, pkg: PackageJson): Manager {
  return managerHere(dir, pkg) ?? managerOfWorkspaceRoot(dir) ?? "npm";
}

/** What `dir` (a folder with a package.json) is: its manager, framework and where Next keeps its routes. */
export function detect(dir: string): Project {
  const pkg = readJsonObject(join(dir, "package.json")) as PackageJson;
  const deps = { ...pkg.dependencies, ...pkg.devDependencies };
  const routerRoot = ["", "src"].find((r) => ["app", "pages"].some((f) => existsSync(join(dir, r, f))));
  return {
    dir,
    manager: managerOf(dir, pkg),
    framework: "next" in deps ? "next" : "react" in deps ? "react" : "none",
    bundler: "next" in deps ? "next" : "vite" in deps ? "vite" : undefined,
    otherBundler: OTHER_BUNDLERS.find((b) => b in deps),
    routerRoot,
    typescript: "typescript" in deps || existsSync(join(dir, "tsconfig.json")),
    workspace: pkg.workspaces !== undefined || existsSync(join(dir, "pnpm-workspace.yaml")),
  };
}
