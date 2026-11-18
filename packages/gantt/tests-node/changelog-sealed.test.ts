// A published release's section of CHANGELOG.md does not change: releases.json holds a digest of each one, and a line for the next release goes under `## Unreleased`.
import { test } from "node:test";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { expect } from "@playwright/test";

const read = (path: string) => readFileSync(new URL(path, import.meta.url), "utf8");
const releases: { version: string; changelog?: string }[] = JSON.parse(read("../releases.json"));
const notOnNpm = new Set(["0.3.1"]); // in releases.json and open to changelog lines until published: its release pull request records the digest and deletes this line
const parts = read("../../../CHANGELOG.md").split(/^(?=## )/m).filter((s) => s.startsWith("## "));
const versions = parts.map((s) => /^## (\S+)/.exec(s)?.[1] ?? "");
const sections = new Map(parts.map((s, i) => [versions[i], s.trimEnd()]));

test("no version has two headings in CHANGELOG.md", () => {
  expect(versions.filter((v, i) => versions.indexOf(v) !== i), "a version with a second heading: the digest covers one section only").toEqual([]);
});

test("each released version's section of CHANGELOG.md matches the digest in releases.json", () => {
  for (const { version, changelog } of releases) {
    const section = sections.get(version);
    if (section === undefined) {
      expect(changelog, `${version} has a digest but CHANGELOG.md has no "## ${version}" section: it was renamed or deleted`).toBeUndefined();
      continue;
    }
    if (notOnNpm.has(version)) continue;
    const digest = createHash("sha256").update(section).digest("hex");
    const record = `record "changelog": "${digest}" on the ${version} entry of packages/gantt/releases.json`;
    expect(changelog, `${version} is released and has no digest: ${record}`).toBeDefined();
    expect(digest, `the ${version} section changed after release: put the line under "## Unreleased", or ${record} in a release pull request`).toBe(changelog);
  }
});
