// What the book's user pages must keep. In `SUMMARY.md` the `# For developers` part runs from its title to the next
// part title: the pages in it are for developers, and every other page is a user page. Pure functions over text; the
// CLI in `scripts/check-book-pages.ts` reads the files.

import { posix } from "node:path";

export const DEVELOPERS_TITLE = "# For developers";

/** The links each user page must carry: the page, then the sample ids it must open with `?sample=<id>`. */
export const REQUIRED_LINKS: ReadonlyArray<{ page: string; samples: readonly string[] }> = [
  { page: "index.md", samples: ["office-fitout"] },
  { page: "open-a-schedule.md", samples: ["office-fitout"] },
  { page: "j2-navigate.md", samples: ["demo-50k"] },
];

/** Strings a user page must not contain: they point at the repository's own tooling, not at the app. */
export const FORBIDDEN = ["demos/", "run.sh"] as const;

/** The pages listed in SUMMARY.md outside the developers part, in order, or an error if the part title is missing. */
export function userPages(summary: string): { pages: string[] } | { error: string } {
  const lines = summary.split("\n");
  const at = lines.findIndex((l) => l.trim() === DEVELOPERS_TITLE);
  if (at < 0) return { error: `SUMMARY.md has no "${DEVELOPERS_TITLE}" title` };
  const pages: string[] = [];
  const end = lines.findIndex((l, i) => i > at && l.startsWith("# "));
  for (const line of [...lines.slice(0, at), ...(end < 0 ? [] : lines.slice(end))]) {
    const m = /\]\(([^)#]+\.md)(?:#[^)]*)?\)/.exec(line);
    if (m) pages.push(m[1]);
  }
  return { pages };
}

/** The sample ids `build-samples.mjs` defines: every `id: "<id>"` in it. */
export function definedSamples(source: string): Set<string> {
  return new Set([...source.matchAll(/\bid:\s*"([a-z0-9-]+)"/g)].map((m) => m[1]));
}

/** The ids a page opens with `?sample=<id>`. */
export function linkedSamples(page: string): string[] {
  return [...page.matchAll(/\?sample=([A-Za-z0-9_-]+)/g)].map((m) => m[1]);
}

/**
 * Markdown links inside HTML blocks, as `line: text`. A line that starts with a tag opens an HTML block that runs to the
 * next blank line (CommonMark), and Markdown is not parsed inside it, so `[text](url)` would show as raw text. Fenced
 * code is skipped.
 */
export function markdownLinksInHtml(page: string): string[] {
  const found: string[] = [];
  let fence = false;
  let inHtml = false;
  page.split("\n").forEach((line, i) => {
    if (/^\s{0,3}(```|~~~)/.test(line)) fence = !fence;
    if (fence) return;
    if (line.trim() === "") inHtml = false;
    else if (/^\s{0,3}<\/?[A-Za-z]/.test(line)) inHtml = true;
    if (inHtml && /\[[^\]]+\]\([^)\s]+\)/.test(line)) found.push(`line ${i + 1}: ${line.trim().slice(0, 60)}`);
  });
  return found;
}

/** Every problem found, as one line each; empty when the book is right. `read` returns a page's text, or null if absent. */
export function checkBook(summary: string, samplesSource: string, read: (page: string) => string | null): string[] {
  const problems: string[] = [];
  const listed = userPages(summary);
  if ("error" in listed) return [listed.error];
  const defined = definedSamples(samplesSource);
  const texts = new Map<string, string>();
  for (const page of new Set([...listed.pages, "index.md"])) {
    const text = read(page);
    if (text === null) problems.push(`${page}: listed in SUMMARY.md but not found`);
    else texts.set(page, text);
  }
  for (const [page, text] of texts) {
    for (const bad of FORBIDDEN) if (text.includes(bad)) problems.push(`${page}: a user page contains "${bad}"`);
    for (const hit of markdownLinksInHtml(text)) problems.push(`${page}: a Markdown link inside an HTML block is not parsed (${hit})`);
    for (const id of linkedSamples(text)) {
      if (!defined.has(id)) problems.push(`${page}: ?sample=${id} names a sample build-samples.mjs does not define`);
    }
  }
  for (const { page, samples } of REQUIRED_LINKS) {
    const text = texts.get(page);
    if (text === undefined) {
      problems.push(`${page}: required page is not a user page`);
      continue;
    }
    for (const id of samples) {
      if (!linkedSamples(text).includes(id)) problems.push(`${page}: no link to /?sample=${id}`);
    }
  }
  return problems;
}

/** The largest a file under `media/` may be, in bytes. Today's largest is `screencast.mp4`, 6,175,675 bytes (a link to `docs/media/`). */
export const MEDIA_BUDGET_BYTES = 7_000_000;

/** Media files kept without a page: the mascot. */
export const KEPT_UNUSED: ReadonlySet<string> = new Set(["media/mascot.webp"]);

/** Link targets that are not files in `book/src/`: URLs and root-relative paths. */
const NOT_SOURCE = /^(?:[a-z][a-z0-9+.-]*:|\/)/i;

const outsideFences = (text: string): string => {
  let fence = false;
  return text.split("\n").filter((l) => (/^\s{0,3}(```|~~~)/.test(l) ? ((fence = !fence), false) : !fence)).join("\n");
};

/** The heading ids mdBook gives a page: lower case, punctuation dropped, spaces to `-`. */
export function anchorsOf(page: string): Set<string> {
  const headings = [...outsideFences(page).matchAll(/^#{1,6}\s+(.+?)\s*#*$/gm)];
  return new Set(headings.map((m) => m[1].toLowerCase().replace(/[^\p{L}\p{N}_\s-]/gu, "").replace(/\s+/g, "-")));
}

/**
 * Problems with the files the pages point at. `texts` maps each page to its text; `files` maps every file under `book/src/`
 * to its size. A relative link, `![`, `src`, `poster` or `href` must name a file that exists (and an `#anchor` a heading in
 * it); a file under `media/` must be used by a page (an `{{#include}}` counts) and no larger than `MEDIA_BUDGET_BYTES`.
 */
export function checkAssets(texts: ReadonlyMap<string, string>, files: ReadonlyMap<string, number>): string[] {
  const problems: string[] = [];
  const used = new Set<string>();
  for (const [page, text] of texts) {
    const body = outsideFences(text);
    const targets = [...body.matchAll(/\]\(([^)\s]+)[^)]*\)|\b(?:src|poster|href)="([^"]+)"/g)].map((m) => m[1] ?? m[2]);
    for (const target of targets.filter((t) => !NOT_SOURCE.test(t))) {
      const [path, anchor] = target.split("#");
      const file = path === "" ? page : posix.join(posix.dirname(page), path);
      const dest = texts.get(file);
      if (!files.has(file)) problems.push(`${page}: "${target}" points at a file that does not exist`);
      else if (anchor && dest !== undefined && !anchorsOf(dest).has(anchor)) problems.push(`${page}: "${target}" points at a heading that does not exist`);
      used.add(file);
    }
    for (const m of body.matchAll(/\{\{#include\s+([^\s}:]+)/g)) used.add(posix.join(posix.dirname(page), m[1]));
  }
  for (const [file, size] of files) {
    if (!file.startsWith("media/")) continue;
    if (!used.has(file) && !KEPT_UNUSED.has(file)) problems.push(`${file}: no page uses this file`);
    if (size > MEDIA_BUDGET_BYTES) problems.push(`${file}: ${size} bytes is over the ${MEDIA_BUDGET_BYTES}-byte media budget (MEDIA_BUDGET_BYTES)`);
  }
  return problems;
}
