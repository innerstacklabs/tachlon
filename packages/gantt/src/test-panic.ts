// Test-side hooks used by the browser verification path.
import { test_panic_at_init } from "./wasm/app.js";

/** `?panic=init`: arm the deliberate init panic (a test hook; see docs/WASM_API.md). */
export function armInitPanic(params: URLSearchParams): void {
  if (params.get("panic") === "init") test_panic_at_init();
}
