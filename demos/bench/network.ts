// A slow link for the driver (`--network <name>`): the page's requests run with the profile's latency and
// speed, and its files come brotli-compressed as a real host serves them, the same for every side.

import { createServer, request, type OutgoingHttpHeaders } from "node:http";
import { brotliCompress, constants } from "node:zlib";

/** Latency in ms and speeds in bytes per second, as `Network.emulateNetworkConditions` takes them. */
export const NETWORK_PROFILES = {
  // Chrome DevTools' "Fast 4G": 9 Mbps down and 1.5 Mbps up (both at 0.9) and 60 ms round trip (at 2.75).
  "fast-4g": { latency: 165, downloadThroughput: 1_012_500, uploadThroughput: 168_750 },
  // WebPageTest's "Cable" (DevTools has no cable preset): 5 Mbps down, 1 Mbps up, 28 ms round trip.
  cable: { latency: 28, downloadThroughput: 625_000, uploadThroughput: 125_000 },
} as const;
export type NetworkName = keyof typeof NETWORK_PROFILES;
export const isNetworkName = (name: string): name is NetworkName => Object.hasOwn(NETWORK_PROFILES, name);

/** Throttle one page's CDP session; call it before the navigation. */
export async function applyNetwork(cdp: { send(method: string, params?: object): Promise<unknown> }, name: NetworkName): Promise<void> {
  await cdp.send("Network.enable");
  await cdp.send("Network.emulateNetworkConditions", { offline: false, ...NETWORK_PROFILES[name] });
}

const COMPRESSIBLE = /^(text\/|application\/(javascript|json|wasm|xml)|image\/svg)/;
/** What a host that precompresses sends (6 MB wasm: 1.61 MB, against 1.83 MB at 9). It takes 14 s, so it is done before the measured navigation (`warmUp`). */
export const BROTLI_QUALITY = 11;

/** Below this a file costs nothing to compress, so one asked for after the warm-up (the side's dataset hash, read after the load) is not a finding. */
const LATE_BYTES = 65_536;

type Reply = { status: number; headers: OutgoingHttpHeaders; body: Buffer };
export interface Proxy {
  url: string;
  close: () => void;
  /** Files over `LATE_BYTES` first asked for after `freeze()`, so compressed during the measured run: a file the warm-up never asked for. */
  late: string[];
  freeze: () => void;
}

/** `Vary` with `Accept-Encoding` added: the origin's tokens kept, and `*` left alone. */
const withVary = (vary: string | undefined) => (!vary ? "accept-encoding" : vary.trim() === "*" || /(^|[\s,])accept-encoding\s*(,|$)/i.test(vary) ? vary : `${vary}, accept-encoding`);

/** A server in front of `origin` that answers each request with the origin's reply, brotli-compressed when it is text, script, JSON, wasm or SVG. Every reply is made once, kept by path, and sent from memory. */
export function brotliProxy(origin: string): Promise<Proxy> {
  const target = new URL(origin);
  const replies = new Map<string, Promise<Reply>>();
  const late: string[] = [];
  let frozen = false;
  const make = (method: string, path: string, after = frozen) =>
    new Promise<Reply>((resolve, reject) => {
      request({ host: target.hostname, port: target.port, path, method }, (r) => {
        const chunks: Buffer[] = [];
        r.on("data", (c: Buffer) => chunks.push(c)).on("error", reject).on("end", () => {
          const raw = Buffer.concat(chunks);
          const status = r.statusCode ?? 502;
          const { "content-length": _l, "transfer-encoding": _t, connection: _c, "keep-alive": _k, ...kept } = r.headers;
          if (method !== "GET" || r.headers["content-encoding"] !== undefined || !COMPRESSIBLE.test(r.headers["content-type"] ?? "")) return resolve({ status, headers: method === "GET" ? { ...kept, "content-length": raw.length } : kept, body: raw });
          if (after && raw.length > LATE_BYTES) late.push(`${method} ${path}`);
          brotliCompress(raw, { params: { [constants.BROTLI_PARAM_QUALITY]: BROTLI_QUALITY, [constants.BROTLI_PARAM_SIZE_HINT]: raw.length } }, (e, body) =>
            e ? reject(e) : resolve({ status, headers: { ...kept, "content-length": body.length, "content-encoding": "br", vary: withVary(r.headers.vary) }, body }));
        });
      }).on("error", reject).end();
    });
  const server = createServer((req, res) => {
    const key = `${req.method} ${req.url}`;
    if (!replies.has(key)) replies.set(key, make(req.method ?? "GET", req.url ?? "/"));
    replies.get(key)!.then(({ status, headers, body }) => res.writeHead(status, headers).end(body), () => res.writeHead(502).end());
  });
  return new Promise((resolve) =>
    server.listen(0, "127.0.0.1", () => resolve({ url: `http://127.0.0.1:${(server.address() as { port: number }).port}/`, close: () => (server.close(), server.closeAllConnections()), late, freeze: () => void (frozen = true) })),
  );
}

/** Load the page once in a throwaway browser, with no throttle, so each file it asks for is compressed and kept before the measured navigation, which then runs in a fresh browser. */
export async function warmUp(proxy: Proxy, launch: () => Promise<any>, url: string): Promise<void> {
  const browser = await launch();
  try {
    await (await browser.newPage()).goto(url, { waitUntil: "networkidle", timeout: 180_000 }).catch(() => undefined);
  } finally {
    await browser.close().catch(() => undefined);
    proxy.freeze();
  }
}
