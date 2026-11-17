// What `doctor --url <page>` reads from the network: the page's policy, and the engine at the address the page names (in its HTML or a module script it loads), or `/app_bg.wasm`, where `init` puts it in a Next app.
// Only this file touches the network; `doctor.ts` turns what it returns into findings.
import { Refusal } from "./detect.ts";
import { metaPolicies, type Policy } from "./csp.ts";

/** What the engine address answered: `status` is null when the request failed (`error` says why). */
export interface EngineAnswer {
  url: string;
  status: number | null;
  type: string;
  bytes: Buffer;
  error: string;
  /** True when the page's HTML names this address; false for `/app_bg.wasm`, the address only a Next app is known to use. */
  named: boolean;
}

export interface Probe {
  page: string;
  /** The policy in the response header and in the page's meta tags. */
  policies: Policy[];
  engine: EngineAnswer;
}

const TIMEOUT_MS = 15_000;
/** A `.wasm` address the page's HTML names, such as a preload link; the first one wins. */
const WASM_REF = /["'(=\s]([^"'()\s<>=]+\.wasm)(?:[?#][^"'()\s<>]*)?(?=["')\s>])/;
/** A preload link (or module preload): what the page loads first, so its `.wasm` is the engine before any other `.wasm` the HTML mentions. */
const PRELOAD = /<link\b[^>]*\brel=["']?(?:module)?preload\b[^>]*>/gi;
/** A module script the page loads: the `src` of `<script type="module" src="...">`. */
const MODULE_SCRIPT = /<script\b(?=[^>]*\stype=["']?module\b)[^>]*\ssrc=["']?([^"'\s>]+)/gi;
/** The most module scripts read for an engine address: a Vite build has an entry script and a few chunks, never dozens. */
const MAX_SCRIPTS = 5;
/** The error's message with its cause's: `fetch failed` alone does not say refused, not found or timed out. */
const why = (e: unknown): string => (e instanceof Error ? `${e.message}${e.cause instanceof Error ? ` (${e.cause.message})` : ""}` : String(e)).replace(/\s+/g, " ");

/** The first `.wasm` address (made absolute against the script that names it) in the page's module scripts on the page's own origin, at most `MAX_SCRIPTS` of them: a Vite build names its hashed engine file only there. A script that cannot be fetched names none. */
async function wasmInScripts(html: string, page: string): Promise<string | undefined> {
  const scripts = [...html.matchAll(MODULE_SCRIPT)].map(([, src]) => new URL(src, page)).filter((u) => u.origin === new URL(page).origin).slice(0, MAX_SCRIPTS);
  for (const script of scripts) {
    try {
      const ref = WASM_REF.exec(await (await fetch(script, { signal: AbortSignal.timeout(TIMEOUT_MS) })).text())?.[1];
      if (ref) return new URL(ref, script).href;
    } catch {
      /* a script that cannot be read names no engine */
    }
  }
  return undefined;
}

/** Fetches `page` and its engine. A page that cannot be fetched is a refusal (exit 2); an engine that cannot be is an answer with no status, which `doctor` reports as a finding. */
export async function probePage(page: string): Promise<Probe> {
  let res: Response;
  let html: string;
  try {
    res = await fetch(page, { signal: AbortSignal.timeout(TIMEOUT_MS) });
    html = await res.text();
  } catch (e) {
    throw new Refusal(`${page} could not be fetched: ${why(e)}`);
  }
  if (res.status !== 200) throw new Refusal(`${page} answered with status ${res.status}, not 200; pass the address of the page that mounts the chart`);
  const source = `the response of ${page}`;
  const header = (res.headers.get("content-security-policy") ?? "").split(",").map((t) => t.trim()).filter(Boolean);
  const ref = [...html.matchAll(PRELOAD)].map(([tag]) => WASM_REF.exec(tag)?.[1]).find(Boolean) ?? WASM_REF.exec(html)?.[1];
  const named = ref ? new URL(ref, res.url).href : await wasmInScripts(html, res.url);
  const url = named ?? new URL("/app_bg.wasm", res.url).href;
  const policies = [...header.map((text) => ({ source, text })), ...metaPolicies(html, source)];
  try {
    const got = await fetch(url, { signal: AbortSignal.timeout(TIMEOUT_MS) });
    return { page: res.url, policies, engine: { url, status: got.status, type: got.headers.get("content-type") ?? "", bytes: Buffer.from(await got.arrayBuffer()), error: "", named: named !== undefined } };
  } catch (e) {
    return { page: res.url, policies, engine: { url, status: null, type: "", bytes: Buffer.alloc(0), error: why(e), named: named !== undefined } };
  }
}
