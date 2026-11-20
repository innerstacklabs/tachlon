import { SaveRefused } from "tachlon";
import type { SaveContext } from "tachlon/react";

export type Task = { id: string; title: string; plannedStart: string; days: number; parentId?: string };
export type Link = { id: string; source: string; target: string };
type Row = Record<string, unknown>;

async function send(method: string, path: string, body?: Row, c?: SaveContext) {
  const headers: Record<string, string> = { ...(body && { "content-type": "application/json" }), ...(c && { "idempotency-key": c.once }) };
  const res = await fetch(`/api${path}`, { method, headers, signal: c?.signal, body: body && JSON.stringify(body) });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw body.field ? new SaveRefused(body) : Object.assign(new Error(body.message ?? res.statusText), { status: res.status }); // a refusal that names a field shows at that cell
  }
  return res.status === 204 ? undefined : res.json();
}
const at = (key: string) => encodeURIComponent(key);

export const api = {
  readSchedule: (): Promise<{ tasks: Task[]; links: Link[] }> => send("GET", "/schedule"),
  createTask: (row: Row, c: SaveContext) => send("POST", "/tasks", row, c),
  updateTask: (key: string, set: Row, c: SaveContext) => send("PATCH", `/tasks/${at(key)}`, set, c),
  deleteTask: (key: string, c: SaveContext) => send("DELETE", `/tasks/${at(key)}`, undefined, c),
  createLink: (row: Row, c: SaveContext) => send("POST", "/links", row, c),
  updateLink: (key: string, set: Row, c: SaveContext) => send("PATCH", `/links/${at(key)}`, set, c),
  deleteLink: (key: string, c: SaveContext) => send("DELETE", `/links/${at(key)}`, undefined, c),
  updateSchedule: (set: Row, c: SaveContext) => send("PATCH", "/schedule", set, c),
};
