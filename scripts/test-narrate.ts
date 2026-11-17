// Checks the narration module (see scripts/lib/narrate.ts) with a fake voice. Run from anywhere:
//   node scripts/test-narrate.ts
// Needs Node 22.18 or later (type stripping). No real voice and no network. Exits 1, one line per problem.
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { clipKey, narrate, scriptLines, vttTime, type Voice } from "./lib/narrate.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-narrate: FAIL ${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
};

const spoken: string[] = [];
const fake: Voice = async (text) => {
  spoken.push(text);
  return { bytes: new TextEncoder().encode(`audio:${text}`), seconds: text.length / 4 };
};

check("a timestamp past the hour", vttTime(3723.5), "01:02:03.500");
check("the script's blank lines drop", scriptLines("  Open the plan.\n\n Drag a bar. \n"), ["Open the plan.", "Drag a bar."]);
check("the key depends on the voice", clipKey("a", "x") === clipKey("b", "x"), false);

const dir = await mkdtemp(join(tmpdir(), "test-narrate-"));
try {
  const opts = { voice: fake, voiceId: "fake-1", dir };
  const first = await narrate(["Open the plan.", "Drag a bar."], opts);
  check("the first run speaks each line", spoken, ["Open the plan.", "Drag a bar."]);
  check("clips play back to back", first.map((c) => [c.start, c.seconds]), [[0, 3.5], [3.5, 2.75]]);
  check("the clip holds the voice's bytes", await readFile(join(dir, first[0]!.file), "utf8"), "audio:Open the plan.");
  check(
    "the captions",
    await readFile(join(dir, "narration.vtt"), "utf8"),
    "WEBVTT\n\n1\n00:00:00.000 --> 00:00:03.500\nOpen the plan.\n\n2\n00:00:03.500 --> 00:00:06.250\nDrag a bar.\n",
  );

  spoken.length = 0;
  const again = await narrate(["Open the plan.", "Drag a bar, then undo."], opts);
  check("a re-record speaks only the changed line", spoken, ["Drag a bar, then undo."]);
  check("the unchanged line comes from the cache", again.map((c) => c.cached), [true, false]);

  spoken.length = 0;
  await narrate(["Open the plan."], { ...opts, voiceId: "fake-2" });
  check("a new voice id misses the cache", spoken, ["Open the plan."]);

  const voiceFile = join(dir, "voice.ts");
  await writeFile(voiceFile, "export default { id: 'cli', voice: async (t: string) => ({ bytes: new Uint8Array([1]), seconds: 2 }) };\n");
  await writeFile(join(dir, "script.txt"), "One.\nTwo.\n");
  const cli = join(dirname(fileURLToPath(import.meta.url)), "narrate.ts");
  const run = (): string => execFileSync(process.execPath, [cli, join(dir, "script.txt"), join(dir, "out"), voiceFile], { encoding: "utf8" });
  check("the CLI speaks each line", run().split("\n").map((l) => l.split(" ")[0]), ["spoken", "spoken", ""]);
  check("a second CLI run is all cached", run().split("\n").map((l) => l.split(" ")[0]), ["cached", "cached", ""]);
  check("the CLI writes the captions", (await readFile(join(dir, "out", "narration.vtt"), "utf8")).includes("00:00:02.000 --> 00:00:04.000\nTwo."), true);
} finally {
  await rm(dir, { recursive: true, force: true });
}

if (failed) process.exit(1);
console.log("test-narrate: ok");
