// The codes and the sentences of a failed start (tl#1249 Row 1): which failure an engine probe shows, and the fix each code carries.
// What a `MountError` carries and what the page rejects with is `tests/engine-error.spec.ts`.
import { type Cause, codeFor, engineFailure, ERROR_DOCS, fixFor, graphicsCode } from "../src/errors.ts";
import { PRODUCT_NAME } from "../src/product.ts";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const wasm = new Uint8Array([0, 0x61, 0x73, 0x6d]);
const html = new TextEncoder().encode("<!do");
const causes: Cause[] = ["DATA_INVALID", "ENGINE_FETCH", "ENGINE_NOT_WASM", "ENGINE_LOAD_FAILED", "ENGINE_CSP", "GPU_NONE", "FILE_UNREADABLE", "SCHEDULE_UNREADABLE"];
const code = (cause: string): string => `${PRODUCT_NAME.toUpperCase()}_${cause}`;

test("the status and the first four bytes name the engine failure", () => {
  const threw = new Error("Failed to fetch");
  expect(engineFailure({ status: null, head: new Uint8Array() }, threw)).toMatchObject({ code: code("ENGINE_FETCH"), message: expect.stringContaining("Failed to fetch") });
  expect(engineFailure({ status: 404, head: html }, threw)).toMatchObject({ code: code("ENGINE_FETCH"), message: expect.stringContaining("status 404") });
  expect(engineFailure({ status: 200, head: html }, threw)).toMatchObject({ code: code("ENGINE_NOT_WASM"), message: expect.stringContaining('"<!do"') });
  expect(engineFailure({ status: 200, head: wasm }, new Error("Compiling violates the following Content Security policy directive"))).toMatchObject({ code: code("ENGINE_CSP") });
  expect(engineFailure({ status: 200, head: wasm }, new Error("call to WebAssembly.instantiate() blocked by CSP"))).toMatchObject({ code: code("ENGINE_CSP") });
  expect(engineFailure({ status: 200, head: wasm }, new Error("LinkError: import object field is not a Function"))).toMatchObject({ code: code("ENGINE_LOAD_FAILED"), message: expect.stringContaining("did not start") });
});

test("a graphics probe asks for WebGL2 only when WebGPU is missing", () => {
  let asked = 0;
  const webgl2 = (answer: boolean) => () => (asked++, answer);
  expect([graphicsCode(true, webgl2(false)), asked]).toEqual([undefined, 0]);
  expect(graphicsCode(false, webgl2(true))).toBeUndefined();
  expect(graphicsCode(false, webgl2(false))).toBe(code("GPU_NONE"));
});

test("every cause has a code that names the product and a fix that says what to do", () => {
  for (const cause of causes) {
    expect(codeFor(cause)).toBe(code(cause));
    expect(fixFor(codeFor(cause)).length).toBeGreaterThan(40);
  }
  expect(new Set(causes.map((c) => fixFor(codeFor(c)))).size).toBe(causes.length);
});

test("the package README holds every code and its fix, under the heading `docs` names", () => {
  const readme = readFileSync(new URL("../README.md", import.meta.url), "utf8");
  expect([ERROR_DOCS, readme.includes("\n## Start errors\n")]).toEqual(["README.md#start-errors", true]);
  for (const cause of causes) expect(readme).toContain(`\`${codeFor(cause)}\` | ${fixFor(codeFor(cause))} |`);
});

test("the package README says a start failure with no known cause has no `fix` and no `code`", () => {
  const readme = readFileSync(new URL("../README.md", import.meta.url), "utf8");
  for (const words of ["its `fix` says what to do when the cause is known.", "`code`, when it has one, is one of these"]) expect(readme).toContain(words);
});
