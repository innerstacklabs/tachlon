import { type TaskDetails } from "./api.js";
import { reason } from "./boot.js";
import { element, optional } from "./dom.js";
import { type Shell } from "./shell.js";
import "./styles/task-dialog.css";

export interface TaskDialog {
  /**
   * Open the dialog on `task`, its fields as the table writes them (gantt#1875). False, doing nothing, while a press is in
   * progress, the dialog is already open, or the task is not there.
   */
  open: (task: number) => boolean;
}

/** The text of the task's fields, under the keys `App.set_task_fields` takes. A date field holds the day, as the cell editor does, so an imported time of day is not cut to midnight. */
function read(app: Shell["app"], task: number, details: TaskDetails): Record<string, string> {
  return {
    name: app.task_label(task),
    duration: app.task_duration(task),
    start: details.start.slice(0, 10),
    finish: details.finish.slice(0, 10),
    percent: app.task_percent(task),
    actual_start: details.actualStart.slice(0, 10),
    actual_finish: details.actualFinish.slice(0, 10),
    remaining: app.task_remaining(task),
    snlt: app.task_limit(task, "snlt"),
    fnet: app.task_limit(task, "fnet"),
    fnlt: app.task_limit(task, "fnlt"),
    predecessors: app.predecessors_text(task) ?? "",
  };
}

/**
 * The task dialog (gantt#1875): `Shift+Enter` on the selected task opens a modal `<dialog>` holding its name, duration, start,
 * finish, percent complete, actual dates, remaining, start no later than, finish no earlier than, finish no later than and, while tasks have Activity IDs, predecessors. Save
 * (or `Enter` in a field) sends the fields that changed through
 * `App.set_task_fields`, which applies them as one undo step; a refusal shows its reason, beginning with the field's name, in
 * the dialog and keeps it open with nothing changed. While the chart is read-only (`refuseReadOnly` says so and shows why) it does not open. Cancel and `Escape` close it and change nothing. It reads and writes only
 * when it opens and when Save is pressed, never per frame. The chart's keys skip a key inside a `<dialog>` (keys.ts).
 */
export function installTaskDialog({ app, root, canvas, status }: Shell, refuseReadOnly: () => boolean): TaskDialog {
  const dialog = optional<HTMLDialogElement>(root, "task-dialog");
  if (dialog === null) return { open: () => false };
  const form = element<HTMLFormElement>(dialog, "task-form");
  const title = element<HTMLElement>(dialog, "task-dialog-title");
  const error = element<HTMLElement>(dialog, "task-dialog-error");
  const inputs = [...form.querySelectorAll<HTMLInputElement>("input[name]")];
  // What the dialog opened with: the task, the file's epoch (another file opened since has other tasks under the same ids) and the fields' texts.
  let held: { task: number; epoch: number; text: Record<string, string> } | null = null;
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    if (held === null) return;
    const { task, epoch, text } = held;
    const changed = Object.fromEntries(inputs.filter((input) => input.value !== text[input.name]).map((input) => [input.name, input.value]));
    if (status().epoch === epoch && Object.keys(changed).length > 0) {
      try {
        app.set_task_fields(task, changed);
      } catch (err) {
        error.textContent = reason(err);
        error.hidden = false;
        return;
      }
    }
    dialog.close();
  });
  element<HTMLButtonElement>(dialog, "task-cancel").addEventListener("click", () => dialog.close());
  // Every way out (Save, Cancel, Escape) ends here: nothing is held and the chart has focus again. The browser has already put
  // focus back on the chart when `close` fires (a task later), and a key pressed in between may have opened a cell editor: its input keeps the focus.
  dialog.addEventListener("close", () => {
    held = null;
    if (!(document.activeElement instanceof HTMLInputElement && root.contains(document.activeElement))) root.focus({ preventScroll: true });
  });
  for (const input of inputs) input.addEventListener("input", () => (error.hidden = true));
  const open: TaskDialog["open"] = (task) => {
    const details = app.task_details(task) as TaskDetails | undefined;
    if (dialog.open || status().gesture !== "idle" || details === undefined || refuseReadOnly()) return false;
    const text = read(app, task, details);
    for (const input of inputs) input.value = text[input.name] ?? "";
    // Predecessors are Activity IDs, so the field is there only while the tasks have them, and not for a summary or a task whose list holds a task with no ID: a Save would drop that link.
    const showPred = app.has_codes() && details.summary !== true && app.predecessors_text(task) !== undefined;
    for (const input of inputs.filter((i) => i.name === "predecessors")) input.closest("label")!.style.display = showPred ? "" : "none";
    held = { task, epoch: status().epoch, text };
    title.textContent = `Edit task: ${text.name}`;
    error.hidden = true;
    dialog.showModal();
    return true;
  };
  // A double-click on a bar opens the dialog for its task; the hover is the bar under the pointer, read when the click lands.
  canvas.addEventListener("dblclick", () => {
    const task = status().hover?.task;
    if (task !== undefined) open(task);
  });
  return { open };
}
