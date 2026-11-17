import type { ViewState } from "gantt-core";
import type { Piece } from "./parts.js";

const fmt = new Intl.NumberFormat("en-US");

/** The longest-path line (gantt#703), at the chart's bottom left while Longest path is on: `Longest path: Excavate to Handover · 14 activities · 212 days`, the calendar days from the first start to the project finish. Empty, so hidden by the chart's CSS, with the filter off. */
export function installPathLine(line: Piece): (state: ViewState) => void {
  const text = line.part<HTMLSpanElement>("path-text");
  return ({ path }) => {
    const next = path === null ? "" : `Longest path: ${path.first} to ${path.last} · ${fmt.format(path.count)} ${path.count === 1 ? "activity" : "activities"} · ${fmt.format(path.days)} ${path.days === 1 ? "day" : "days"}`;
    if (text.textContent !== next) text.textContent = next;
  };
}
