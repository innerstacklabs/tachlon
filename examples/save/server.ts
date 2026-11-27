// A fake host server for the save example (decision 0022, rule 11.1). It keeps its rows in memory, makes them from a seed, speaks the
// first section's convention ("A backend you are writing now": routes, answers, the refusal, `Idempotency-Key`) and misbehaves as its script says.
// It is also a reference backend: a coding assistant can read it to see what a server of the convention does.
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

type Row = Record<string, unknown>;
type Answer = { status: number; body?: unknown };
/** One request as the script sees it; `n` counts the requests the server has received, from 1. `key` is the decoded `:key` of the path. `repeat`: an earlier request carried the same `Idempotency-Key`. */
export interface Seen { n: number; method: string; path: string; key?: string; once?: string; repeat: boolean; body?: Row }
export interface Db { tasks: Map<string, Row>; links: Map<string, Row>; schedule: Row }

export interface Script {
  keys?: "chart" | "server"; // who makes the key of a new row: the body of a create (default), or this server
  names?: "chart" | "host"; // the member names rows carry: the chart's (`name`) or the host's own (`title`, the default)
  seed?: number;
  latency?: number | ((r: Seen) => number); // milliseconds before the request is applied
  refuse?: (r: Seen) => Answer | undefined; // which request to refuse, with what status and body (a string body is sent as text, not JSON)
  fail?: (r: Seen) => number | undefined; // when to answer a 401, a 500 or any status that judges nothing: nothing is applied, no answer is stored
  outside?: (r: Seen, db: Db) => void; // when to change rows from outside, as the request arrives
  push?: (r: Seen) => { after: number; tasks: Row[] } | undefined; // when to push a late result to the listeners of GET /api/events
  stale?: (r: Seen) => boolean; // answer a read with the rows as they were when it arrived, however late the answer is sent (an old read)
  changed?: (r: Seen, db: Db) => Row[] | undefined; // rows the server changes beyond a change set it applies (host names, by `id`); the answer returns them in `changed`
  drop?: (r: Seen) => "request" | "response" | undefined; // close the connection before applying the request, or after applying and storing it
  honor?: boolean; // answer a repeated idempotency key as the first request was (default true)
}

/** `script` is the one the server reads at every request, so a scenario changes it as the person acts (the server comes back, the session is accepted again). */
export interface FakeServer { url: string; seen: Seen[]; db: Db; script: Script; maxInFlight: number; close(): Promise<void> }

const TO_CHART = {
  task: { id: "key", title: "name", plannedStart: "start", days: "duration", parentId: "parent" },
  link: { id: "key", source: "from", target: "to" },
};
type Kind = keyof typeof TO_CHART;
const KIND: Record<string, Kind> = { tasks: "task", links: "link" };
const METHOD = { create: "POST", update: "PATCH", delete: "DELETE" } as const;
const rename = (row: Row, map: Record<string, string>): Row => Object.fromEntries(Object.entries(row).map(([k, v]) => [map[k] ?? k, v]));
const invert = (map: Record<string, string>): Record<string, string> => Object.fromEntries(Object.entries(map).map(([k, v]) => [v, k]));
const day = (n: number): string => new Date(Date.UTC(2026, 10, 2 + n)).toISOString().slice(0, 10);

export async function startServer(script: Script = {}): Promise<FakeServer> {
  const db: Db = { tasks: new Map(), links: new Map(), schedule: { name: "Launch plan" } };
  let s = script.seed ?? 1;
  const rand = (n: number): number => 1 + (((s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32) * n | 0);
  let at = 0;
  for (const [i, title] of ["Design", "Build", "Ship"].entries()) {
    const days = rand(5);
    db.tasks.set(`t${i + 1}`, { id: `t${i + 1}`, title, plannedStart: day(at), days });
    if (i > 0) db.links.set(`l${i}`, { id: `l${i}`, source: `t${i}`, target: `t${i + 1}` });
    at += days;
  }
  // In the chart's names a task also carries `id`, the handle that `parent`, `from` and `to` name (decision 0017, "A task").
  const view = (kind: Kind, row: Row): Row => (script.names === "chart" ? { ...(kind === "task" && { id: row.id }), ...rename(row, TO_CHART[kind]) } : row);
  const parse = (kind: Kind, row: Row): Row => (script.names === "chart" ? rename(row, invert(TO_CHART[kind])) : row);
  const stored = new Map<string, Answer>(); // the answers of applied requests, by Idempotency-Key
  const listeners = new Set<ServerResponse>();
  const seen: Seen[] = [];
  let made = 0;
  let inFlight = 0;
  const out: FakeServer = { url: "", seen, db, script, maxInFlight: 0, close: async () => void 0 };

  const apply = (r: Seen): Answer => {
    const what = r.path.split("/")[2] ?? "";
    if (what === "schedule" && r.method === "GET") {
      const rows = (t: Map<string, Row>, k: Kind): Row[] => [...t.values()].map((row) => view(k, row));
      return { status: 200, body: { ...db.schedule, tasks: rows(db.tasks, "task"), links: rows(db.links, "link") } };
    }
    if (what === "schedule") return (Object.assign(db.schedule, r.body), { status: 204 });
    if (what === "changes") return changes(r);
    const kind = KIND[what];
    if (kind === undefined) return { status: 404, body: { message: `No route ${r.path}` } };
    const table = what === "tasks" ? db.tasks : db.links;
    if (r.method === "POST") {
      const row = parse(kind, r.body ?? {});
      const key = script.keys === "server" && row.id === undefined ? `s${++made}` : row.id; // a row that comes back (an undo) carries the key it had
      if (typeof key !== "string") return { status: 400, body: { message: "A new row needs a key" } };
      if (table.has(key)) return { status: 409, body: { message: "A row with that key exists", key } };
      // The chart reads a row back with no `null` member and no calendar without a key (`{ calendar: n }`); it sends no start, so a new task starts where the stored days end.
      const sent = Object.fromEntries(Object.entries(row).filter(([k, v]) => v !== null && !(k === "calendar" && typeof v === "object")));
      const start = kind === "task" && sent.plannedStart === undefined ? { plannedStart: day([...table.values()].reduce((n, t) => n + Number(t.days), 0)) } : {};
      table.set(key, { ...start, ...sent, id: key });
      return { status: 201, body: view(kind, table.get(key)!) };
    }
    const row = table.get(r.key ?? "");
    if (row === undefined) return { status: 404, body: { message: "No such row", key: r.key } };
    if (r.method === "DELETE") return (table.delete(r.key!), { status: 204 });
    return (Object.assign(row, parse(kind, r.body ?? {})), { status: 204 });
  };

  // The change-set endpoint beside the per-record routes (rule 11.1): one transaction, answered with the keys it made for the creates that carry a `ref`.
  const changes = (r: Seen): Answer => {
    const before = structuredClone(db);
    const keys: { ref: string; key: string }[] = [];
    const swap = (v: unknown): any => (v !== null && typeof v === "object" && "ref" in v ? keys.find((k) => k.ref === v.ref)?.key : v);
    for (const c of (r.body as { changes: Row[] }).changes) {
      const key = swap(c.key) as string | null;
      const path = c.record === "schedule" ? "/api/schedule" : `/api/${c.record}s${c.op === "create" ? "" : `/${encodeURIComponent(key!)}`}`;
      const set = c.set && Object.fromEntries(Object.entries(c.set).map(([k, v]) => [k, swap(v)]));
      const answer = apply({ ...r, method: METHOD[c.op as keyof typeof METHOD], path, key: key ?? undefined, body: set });
      if (answer.status >= 300) return (Object.assign(db, before), answer);
      if (typeof c.ref === "string") keys.push({ ref: c.ref, key: typeof c.key === "string" ? c.key : `s${made}` });
    }
    const more = script.changed?.(r, db);
    for (const row of more ?? []) Object.assign(db.tasks.get(row.id as string) ?? {}, row);
    return { status: 200, body: more ? { keys, changed: { tasks: more } } : { keys } };
  };

  const handle = async (req: IncomingMessage, res: ServerResponse): Promise<void> => {
    const url = new URL(req.url ?? "/", "http://fake");
    if (url.pathname === "/api/events") return void (res.writeHead(200, { "content-type": "text/event-stream" }), listeners.add(res), res.on("close", () => listeners.delete(res)));
    let text = "";
    for await (const chunk of req) text += chunk;
    const part = url.pathname.split("/")[3];
    const once = req.headers["idempotency-key"] as string | undefined;
    const r: Seen = { n: seen.length + 1, method: req.method ?? "GET", path: url.pathname, key: part && decodeURIComponent(part), once, repeat: once !== undefined && seen.some((p) => p.once === once), body: text ? JSON.parse(text) : undefined };
    seen.push(r);
    out.maxInFlight = Math.max(out.maxInFlight, ++inFlight);
    res.on("close", () => inFlight--);
    script.outside?.(r, db);
    const late = script.push?.(r);
    if (late) setTimeout(() => listeners.forEach((l) => l.write(`data: ${JSON.stringify({ tasks: late.tasks })}\n\n`)), late.after);
    const early = script.stale?.(r) ? apply(r) : undefined;
    const wait = typeof script.latency === "function" ? script.latency(r) : script.latency ?? 0;
    await new Promise((resolve) => setTimeout(resolve, wait));
    const drop = script.drop?.(r);
    if (drop === "request") return void req.socket.destroy();
    const kept = script.honor === false || r.once === undefined ? undefined : stored.get(r.once);
    const status = script.fail?.(r);
    const answer: Answer = kept ?? (status !== undefined ? { status, body: { message: `The server answered ${status}` } } : script.refuse?.(r) ?? early ?? apply(r));
    if (kept === undefined && status === undefined && answer.status < 300 && r.once !== undefined) stored.set(r.once, answer);
    if (drop === "response") return void req.socket.destroy();
    const { body } = answer;
    res.writeHead(answer.status, body === undefined ? {} : { "content-type": typeof body === "string" ? "text/plain" : "application/json" });
    res.end(body === undefined || typeof body === "string" ? body : JSON.stringify(body));
  };

  const server = createServer((req, res) => handle(req, res).catch((e) => (console.error(e), res.destroy())));
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  out.url = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  out.close = () => new Promise((resolve) => (server.closeAllConnections(), server.close(() => resolve())));
  return out;
}
