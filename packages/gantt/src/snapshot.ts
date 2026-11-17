// `Gantt.snapshot` (gantt#2077): the saved document as plain data, each task's start read from the running session, which has solved it.
import { toPlain } from "./adapters/to-plain.js";
import type { PublicApp } from "./api.js";

/** The open schedule as `toPlain` returns it. `App.task_starts` lists the solved starts in the order `App.save` lists the tasks, each in one call. */
export function snapshot(app: PublicApp, name: string): ReturnType<typeof toPlain> {
  const document = JSON.parse(app.save(name)) as Parameters<typeof toPlain>[0];
  const starts = JSON.parse(app.task_starts()) as string[];
  if (starts.length !== document.tasks.length) throw new Error(`snapshot: ${document.tasks.length} tasks saved, ${starts.length} starts read`);
  return toPlain(document, starts);
}
