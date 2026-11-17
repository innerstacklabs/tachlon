// The feature matrix has one source, docs/features.md. The README's "How it compares" table is that table, between two marker
// comments, with the product's name as the first column's heading. Called by scripts/features-matrix.ts; tested by scripts/test-features-matrix.ts.
import { readFileSync } from "node:fs";
import { join } from "node:path";

export const BEGIN = "<!-- features-matrix:begin (copied from docs/features.md by scripts/features-matrix.ts --write) -->";
export const END = "<!-- features-matrix:end -->";

/** The product's name, as `packages/gantt/src/product.ts` writes it (the name is written there, not here). */
export function productName(root: string): string {
  const m = /PRODUCT_NAME\s*=\s*"([^"]+)"/.exec(readFileSync(join(root, "packages", "gantt", "src", "product.ts"), "utf8"));
  if (!m) throw new Error("product.ts has no PRODUCT_NAME");
  return m[1];
}

/** The README's block for `features`: its first table, whose third column is headed with the product's name. */
export function renderBlock(features: string, name: string): string {
  const lines = features.split("\n");
  const start = lines.findIndex((l) => l.startsWith("|"));
  if (start < 0) throw new Error("docs/features.md has no table");
  let end = start;
  while (lines[end]?.startsWith("|")) end++;
  const head = lines[start].split("|");
  head[3] = ` ${name} `;
  return [head.join("|"), ...lines.slice(start + 1, end)].join("\n");
}

/** The `**Counts.**` line that belongs under the table: its numbered rows, and how many of their third-column cells begin `Yes`, `In progress` or `No`, a zero count left out. */
export function countsLine(features: string): string {
  const cells = renderBlock(features, "").split("\n").filter((l) => /^\| *\d+ *\|/.test(l)).map((l) => l.split("|")[3].trim());
  const parts = ["Yes", "In progress", "No"]
    .map((word) => [cells.filter((c) => new RegExp(`^${word}\\b`).test(c)).length, word] as const)
    .filter(([n]) => n > 0)
    .map(([n, word]) => `${n} ${word}`);
  return `**Counts.** ${cells.length} rows. This chart: ${parts.join(", ")}.`;
}

/** The text between the two markers of a README, trimmed, or null when a marker is missing. */
export function readmeBlock(readme: string): string | null {
  const a = readme.indexOf(BEGIN);
  const b = readme.indexOf(END);
  return a < 0 || b < a ? null : readme.slice(a + BEGIN.length, b).trim();
}

/** The README with the text between its markers replaced by `block`, or null when a marker is missing. */
export function withBlock(readme: string, block: string): string | null {
  const a = readme.indexOf(BEGIN);
  const b = readme.indexOf(END);
  return a < 0 || b < a ? null : `${readme.slice(0, a + BEGIN.length)}\n${block}\n${readme.slice(b)}`;
}
