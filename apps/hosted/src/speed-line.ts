// The hosted page's speed line (gantt#1790): `s` with focus in the chart shows the last open's time and the last frames' numbers in one
// line over the chart's bottom edge, and `s` again hides it. It reads `Gantt.speed()` only, which calls no `App` method, so polling it
// does not wake a sleeping chart. It polls four times a second while shown and writes the text only when it changed.
import type { Gantt, Speed } from "gantt-core";

const POLL_MS = 250;

/** Milliseconds as the developer page's status line writes them: one decimal under 10, whole numbers above. */
const ms = (value: number): string => (value < 10 ? value.toFixed(1) : Math.round(value).toLocaleString("en-US"));

/** The line's text: the open's total, then the frame window's p50 and p95. */
export function speedText({ open, frames }: Speed): string {
  const opened = open === null ? "open: no timed open yet" : `open ${open.name} ${ms(open.timings.total)} ms`;
  const drawn =
    frames === null
      ? "frames: scroll or drag to measure"
      : `frames p50 ${ms(frames.p50)} ms, p95 ${ms(frames.p95)} ms, ${frames.missed} of ${frames.count} missed at ${Math.round(frames.refreshHz)} Hz`;
  return `${opened}  |  ${drawn}`;
}

/** Make `s` (the chart's `toggle-status`) show and hide `line`, an element that starts hidden. */
export function installSpeedLine(gantt: Gantt, line: HTMLElement): void {
  let timer: number | undefined;
  const write = (): void => {
    const text = speedText(gantt.speed());
    if (line.textContent !== text) line.textContent = text;
  };
  gantt.on("toggle-status", () => {
    if (timer === undefined) {
      write();
      line.hidden = false;
      timer = window.setInterval(write, POLL_MS);
    } else {
      window.clearInterval(timer);
      timer = undefined;
      line.hidden = true;
    }
  });
}
