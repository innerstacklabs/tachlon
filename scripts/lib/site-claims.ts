// The site's publish gate (apps/site/site.css, "Claims not yet true"): a page publishes only when no
// [data-claim] is left. Called by scripts/site-claims.ts; tested by scripts/test-site-claims.ts.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

/** One claim left: its page, its 1-based line and the stories its value names (empty for a bare attribute). */
export interface Claim { file: string; line: number; stories: string[] }

const ATTR = /\sdata-claim(?![\w-])(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+)))?/g;

export function claimsIn(file: string, html: string): Claim[] {
  return [...html.matchAll(ATTR)].map((m) => ({
    file,
    line: html.slice(0, m.index).split("\n").length,
    stories: (m[1] ?? m[2] ?? m[3] ?? "").split(/\s+/).filter(Boolean),
  }));
}

/** The `.html` files under each directory (walked in name order); a path that is a file is taken as given. */
export function htmlFiles(paths: string[]): string[] {
  return paths.flatMap((p) => {
    if (!statSync(p).isDirectory()) return [p];
    const children = readdirSync(p).sort().map((name) => join(p, name));
    return children.flatMap((c) => (statSync(c).isDirectory() ? htmlFiles([c]) : c.endsWith(".html") ? [c] : []));
  });
}

/** The pages under `paths` and the claims left in them. Throws when a path or a page cannot be read. */
export function siteClaims(paths: string[]): { files: string[]; claims: Claim[] } {
  const files = htmlFiles(paths);
  return { files, claims: files.flatMap((f) => claimsIn(f, readFileSync(f, "utf8"))) };
}
