// The fresh apps the install matrix runs the README's prompt in (to#36). `scaffold` writes the starter into the folder; `page` is the starter page the
// install may not replace; `serve` is where the built app is served from ("next" runs `next start`). The plain page's `build` is a no-op: it has no bundler.
import { execFileSync, spawn } from "node:child_process";
import { cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { PACKAGE } from "./agent-install-check.ts";
import { serve } from "./consumer-check.ts";

export type App = { scaffold: (dir: string) => void; page: string; serve: string };
// A starter from the framework's own scaffolder: `head`, the folder, then `tail`; any git folder it makes goes, the harness makes its own.
const fresh = (head: string[], tail: string[]) => (dir: string) => { execFileSync(head[0]!, [...head.slice(1), basename(dir), ...tail], { cwd: dirname(dir), stdio: "ignore", input: "" }); rmSync(join(dir, ".git"), { recursive: true, force: true }); };
const PLAIN = '<!doctype html>\n<html lang="en">\n  <head><meta charset="UTF-8" /><title>Plain page</title></head>\n  <body>\n    <h1>Plain page</h1>\n  </body>\n</html>\n';
export const APPS: Record<string, App> = {
  "vite-react": { scaffold: (dir) => cpSync(resolve(import.meta.dirname, "../fixtures/agent-install/app"), dir, { recursive: true }), page: "src/App.tsx", serve: "dist" },
  vue: { scaffold: fresh(["npm", "create", "vite@latest"], ["--", "--template", "vue-ts", "--no-interactive"]), page: "src/App.vue", serve: "dist" },
  next: { scaffold: fresh(["npx", "--yes", "create-next-app@latest"], ["--ts", "--app", "--use-npm", "--no-tailwind", "--no-eslint", "--no-src-dir", "--skip-install", "--yes"]), page: "app/page.tsx", serve: "next" },
  html: { scaffold: (dir) => { mkdirSync(dir); writeFileSync(join(dir, ".gitignore"), "node_modules\n"); writeFileSync(join(dir, "index.html"), PLAIN); writeFileSync(join(dir, "package.json"), '{ "name": "plain-page", "private": true, "scripts": { "build": "true" } }\n'); }, page: "index.html", serve: "." },
};

/** The task after the install: the look and the data of the host, by the package's own `describe fit`. */
export const FIT_PROMPT = `Make the ${PACKAGE} chart in this app fit it. Follow \`npx ${PACKAGE} describe fit\`. The app's colors: background #10161c, text #e8edf2, accent #ff6a3d. The app's own records, each with its key: [{ key: "rel-101", title: "Plan release", start: "2026-11-02", days: 3 }, { key: "rel-102", title: "Run tests", start: "2026-11-05", days: 5 }, { key: "rel-103", title: "Deploy", start: "2026-11-12", days: 1 }]. Draw those records, each task keyed by its record's key, in place of the sample tasks, in the colors above. Run the build and npx ${PACKAGE} doctor until both pass. Commit on the same branch; do not push.`;

/** The built app served for the mount probe: its address and how to stop it. */
export async function open(dir: string, how: string): Promise<{ url: string; close: () => void }> {
  if (how !== "next") { const { server, url } = await serve(join(dir, how)); return { url, close: () => void server.close() }; }
  const child = spawn(join(dir, "node_modules/.bin/next"), ["start", "-p", "4882"], { cwd: dir, stdio: "ignore" });
  const url = "http://127.0.0.1:4882/";
  for (let i = 0; i < 60 && !(await fetch(url).then(() => true, () => false)); i++) await sleep(500);
  return { url, close: () => void child.kill() };
}
