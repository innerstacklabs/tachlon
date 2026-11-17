import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// The preview port. GANTT_PORT wins. CI keeps 4173 (its job sets GANTT_PORT anyway). Anywhere else
// each checkout gets its own port from a hash of its root, 4400 to 4899, so a local run does not
// meet the vite preview of the CI runner or of another worktree on 4173 (gs#378). Two checkouts can
// still hash to the same port; playwright.config.ts then refuses to reuse the server it finds.
export function previewPort(env: NodeJS.ProcessEnv, checkoutRoot: string): number {
  if (env.GANTT_PORT) return Number(env.GANTT_PORT);
  if (env.CI) return 4173;
  const hash = createHash("sha256").update(checkoutRoot).digest().readUInt32BE(0);
  return 4400 + (hash % 500);
}

const checkoutRoot = fileURLToPath(new URL("../../..", import.meta.url));
export const port = previewPort(process.env, checkoutRoot);
export const url = process.env.GANTT_URL ?? `http://127.0.0.1:${port}/`;
export const backend = process.env.BACKEND ?? "auto";
export const dpr = Number(process.env.DPR ?? 1);
export const n = Number(process.env.N ?? 50000);
// The frame count at which a spec that opts in destroys the WebGPU device to force a GPU device loss (gs#1159); 0 leaves the device alone.
export const loseDeviceFrame = Number(process.env.LOSE_DEVICE_AT_FRAME ?? 0);
// How long that spec holds the WebGL2 fallback page back, in milliseconds (a slow runner's reload).
export const loseDeviceHoldMs = Number(process.env.LOSE_DEVICE_HOLD_MS ?? 0);
const chrome = process.env.CHROME;

export function chromePath(): string {
  if (chrome) return chrome;
  const base = join(homedir(), ".cache", "ms-playwright");
  const dir = readdirSync(base)
    .filter((entry) => /^chromium-\d+$/.test(entry))
    .sort((left, right) => Number(left.slice("chromium-".length)) - Number(right.slice("chromium-".length)))
    .at(-1);
  if (!dir) throw new Error(`no Chromium under ${base}; set CHROME`);
  return join(base, dir, "chrome-linux64", "chrome");
}
