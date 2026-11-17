// The SVAR side's font download (`svar-setup.ts`): SVAR's CDN fonts go into the scratch directory
// only at paths inside it and only with their pinned SHA-256. A fake CDN serves the files, so the
// test needs no network.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { FONT_SHA256, GANTT, localFonts } from "./svar-setup.ts";

const sha256 = (b: string | Buffer) => createHash("sha256").update(b).digest("hex");
const scratch = (prefix: string) => mkdtempSync(join(tmpdir(), prefix));

const CDN_CSS = `@font-face{font-family:"Open Sans";src:url(https://cdn.svar.dev/fonts/open-sans/regular.woff2) format("woff2"),url(https://cdn.svar.dev/fonts/open-sans/regular.woff) format("woff")}
.wx-willow-theme{font-family:Roboto}@font-face{font-family:Roboto;src:url(https://cdn.svar.dev/fonts/roboto/500.woff2)}`;

/** A scratch directory holding SVAR's stylesheet `css`, inside a parent of its own, and a fake CDN that serves `files`. */
function fontFixture(css: string, files: Record<string, string>) {
  const parent = scratch("bench-svar-fonts-");
  const dir = join(parent, "side");
  mkdirSync(join(dir, "node_modules", GANTT, "dist-full"), { recursive: true });
  writeFileSync(join(dir, "node_modules", GANTT, "dist-full", "index.css"), css);
  const asked: string[] = [];
  const cdn = async (url: string) => {
    const path = url.slice("https://cdn.svar.dev/".length);
    asked.push(path);
    return path in files ? { ok: true, status: 200, arrayBuffer: async () => new TextEncoder().encode(files[path]).buffer } : { ok: false, status: 404 };
  };
  const pins = Object.fromEntries(Object.entries(files).map(([p, t]) => [p, sha256(t)]));
  return { parent, dir, cdn, pins, asked };
}

const ICONS = 'src: url("./wx-icons.woff?5c1d") format("woff");';

test("the font download refuses a path from the CDN's stylesheet that leaves the scratch directory, and writes nothing outside it", async () => {
  const escape = 'src: url("./../../../escape.woff") format("woff");';
  const f = fontFixture(CDN_CSS, { "fonts/wxi/wx-icons.css": escape, "fonts/wxi/../../../escape.woff": "x", "fonts/open-sans/regular.woff2": "a", "fonts/open-sans/regular.woff": "b", "fonts/roboto/500.woff2": "c" });
  await assert.rejects(localFonts(f.dir, f.cdn, f.pins), /refusing to write fonts\/wxi\/\.\.\/\.\.\/\.\.\/escape\.woff: outside the scratch directory/);
  assert.equal(existsSync(join(f.parent, "escape.woff")), false, "nothing written outside");
  assert.equal(f.asked.includes("fonts/wxi/../../../escape.woff"), false, "not even fetched");
});

test("the font download keeps a SHA-256 per file: a file with other bytes, or one not pinned, is refused", async () => {
  const files = { "fonts/wxi/wx-icons.css": ICONS, "fonts/wxi/wx-icons.woff": "i", "fonts/open-sans/regular.woff2": "a", "fonts/open-sans/regular.woff": "b", "fonts/roboto/500.woff2": "c" };
  const ok = fontFixture(CDN_CSS, files);
  await localFonts(ok.dir, ok.cdn, ok.pins);
  assert.equal(readFileSync(join(ok.dir, "fonts", "roboto", "500.woff2"), "utf8"), "c");
  assert.doesNotMatch(readFileSync(join(ok.dir, "svar.css"), "utf8"), /cdn\.svar\.dev/);
  const changed = fontFixture(CDN_CSS, files);
  await assert.rejects(localFonts(changed.dir, changed.cdn, { ...changed.pins, "fonts/roboto/500.woff2": sha256("not c") }), /fonts\/roboto\/500\.woff2 has SHA-256 [0-9a-f]{64}, not the pinned [0-9a-f]{64}/);
  const unpinned = fontFixture(CDN_CSS, files);
  const { "fonts/wxi/wx-icons.woff": _, ...fewer } = unpinned.pins;
  await assert.rejects(localFonts(unpinned.dir, unpinned.cdn, fewer), /fonts\/wxi\/wx-icons\.woff is not a pinned font file/);
  // The pins setup uses name every file SVAR 2.8.2's stylesheet and theme load.
  assert.equal(Object.keys(FONT_SHA256).length, 15);
  for (const h of Object.values(FONT_SHA256)) assert.match(h as string, /^[0-9a-f]{64}$/);
});
