// What `--record <dir>` keeps (gantt#1560, B7): a stub assistant that prints one JSON line and one other line goes through the real runner; the probes are stubbed.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { PRODUCT_NAME } from "../src/product.ts";
import { record, streaming } from "../../../scripts/lib/agent-install-record.ts";
import { ASSISTANTS, runInstall } from "../../../scripts/lib/agent-install-run.ts";
import { scriptLines } from "../../../scripts/lib/narrate.ts";

process.env.GIT_CONFIG_GLOBAL = "/dev/null";
process.env.GIT_CONFIG_NOSYSTEM = "1";
const root = mkdtempSync(join(tmpdir(), "agent-install-record-test-"));
after(() => rmSync(root, { recursive: true, force: true }));
const tarball = { file: join(root, "pkg.tgz"), version: "9.9.9" };
mkdirSync(join(root, "package")); // the registry reads the tarball's package.json (gantt#1688)
writeFileSync(join(root, "package", "package.json"), "{}");
execFileSync("tar", ["-czf", tarball.file, "-C", root, "package"]);
const probes = { network: async () => {}, build: () => ({ code: 0 }), doctor: () => ({ code: 0, out: "{}" }), mount: async () => ({ width: 800, height: 400, seconds: 3, pageErrors: 0 }) };
const stub = { bin: process.execPath, args: ["-e", `console.log('{"type":"init"}'); console.log("not json")`] };
let work = "";
before(async () => { work = (await runInstall(stub, "x", tarball, probes, { port: 4873 })).work; });

const pass = [{ check: "branch", ok: true, why: "" }, { check: "build", ok: true, why: "" }];
const fail = [pass[0]!, { check: "build", ok: false, why: "it failed" }];
const keep = (dir: string, assistant: string, results = pass) => record(join(root, dir), { assistant, work, results, timings: { run: 41.6 } });
const story = (dir: string, assistant: string) => scriptLines(readFileSync(join(root, dir, `story-${assistant}.txt`), "utf8"));

test("the stream keeps its JSON lines, the timings are kept, and the story is one narration line per cue", () => {
  keep("a", "claude");
  assert.equal(readFileSync(join(root, "a/stream-claude.jsonl"), "utf8"), '{"type":"init"}\n');
  assert.equal(JSON.parse(readFileSync(join(root, "a/results-claude.json"), "utf8")).timings.run, 41.6);
  const lines = story("a", "claude");
  assert.equal(lines[0], `Install ${PRODUCT_NAME} with a coding assistant.`);
  assert.match(lines.at(-1)!, /^2 of 2 checks pass, and the run took 42 seconds\.$/);
  assert.ok(streaming("claude", ASSISTANTS.claude!).args.includes("stream-json"));
  assert.deepEqual(streaming("codex", ASSISTANTS.codex!).args.slice(0, 3), ["exec", "--json", "-s"]);
});

test("the story says it works the same in others only when another assistant's run passed, whichever was recorded first", () => {
  keep("b", "codex");
  keep("b", "claude");
  assert.equal(story("b", "claude").at(-1), "The install works the same in others.");
  assert.equal(story("b", "codex").at(-1), "The install works the same in others.");
  keep("c", "codex", fail);
  keep("c", "claude");
  assert.ok(!story("c", "claude").join("\n").includes("others"));
  assert.ok(!story("c", "codex").join("\n").includes("others"));
});
