// What a failed start tells a person or a coding assistant (tl#1249): one stable `code` for each cause `mount` can name, and for each
// code one `fix`, a sentence to act on. The code is the product name in capitals and the cause (`<NAME>_ENGINE_FETCH`), read from
// `PRODUCT_NAME`, so a rename is that file. This file is pure: no DOM, no wasm. `mount-load.ts` probes the page and asks it what to say.
import { PRODUCT_NAME } from "./product.ts"; // `.ts`, so the Node tier can load this file as it loads the adapters

/** Every cause `mount` names, with the sentence that says what to do. */
const FIXES = {
  DATA_INVALID: "Correct the fields `message` lists and call `mount` again; `validate(data)` checks plain task data against the same rules without mounting.",
  ENGINE_FETCH: "Serve the engine file (`app_bg.wasm`, the file the chart's script names with `new URL`) from the address the page asks for, with status 200; the `message` has the status.",
  ENGINE_NOT_WASM: "The address answers with something other than the engine. Serve the file `app_bg.wasm` itself, as `application/wasm`, and make a single-page fallback route skip `.wasm` files.",
  ENGINE_LOAD_FAILED: "The file is the engine, but it did not start. Rebuild so `app.js` and `app_bg.wasm` come from the same build, and read the browser console and the error in `cause` for the underlying error.",
  ENGINE_CSP: "Add `'wasm-unsafe-eval'` to the page's `script-src` (or `default-src`) in its Content-Security-Policy, so the browser may compile the engine.",
  GPU_NONE: "The browser has neither WebGPU nor WebGL2 here. Try a current Chrome, Edge, Firefox or Safari with hardware acceleration on; a headless or remote desktop session often has none.",
  FILE_UNREADABLE: "The browser could not read the `File` given as `schedule`: it was moved, deleted or is locked. Pick the file again and call `mount` again.",
  SCHEDULE_UNREADABLE: "The chart could not read the schedule as a Primavera XER file, a Microsoft Project XML file or a schedule document. Open it in the app it came from, or export it again as XER or XML, then call `mount` again.",
} as const;
export type Cause = keyof typeof FIXES;

/** A stable name for a cause: the product name in capitals, an underscore and the cause, like `<NAME>_ENGINE_FETCH`. */
export type ErrorCode = `${Uppercase<typeof PRODUCT_NAME>}_${Cause}`;

/** Where the codes are explained: the "Start errors" heading in the package's own `README.md`, which lists every code with its `fix`. */
export const ERROR_DOCS = "README.md#start-errors";

/** The code for a cause. */
export function codeFor(cause: Cause): ErrorCode {
  return `${PRODUCT_NAME.toUpperCase()}_${cause}` as ErrorCode;
}

/** The sentence that says what to do about `code`. */
export function fixFor(code: ErrorCode): string {
  return FIXES[code.slice(PRODUCT_NAME.length + 1) as Cause];
}

/** What a probe of the engine's address found: the status (`null` when the request itself threw) and the first bytes of the answer. */
export interface EngineProbe {
  status: number | null;
  head: Uint8Array;
}

const MAGIC = [0x00, 0x61, 0x73, 0x6d];

/** The code and the one line for a failed engine load, from the probe and from what `init` threw. A pure function of both. */
export function engineFailure(probe: EngineProbe, thrown: unknown): { code: ErrorCode; message: string } {
  const text = thrown instanceof Error ? thrown.message : String(thrown);
  if (probe.status === null) return { code: codeFor("ENGINE_FETCH"), message: `The chart's engine file could not be requested: ${text}` };
  if (probe.status < 200 || probe.status > 299) return { code: codeFor("ENGINE_FETCH"), message: `The chart's engine file answered with status ${probe.status}.` };
  if (!MAGIC.every((byte, i) => probe.head[i] === byte)) {
    const shown = [...probe.head].map((b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : `\\x${b.toString(16).padStart(2, "0")}`)).join("");
    return { code: codeFor("ENGINE_NOT_WASM"), message: `The chart's engine file is not WebAssembly: it starts with "${shown}", not "\\0asm".` };
  }
  if (/content security polic|blocked by CSP/i.test(text)) return { code: codeFor("ENGINE_CSP"), message: "The page's Content-Security-Policy does not let the browser compile the chart's engine." };
  return { code: codeFor("ENGINE_LOAD_FAILED"), message: `The chart's engine file is WebAssembly, but it did not start: ${text}` };
}

/** `GPU_NONE` when the browser has neither a WebGPU adapter (`gpu`) nor WebGL2 (`webgl2`, asked only when `gpu` is false), else `undefined`. */
export function graphicsCode(gpu: boolean, webgl2: () => boolean): ErrorCode | undefined {
  return gpu || webgl2() ? undefined : codeFor("GPU_NONE");
}
