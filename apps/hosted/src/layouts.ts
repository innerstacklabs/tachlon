// Print layouts kept by name (gantt#703): the hosted page stores each one in this browser's `localStorage`, as `hidden-columns.ts` stores the hidden columns.
// The chart reads a layout and takes one back (`Commands.layoutJson`, `Commands.applyLayout`); where it is kept is the page's call, never `packages/gantt/`, which ships to npm.
import type { Commands } from "gantt-core";

type LayoutCommands = Pick<Commands, "layoutJson" | "applyLayout">;
const KEY = "gantt-layouts";

/** The layouts kept in this browser, name to the layout text the chart gave. */
function stored(): Record<string, string> {
  try {
    const kept = JSON.parse(localStorage.getItem(KEY) ?? "{}") as unknown;
    return typeof kept === "object" && kept !== null ? (kept as Record<string, string>) : {};
  } catch {
    return {}; // storage blocked or not ours: nothing is kept
  }
}

/** The names kept, in the order they were first saved. */
export function layoutNames(): string[] {
  return Object.keys(stored());
}

/** Keep the print setup on screen now under `name`, replacing a layout of that name. Returns the line the page says. */
export function saveLayout(commands: LayoutCommands, name: string): string {
  const layout = commands.layoutJson(name);
  if (typeof layout !== "string") return "The chart could not read its print layout, so nothing was saved.";
  try {
    localStorage.setItem(KEY, JSON.stringify({ ...stored(), [name]: layout }));
  } catch {
    return "This browser would not keep the layout, so nothing was saved.";
  }
  return `Saved the layout "${name}" in this browser.`;
}

/** Put the layout kept under `name` on the chart. Returns the line the page says: the parts the schedule could not take are named in it. */
export function applyLayout(commands: LayoutCommands, name: string): string {
  const layout = stored()[name];
  if (layout === undefined) return `No layout named "${name}" is kept in this browser.`;
  const result = commands.applyLayout(layout);
  if (result === undefined) return "";
  if (!result.ok) return `The layout "${name}" was not applied: ${result.message}`;
  if (result.notApplied.length === 0) return `Applied the layout "${name}".`;
  return `Applied the layout "${name}". Not applied: ${byReason(result.notApplied).join("; ")}.`;
}

const SEPARATOR = ": not applied, ";

/** The parts the engine did not apply ("column Start: not applied, the file has no such column"), the parts that share a reason listed once with it. */
function byReason(notApplied: readonly string[]): string[] {
  const parts = new Map<string, string[]>();
  for (const line of notApplied) {
    const at = line.indexOf(SEPARATOR);
    const reason = at < 0 ? "" : line.slice(at + SEPARATOR.length);
    const part = (at < 0 ? line : line.slice(0, at)).replace(/^column /, "");
    parts.set(reason, [...(parts.get(reason) ?? []), part]);
  }
  return [...parts].map(([reason, list]) => (reason === "" ? list.join(", ") : `${list.join(", ")} (${reason})`));
}
