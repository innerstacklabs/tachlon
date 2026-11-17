// The Content-Security-Policy a page needs for the engine to load: where `doctor` finds a policy (the project's config, a served page) and whether it lets the engine compile and connect.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

/** A policy and where it was read: `source` names the file or the response, for the sentence that says what to change. */
export interface Policy {
  source: string;
  text: string;
}

type Kind = "string" | "headers" | "html";
/** The files that carry a policy in the frameworks and hosts the command line knows, and how each holds it: Next's headers, Vercel and Netlify (a string), Netlify's `_headers` (a line), a page (a meta tag). */
const CONFIG_FILES: [string, Kind][] = [["next.config.js", "string"], ["next.config.mjs", "string"], ["next.config.cjs", "string"], ["next.config.ts", "string"], ["vercel.json", "string"], ["netlify.toml", "string"], ["public/_headers", "headers"], ["index.html", "html"], ["public/index.html", "html"]];
/** A string that holds (a piece of) a policy: it names a `*-src` directive. The key beside it does not matter, so a policy kept in a constant is found too. Only a template literal runs over lines. */
const STRING = /(["'])((?:(?!\1)[^\n])*?\b[a-z]+-src\s(?:(?!\1)[^\n])*)\1|`([^`]*?\b[a-z]+-src\s[^`]*)`/g;
/** A policy's header key in a config file; group 1 is set for the report-only one. */
const HEADER_KEY = /Content-Security-Policy(-Report-Only)?/gi;
const HEADERS_LINE = /^[ \t]+Content-Security-Policy:[ \t]*(.+)$/gim;
const META = /<meta\b[^>]*?http-?equiv=["']Content-Security-Policy["'][^>]*>/gi;
const flat = (s: string): string => s.replace(/\s+/g, " ").trim();

/** The policies in the `<meta http-equiv>` tags of `html`. */
export function metaPolicies(html: string, source: string): Policy[] {
  return [...html.matchAll(META)].flatMap(([tag]) => {
    const content = /\bcontent=(["'])(.*?)\1/is.exec(tag);
    return content ? [{ source, text: flat(content[2]) }] : [];
  });
}

/** The policies the project's config files hold as an enforced header or meta tag; a report-only one blocks nothing and is skipped. */
export function configPolicies(dir: string): Policy[] {
  return CONFIG_FILES.filter(([f]) => existsSync(join(dir, f))).flatMap(([file, kind]) => {
    const text = readFileSync(join(dir, file), "utf8");
    if (kind === "html") return metaPolicies(text, file);
    if (kind === "headers") return [...text.matchAll(HEADERS_LINE)].map((m) => ({ source: file, text: flat(m[1]) }));
    // The strings that belong to one header key are the pieces of one policy (an array joined with "; ", strings added across lines): each string belongs to the last key before it, and one before the first key (a constant) to the first. A file with no key is one policy. A report-only key owns a policy that is skipped.
    const keys = [...text.matchAll(HEADER_KEY)];
    const owned = new Map<number, string[]>();
    for (const m of text.matchAll(STRING)) {
      const key = Math.max(0, keys.filter((k) => k.index < m.index).length - 1);
      owned.set(key, [...(owned.get(key) ?? []), flat(m[2] ?? m[3])]);
    }
    return [...owned].filter(([key]) => !keys[key]?.[1]).map(([, pieces]) => ({ source: file, text: pieces.join("; ") }));
  });
}

/** The source list `policy` sets for `directive`, else for `default-src`; undefined when it sets neither (the policy puts no limit on it). */
function sources(policy: string, directive: string): string[] | undefined {
  const set = new Map<string, string[]>();
  for (const part of policy.split(";")) {
    const [name, ...list] = part.trim().split(/\s+/);
    if (name && !set.has(name.toLowerCase())) set.set(name.toLowerCase(), list);
  }
  return set.get(directive) ?? set.get("default-src");
}

/** Whether the browser may compile WebAssembly under `policy`: `script-src` (else `default-src`) holds `'wasm-unsafe-eval'` or `'unsafe-eval'`, or the policy sets neither. */
export function allowsWasm(policy: string): boolean {
  const list = sources(policy, "script-src");
  return list === undefined || list.some((s) => ["'wasm-unsafe-eval'", "'unsafe-eval'"].includes(s.toLowerCase()));
}

/** The directive that decides whether `policy` lets the engine compile, and so the one to change: `script-src` when the policy sets one, else `default-src`. */
export function wasmDirective(policy: string): "script-src" | "default-src" {
  return policy.split(";").some((part) => part.trim().split(/\s+/)[0]?.toLowerCase() === "script-src") ? "script-src" : "default-src";
}

/** Whether `connect-src` (else `default-src`) lets a page at `page` request `target`, or the policy sets neither. */
export function allowsConnect(policy: string, target: URL, page: URL): boolean {
  const list = sources(policy, "connect-src");
  return list === undefined || list.some((s) => matches(s.toLowerCase(), target, page));
}

/** One source expression of a source list against `target`: `*`, `'self'`, a scheme (`https:`), or a host with an optional scheme, `*.` prefix and port. */
function matches(source: string, target: URL, page: URL): boolean {
  if (source === "*") return true;
  if (source === "'self'") return target.origin === page.origin;
  if (source.endsWith(":")) return target.protocol === source;
  const m = /^(?:([a-z][a-z0-9+.-]*):\/\/)?(\*\.)?([^/:*]+)(?::(\d+|\*))?(?:\/.*)?$/.exec(source);
  if (!m) return false;
  const [, scheme, wild, host, port] = m;
  if (scheme !== undefined ? `${scheme}:` !== target.protocol : target.protocol !== page.protocol && target.protocol !== "https:") return false;
  if (port === undefined ? target.port !== "" : port !== "*" && port !== target.port) return false;
  return wild ? target.hostname.endsWith(`.${host}`) : target.hostname === host;
}
