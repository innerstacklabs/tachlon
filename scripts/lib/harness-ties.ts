// What still ties the benchmark harness to this repo (gantt#1285), found before the harness is made public. Two classes:
// a text tie (a private issue, a person, a computer or a home folder named in a comment, a title or code) fails the check;
// an outside import (a relative import that leaves the harness) is printed only, since each one is a file to move or cut.
import { posix } from "node:path";

export type Tie = { kind: "text" | "import"; path: string; line: number; what: string };
export type File = { path: string; text: string };

const TEXT: [string, RegExp][] = [
  ["private issue", /\b[a-z][a-z-]*#\d+/],
  ["person", /\b(?:Dave|Remy|karpathy)\b/],
  ["computer", /\b(?:mini|hub)\b/],
  ["home folder", /homedir\(|~\/|\/home\/|\$HOME|\blifeos\b/],
];
const IMPORT = /(?:\bfrom\s*|\bimport\s*\(?\s*)["'](\.[^"']*)["']/g;

/** The files the harness is made of: both folders but the saved runs and `profile*`, which runs on a machine of ours. */
export const harnessFiles = (paths: string[]): string[] =>
  paths.filter((p) => /^demos\/(?:bench|dhtmlx-compare)\//.test(p) && !p.startsWith("demos/bench/results/") && !/^profile/.test(posix.basename(p)));

/** The ties of `files`; a `.json` file is data (the measured record, a fixture) and is not read, since a rerun rewrites it. */
export const harnessTies = (files: File[]): Tie[] => {
  const known = new Set(files.map((f) => f.path));
  const ties: Tie[] = [];
  for (const { path, text } of files.filter((f) => !f.path.endsWith(".json"))) {
    text.split("\n").forEach((line, i) => {
      for (const [what, re] of TEXT) if (re.test(line)) ties.push({ kind: "text", path, line: i + 1, what });
      for (const m of line.matchAll(IMPORT)) {
        const to = posix.join(posix.dirname(path), m[1]);
        if (!known.has(to) && !to.includes("node_modules")) ties.push({ kind: "import", path, line: i + 1, what: m[1] });
      }
    });
  }
  return ties;
};
