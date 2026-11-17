import { readFileSync } from "node:fs";

// Where the built samples are, and the one way the specs read their `samples.json` (gs#561). Without this a missing
// file made `samples.spec.ts` throw a bare ENOENT while Playwright loaded it, which names no cause and no fix.

/** The directory `scripts/build-samples.mjs` writes to. */
export const SAMPLES_DIR = new URL("../public/samples/", import.meta.url);

/** Read and parse `samples.json` from `dir`, or throw an error that names the script that writes it. */
export function readSamplesJson<T>(dir: URL = SAMPLES_DIR): T {
  const file = new URL("samples.json", dir);
  let text: string;
  try {
    text = readFileSync(file, "utf8");
  } catch (err) {
    throw new Error(
      `the samples are not built: cannot read ${file.pathname} (${err instanceof Error ? err.message : String(err)}). ` +
        "Run `node apps/hosted/scripts/build-samples.mjs` first.",
    );
  }
  try {
    return JSON.parse(text) as T;
  } catch (err) {
    throw new Error(
      `${file.pathname} is not valid JSON (${err instanceof Error ? err.message : String(err)}). ` +
        "Run `node apps/hosted/scripts/build-samples.mjs` again.",
    );
  }
}
