// The local package server (gantt#1560, build row B5): the packument's integrity is the tarball's, the tarball is served byte for byte,
// and any other package redirects to npmjs (the redirect is read, never followed: no network).
import assert from "node:assert/strict";
import { execFile, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import type { Server } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { promisify } from "node:util";
import { startRegistry } from "../../../scripts/lib/local-registry.ts";
import { PRODUCT_NAME } from "../src/product.ts";

const NAME = PRODUCT_NAME.toLowerCase();
const PORT = 4871; // never 4173 or 4300 (lifeos#3278)
const base = `http://127.0.0.1:${PORT}`;
const dir = mkdtempSync(join(tmpdir(), "registry-"));
const tgz = join(dir, "pkg.tgz");
let server: Server;

before(async () => {
  mkdirSync(join(dir, "package"));
  writeFileSync(join(dir, "package", "package.json"), JSON.stringify({ name: NAME, version: "0.0.1", bin: { [NAME]: "cli.js" }, engines: { node: ">=22" } }));
  writeFileSync(join(dir, "package", "cli.js"), "#!/usr/bin/env node\n");
  execFileSync("tar", ["-czf", tgz, "-C", dir, "package"]);
  server = await startRegistry(tgz, "0.0.1", PORT);
});
after(() => {
  server.closeAllConnections();
  server.close();
  rmSync(dir, { recursive: true, force: true });
});

test("the packument carries the tarball's integrity, and the tarball is served as it is", async () => {
  const bytes = readFileSync(tgz);
  const packument = await (await fetch(`${base}/${NAME}`)).json();
  const dist = packument.versions["0.0.1"].dist;
  assert.equal(packument["dist-tags"].latest, "0.0.1");
  assert.equal(dist.integrity, `sha512-${createHash("sha512").update(bytes).digest("base64")}`);
  assert.ok(Buffer.from(await (await fetch(dist.tarball)).arrayBuffer()).equals(bytes));
});

test("the listing carries the manifest's bin and engines, and an install links the bin", async () => {
  const listed = (await (await fetch(`${base}/${NAME}`)).json()).versions["0.0.1"];
  assert.deepEqual(listed.bin, { [NAME]: "cli.js" });
  assert.deepEqual(listed.engines, { node: ">=22" });
  const project = join(dir, "project");
  mkdirSync(project);
  writeFileSync(join(project, "package.json"), "{}");
  await promisify(execFile)("npm", ["install", "--registry", base, "--no-audit", "--no-fund", NAME], { cwd: project }); // async: the registry is this process
  assert.ok(existsSync(join(project, "node_modules", ".bin", NAME)));
});

test("another package redirects to npmjs", async () => {
  const res = await fetch(`${base}/left-pad`, { redirect: "manual" });
  assert.equal(res.status, 302);
  assert.equal(res.headers.get("location"), "https://registry.npmjs.org/left-pad");
});
