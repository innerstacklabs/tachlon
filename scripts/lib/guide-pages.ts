// The checks on the embeddable user guide: the files under the chart's `guide/` folder, written for any product and included in the book by anchor.
// The folder's `README.md` owns the format. Pure functions over text: `scripts/check-guide.ts` does the reading.
import { posix } from "node:path";

/** What the checks read. `files` and `pages` map a path from the repo root to the file's text. */
export type Guide = {
  /** Every markdown file in the guide folder. */
  files: ReadonlyMap<string, string>;
  /** Every book page. */
  pages: ReadonlyMap<string, string>;
  /** The names a `needs` term may carry, by its kind (`NAMED_TERMS`). */
  names: Readonly<Record<string, ReadonlySet<string>>>;
  /** The English of every text key: what a bold label may be. */
  labels: ReadonlySet<string>;
  /** The ids of the hosted page's samples. */
  samples: readonly string[];
};

/** The files that describe the guide and hold no feature's text: of the rules, only the spelling rule reads them. */
export const FRAME_FILES: ReadonlySet<string> = new Set(["README.md", "PROMPT.md"]);
/** The comments a guide file may carry, and the terms of a `needs` comment: the two that stand alone and the three that carry a name. */
export const COMMENTS = ["needs", "control", "shot"] as const;
export const PLAIN_TERMS: readonly string[] = ["chart", "edit"];
export const NAMED_TERMS: readonly string[] = ["option", "event", "call"];

type Rule = (guide: Guide) => string[];
type Part = { heading: string; body: string };

const features = (guide: Guide): [string, string][] => [...guide.files].filter(([file]) => !FRAME_FILES.has(posix.basename(file)));
const comments = (text: string, kind: (typeof COMMENTS)[number]): string[] => [...text.matchAll(new RegExp(`<!--\\s*${kind}:\\s*(.*?)\\s*-->`, "g"))].map((m) => m[1]!);
/** The terms of the first `needs` comment in the text, or null when it has none. */
const termsOf = (text: string): string[] | null => comments(text, "needs")[0]?.split(",").map((term) => term.trim()) ?? null;
/** A file's parts: the text before its first `##` (headed by the title), then each `##` section. */
const parts = (text: string): Part[] => text.split(/^(?=## )/m).map((body) => ({ heading: /^#+ (.*)$/m.exec(body)?.[1] ?? "", body }));
const ITEM = /^\s*(?:\d+\.|[-*])\s/;

const FORBIDDEN: ReadonlyArray<[RegExp, string]> = [
  [/<(?:img|video|figure)\b|\bmedia\/|\bclass=|\{\{#/, "a picture, a class or an include: write a shot comment"],
  [/\?sample=|\]\((?:\/|[^)\s]*\.md\b)/, "a sample link, or a link to a page or a path"],
  [/\b(?:docs|packages|crates|apps|demos|scripts)\/|#\d+|\bdecision \d+|Where it is checked/i, "a repository path, an issue id, a decision number or the proof: it stays in the book page"],
];
const squash = (text: string): string => text.toLowerCase().replace(/[\s-]/g, "");

/** Rule 1: nothing that is true only of the hosted page or this repository. */
const forbidden: Rule = (guide) =>
  features(guide).flatMap(([file, text]) => [
    ...FORBIDDEN.flatMap(([pattern, what]) => (pattern.exec(text) ?? []).slice(0, 1).map((hit) => `${file}: "${hit}" is ${what}`)),
    ...guide.samples.filter((id) => squash(text).includes(squash(id))).map((id) => `${file}: names the sample "${id}": write a neutral example`),
  ]);

/** Rule 2: every part that holds text is under a `needs` comment, its own or the one under the title, and every term names something real. */
const needs: Rule = (guide) =>
  features(guide).flatMap(([file, text]) => {
    const [top, ...sections] = parts(text);
    const real = (term: string): boolean => {
      const [kind = "", name = ""] = term.split(":");
      return PLAIN_TERMS.includes(term) || (NAMED_TERMS.includes(kind) && (guide.names[kind]?.has(name) ?? false));
    };
    const hasText = (part: Part): boolean => part.body.replace(/^#.*$/m, "").replace(/<!--[\s\S]*?-->/g, "").trim() !== "";
    return [
      ...[top!, ...sections].filter((part) => hasText(part) && (termsOf(part.body) ?? termsOf(top!.body)) === null).map((part) => `${file}: "${part.heading}" is under no needs comment`),
      ...comments(text, "needs").flatMap((list) => list.split(",").map((term) => term.trim())).filter((term) => !real(term)).map((term) => `${file}: the needs term "${term}" names nothing the package has`),
    ];
  });

/** What is wrong with an anchor name by the order its `ANCHOR` and `ANCHOR_END` comments come in; any other order is a name used twice. */
const UNPAIRED: Readonly<Record<string, string>> = { ANCHOR: "has no ANCHOR_END after it", ANCHOR_END: "has no ANCHOR before it", "ANCHOR_END ANCHOR": "has its ANCHOR_END before its ANCHOR" };

/** Rule 3: every anchor of a guide file pairs with an end and is included by exactly one book page, and no book page includes a guide file whole. */
const includes: Rule = (guide) => {
  const problems: string[] = [];
  const count = new Map<string, number>();
  for (const [page, text] of guide.pages)
    for (const m of text.matchAll(/\{\{#include\s+([^\s}:]+)(?::([\w-]+))?/g)) {
      const file = posix.join(posix.dirname(page), m[1]!);
      if (!guide.files.has(file)) continue;
      if (m[2] === undefined) problems.push(`${page}: includes ${file} whole: include it by anchor`);
      else count.set(`${file}:${m[2]}`, (count.get(`${file}:${m[2]}`) ?? 0) + 1);
    }
  for (const [file, text] of features(guide)) {
    const marks = [...text.matchAll(/<!--\s*(ANCHOR(?:_END)?):\s*([\w-]+)\s*-->/g)].map((m) => [m[1]!, m[2]!] as const);
    for (const name of new Set(marks.map(([, n]) => n))) {
      const order = marks.filter(([, n]) => n === name).map(([kind]) => kind).join(" ");
      if (order !== "ANCHOR ANCHOR_END") problems.push(`${file}: the anchor "${name}" ${UNPAIRED[order] ?? "is used for more than one anchor"}`);
      if (!order.split(" ").includes("ANCHOR")) continue;
      const times = count.get(`${file}:${name}`) ?? 0;
      count.delete(`${file}:${name}`);
      if (times !== 1) problems.push(`${file}: the anchor "${name}" is included by ${times} book pages, not 1`);
    }
  }
  return [...problems, ...[...count.keys()].map((key) => `a book page includes ${key}, an anchor that file does not have`)];
};

/** Rule 4: a comment inside a list is indented with its item, because one at the left margin splits the list in two. An anchored include leaves the `ANCHOR` lines out, so they are not read. */
const listComments: Rule = (guide) =>
  features(guide).flatMap(([file, text]) => {
    const lines = text.split("\n").filter((line) => line.trim() !== "");
    const inList = (line: string): boolean => ITEM.test(line) || /^\s+\S/.test(line);
    return lines.filter((line, i) => /^<!--(?!\s*ANCHOR)/.test(line) && i > 0 && inList(lines[i - 1]!) && ITEM.test(lines[i + 1] ?? "")).map((line) => `${file}: the comment "${line}" at the left margin splits a list: indent it with its item`);
  });

/** Rule 5: text that needs a `call:` holds a `control` comment (anywhere in the file when the term is under the title), and a `shot` id is used once. */
const controls: Rule = (guide) => {
  const problems: string[] = [];
  const shots = new Map<string, number>();
  for (const [file, text] of features(guide)) {
    parts(text).forEach((part, i) => {
      const calls = (termsOf(part.body) ?? []).some((term) => term.startsWith("call:"));
      if (calls && comments(i === 0 ? text : part.body, "control").length === 0) problems.push(`${file}: "${part.heading}" needs a call and holds no control comment`);
    });
    for (const id of comments(text, "shot").map((shot) => shot.split("|")[0]!.trim())) shots.set(id, (shots.get(id) ?? 0) + 1);
  }
  return [...problems, ...[...shots].filter(([, times]) => times > 1).map(([id, times]) => `the shot id "${id}" is used ${times} times, not once`)];
};

/** Rule 6: a bold label is the English of a text key, so a host that passes `texts` can map it. `**Goal:**` and the phrase that opens a numbered step are not labels. */
const labels: Rule = (guide) =>
  features(guide).flatMap(([file, text]) =>
    [...text.matchAll(/\*\*([^*]+)\*\*/g)]
      .filter((m) => m[1] !== "Goal:" && !/^\s*\d+\.\s+$/.test(text.slice(text.lastIndexOf("\n", m.index) + 1, m.index)))
      .map((m) => m[1]!.replace(/\s+/g, " "))
      .filter((label) => !guide.labels.has(label))
      .map((label) => `${file}: the bold label "${label}" is the English of no text key`),
  );

/** Rule 7: American English, in the frame files too. */
const spelling: Rule = (guide) =>
  [...guide.files].flatMap(([file, text]) =>
    [...text.matchAll(/centre|programme|behaviour|colour|organis|recognis|summaris|authoris|neighbour|labelled|cancelled|catalogue|grey/gi)].map((m) => `${file}: "${m[0]}" is British spelling`),
  );

const RULES: readonly Rule[] = [forbidden, needs, includes, listComments, controls, labels, spelling];

/** The problems with the guide, one line each; none when it keeps every rule. */
export function checkGuide(guide: Guide): string[] {
  return RULES.flatMap((rule) => rule(guide));
}
