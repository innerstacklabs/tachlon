// The size cap on a served sample (gs#511 S6, gs#561). `build-samples.mjs` imports it, and `tests/build-samples.spec.ts`
// imports it too, so the rule the build applies is the rule the test checks. Node 22.18 or later runs this file as it
// is, so it uses type annotations only.

/** A served file this size or larger fails the build: the per-file limit of a Pages deploy. */
export const MAX_BYTES = 25 * 1024 * 1024;

/** The environment variable that lowers the cap for a test. It cannot raise it. */
export const CAP_ENV = "BUILD_SAMPLES_MAX_BYTES";

/**
 * The cap this run applies: `MAX_BYTES`, or the value of `BUILD_SAMPLES_MAX_BYTES` when that is a whole number from 1
 * up to `MAX_BYTES`. A value that is not a number, or that is above `MAX_BYTES`, throws, so a typo or a CI variable
 * set too high stops the build and never raises the limit.
 */
export function capFor(env: Record<string, string | undefined> = process.env): number {
  const raw = env[CAP_ENV];
  if (raw === undefined || raw === "") return MAX_BYTES;
  const asked = /^\d+$/.test(raw) ? Number(raw) : Number.NaN;
  if (!(asked >= 1 && asked <= MAX_BYTES)) {
    throw new Error(`${CAP_ENV}=${raw} is not a whole number of bytes from 1 to ${MAX_BYTES}; it can lower the cap, not raise it`);
  }
  return asked;
}

/** Throw, naming `file`, when it is `length` bytes and that is at or over `cap`. */
export function checkSize(file: string, length: number, cap: number): void {
  if (length >= cap) throw new Error(`${file} is ${length} bytes, at or over the ${cap} byte cap`);
}
