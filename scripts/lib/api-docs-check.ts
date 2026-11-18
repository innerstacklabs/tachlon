// Fails a public export of the package that has no doc comment (tl#1094). Reads the doc model API Extractor writes for the
// package entry (packages/gantt/api-extractor.json): `members[0]` is the entry point, and each of its members is one export with its
// `docComment` as the source text (an empty string when the export has none).

interface Member {
  kind?: string;
  name?: string;
  docComment?: string;
}

// The entry point's exports, or an error when the text is not a doc model with at least one export (a check that read nothing is not a pass).
export function entryExports(text: string): Member[] {
  const model = JSON.parse(text) as { kind?: string; members?: Array<{ kind?: string; members?: Member[] }> };
  const members = model.kind === "Package" ? model.members?.find((m) => m.kind === "EntryPoint")?.members : undefined;
  if (!members?.length) throw new Error("no entry point with exports in the doc model");
  return members;
}

// The sentence(s) before the first block tag: what a reader sees on the page.
export function summary(comment: string): string {
  const lines = comment.replace(/^\s*\/\*\*/, "").replace(/\*\/\s*$/, "").split("\n").map((l) => l.replace(/^\s*\*? ?/, ""));
  const tag = lines.findIndex((l) => l.startsWith("@"));
  return lines.slice(0, tag < 0 ? undefined : tag).join("\n").trim();
}

// The sorted names of the exports whose doc comment is missing or has no summary.
export function undocumentedExports(text: string): string[] {
  return entryExports(text).filter((m) => !summary(m.docComment ?? "")).map((m) => `${m.name} (${m.kind})`).sort();
}
