import type { Status, TaskDetails } from "./api.ts"; // `.ts`, so the Node tier can load this file
import { element } from "./dom.ts";
import { say } from "./messages.ts";
import type { Shell } from "./shell.ts";

/** What a screen reader hears for a task: its name and dates, and for a leaf its float, critical state, percent complete when set, link counts and baseline start when set. Alt+Enter writes the same sentence. */
export function barSentence(root: object, d: TaskDetails): string {
  const parts = [say(root, "bar-dates", { name: d.name, start: d.start, finish: d.finish })];
  if (!d.summary) {
    parts.push(say(root, "bar-float", { float: d.floatText }), say(root, "bar-critical", { value: say(root, d.critical ? "bar-yes" : "bar-no") }));
    if (d.percentComplete !== "") parts.push(say(root, "bar-percent", { percent: d.percentComplete }));
    parts.push(say(root, "bar-predecessors", { count: d.predecessorCount }), say(root, "bar-successors", { count: d.successorCount }));
    if (d.baselineStart !== undefined) parts.push(say(root, "bar-baseline", { start: d.baselineStart }));
  }
  return parts.join(", ");
}

/**
 * Says the selected task through the `card-live` region when the selection moves to another task, a click as much as a key. The
 * returned function takes each frame's status and reads `task_details` once per change, so a frame with the same selection costs nothing.
 */
export function createBarSpeech({ app, root }: Pick<Shell, "app" | "root">): (s: Status) => void {
  const live = element<HTMLDivElement>(root, "card-live");
  let spoken: number | null = null;
  return (s) => {
    const task = s.selected?.task ?? null;
    if (task === spoken) return;
    spoken = task;
    if (task !== null) live.textContent = barSentence(root, app.task_details(task) as TaskDetails);
  };
}
