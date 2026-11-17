// Checks the Inworld voice (see scripts/lib/voice-inworld.ts) with an injected fetch. Run from anywhere:
//   node scripts/test-voice-inworld.ts
// No real voice and no network. Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { inworldVoice } from "./lib/voice-inworld.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-voice-inworld: FAIL ${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
};

const mp3 = new Uint8Array([0x49, 0x44, 0x33, 0x04, 0x00, 0xff, 0xfb]); // an ID3 header, then a frame sync
const calls: { url: string; init: RequestInit }[] = [];
const reply = (status: number, body: unknown): typeof fetch => async (url, init) => {
  calls.push({ url: String(url), init: init! });
  return new Response(JSON.stringify(body), { status });
};
const ok = reply(200, { audioContent: Buffer.from(mp3).toString("base64") });
const spoken = await inworldVoice({ key: "k-secret", voiceId: "Ashley", fetch: ok, probeSeconds: (b) => b.length / 2 })("Open the plan.");
check("the endpoint", calls[0]?.url, "https://api.inworld.ai/tts/v1/voice");
check("the auth header", (calls[0]?.init.headers as Record<string, string>).Authorization, "Basic k-secret");
check("the body", JSON.parse(String(calls[0]?.init.body)), { text: "Open the plan.", voiceId: "Ashley", modelId: "inworld-tts-1", outputFormat: "mp3" });
check("the audio decodes", [...spoken.bytes], [...mp3]);
check("the length comes from the probe", spoken.seconds, 3.5);

const denied = await inworldVoice({ key: "k-secret", voiceId: "Ashley", fetch: reply(401, { message: "bad key" }) })("x").catch((e: Error) => e.message);
check("a 401 names the status and not the key", [/\b401\b/.test(denied as string), (denied as string).includes("k-secret")], [true, false]);

const dir = await mkdtemp(join(tmpdir(), "test-voice-inworld-"));
try {
  const trap = join(dir, "trap.mjs");
  const mark = JSON.stringify(join(dir, "fetched"));
  await writeFile(trap, `globalThis.fetch = () => { process.getBuiltinModule("node:fs").writeFileSync(${mark}, ""); throw new Error("fetch"); };\n`);
  await writeFile(join(dir, "script.txt"), "One.\n");
  const cli = fileURLToPath(new URL("narrate.ts", import.meta.url));
  const env: NodeJS.ProcessEnv = { ...process.env, NODE_OPTIONS: `--import=${trap}` };
  delete env.INWORLD_API_KEY;
  const run = spawnSync(process.execPath, [cli, join(dir, "script.txt"), join(dir, "out"), "--voice", "inworld"], { env, encoding: "utf8" });
  check("no INWORLD_API_KEY exits 2 with no call", [run.status, existsSync(join(dir, "fetched"))], [2, false]);
} finally {
  await rm(dir, { recursive: true, force: true });
}

if (failed) process.exit(1);
console.log("test-voice-inworld: ok");
