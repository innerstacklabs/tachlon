import type { CheckKind, Gantt } from "gantt-core";
import "./checks.css";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

/** What `Gantt.checks` returns. */
type ChecksReport = ReturnType<Gantt["checks"]>;

/** One panel line: its name, value, explanations and optional Check filter choice while its count is above 0. */
export interface CheckLine {
  name: string;
  value: string;
  says: string;
  why?: string;
  show?: CheckKind;
}

/** `why` while the line has a problem to show, else nothing: a clean line says no more than what it counts. */
const when = (problem: boolean, why: string): string | undefined => (problem ? why : undefined);

/** The share of links that are finish to start, as the panel words it: "4 of 5 links", "no links" with none. */
const share = (of: number, links: number): string => (links === 0 ? "no links" : `${of} of ${links} ${links === 1 ? "link" : "links"}`);

/** The panel's lines for `report`, one for each check; a missing data date is a sentence, never a 0. */
export function checkLines(report: ChecksReport): CheckLine[] {
  return [
    { name: "Missing logic", value: String(report.missingLogic), says: "Unfinished tasks with nothing before them or nothing after them. Summary tasks are not counted.", why: when(report.missingLogic > 0, "Nothing moves these tasks when a task before them slips."), show: report.missingLogic > 0 ? "missing-logic" : undefined },
    { name: "Open starts", value: String(report.openStarts), says: "Unfinished tasks with nothing before them. Summary tasks are not counted.", why: when(report.openStarts > 0, "Nothing comes before these tasks, so the schedule does not drive their start."), show: report.openStarts > 0 ? "open-start" : undefined },
    { name: "Open ends", value: String(report.openEnds), says: "Unfinished tasks with nothing after them. Summary tasks are not counted.", why: when(report.openEnds > 0, "Nothing comes after these tasks, so their finish pushes nothing."), show: report.openEnds > 0 ? "open-end" : undefined },
    { name: "Leads", value: String(report.leads), says: "Links with a negative lag, so the next task starts before the one it follows has finished.", why: when(report.leads > 0, "The next task starts before the one it follows ends, on an overlap the bar hides.") },
    { name: "Lags", value: String(report.lags), says: "Links with a positive lag, a wait between one task and the next.", why: when(report.lags > 0, "A wait is hidden inside a link; a task for the wait shows it.") },
    { name: "Finish to start", value: share(report.finishToStart, report.links), says: "Links where the next task starts when the one before it finishes.", why: when(report.links > report.finishToStart, "Other link types are harder to follow, so most links are usually finish to start.") },
    { name: "High float", value: String(report.highFloat), says: "Unfinished tasks that can slip more than 44 working days before they push the finish.", why: when(report.highFloat > 0, "A task this loose is often missing a successor."), show: report.highFloat > 0 ? "high-float" : undefined },
    { name: "High duration", value: String(report.highDuration), says: "Tasks longer than 44 working days.", why: when(report.highDuration > 0, "A very long task may be a typing slip; if real, split it into steps you can track."), show: report.highDuration > 0 ? "high-duration" : undefined },
    { name: "Hard constraints", value: String(report.hardConstraints), says: "Unfinished tasks a fixed date holds: a start no later than, a finish no later than, or a must start on or must finish on date. Summary tasks are not counted.", why: when(report.hardConstraints > 0, "A fixed date overrides the logic, so predecessors stop driving the task."), show: report.hardConstraints > 0 ? "hard-constraints" : undefined },
    { name: "Negative float", value: String(report.negativeFloat), says: "Unfinished tasks that cannot meet a late date, so their float is below 0 days. Summary tasks are not counted.", why: when(report.negativeFloat > 0, "The task cannot meet its late date as the logic stands."), show: report.negativeFloat > 0 ? "negative-float" : undefined },
    {
      name: "Invalid dates",
      value: report.invalidDates === null ? "needs a data date" : String(report.invalidDates),
      says: "Tasks with an actual start or finish after the data date. Show keeps leaf tasks only.",
      why: when((report.invalidDates ?? 0) > 0, "Work is recorded after the data date: move the date or fix the actual."),
      show: (report.invalidDates ?? 0) > 0 ? "invalid-dates" : undefined,
    },
    { name: "Missing resources", value: report.missingResources === null ? "needs resources" : String(report.missingResources), says: "Unfinished tasks with a duration and no resource. Summary tasks are not counted.", why: when((report.missingResources ?? 0) > 0, "No one is assigned, so the task carries no labor or cost."), show: (report.missingResources ?? 0) > 0 ? "missing-resources" : undefined },
    { name: "Missed", value: String(report.missed), says: "Tasks the baseline had finishing by the data date that finished later or have not finished. 0 with no baseline.", why: when(report.missed > 0, "The baseline had it finished by the data date and it is not, or it finished late."), show: report.missed > 0 ? "missed" : undefined },
    { name: "Execution index", value: report.executionIndex === null ? "none due" : report.executionIndex.toFixed(2), says: "Tasks finished over tasks the baseline had finishing by the data date.", why: when(report.executionIndex !== null && report.executionIndex < 1, "Finished over due; below one means the plan is behind.") },
  ];
}

/** The Checks button opens the report; Show on any of its ten filterable nonzero checks filters and closes it. */
export function installChecks(chrome: ChromeContext, tools: Piece, dialogs: Piece): void {
  const dialog = dialogs.part<HTMLDialogElement>("checks-dialog");
  const body = dialog.querySelector("tbody")!;
  tools.part("checks-button").addEventListener("click", () => {
    body.replaceChildren();
    for (const { name, value, says, why, show } of checkLines(chrome.gantt.checks())) {
      const row = body.insertRow();
      row.insertCell().textContent = name;
      row.insertCell().textContent = value;
      const sentence = row.insertCell();
      sentence.textContent = says;
      if (why !== undefined) {
        const reason = sentence.appendChild(document.createElement("span"));
        reason.dataset.part = "checks-why";
        reason.textContent = why;
      }
      const cell = row.insertCell();
      if (show === undefined) continue;
      const button = cell.appendChild(document.createElement("button"));
      button.type = "button";
      button.dataset.part = "checks-show";
      button.textContent = "Show";
      button.setAttribute("aria-label", `Show ${name}`);
      button.addEventListener("click", () => {
        chrome.gantt.commands.setFilter({ check: show });
        dialog.close();
      });
    }
    dialog.showModal();
  });
  // Escape closes a modal dialog by itself; every way of closing it ends here, and the chart gets its keys back.
  dialog.addEventListener("close", () => refocus(chrome.root));
  dialogs.part("checks-close").addEventListener("click", () => dialog.close());
}
