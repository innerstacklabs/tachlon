import type { ChromeContext } from "./parts.js";

/** Find's floor beside a plain count: its 90px hint and padding (gs#1055). */
const FIND_FLOOR_PX = 104;

export interface RowCountFit {
  /** Say whether the count is plain; the fit runs again when that changes. */
  fit: (plain: boolean) => void;
  /** Watch the chart's find slot as it is now: its elements are new after a switch to WebGL2. */
  watch: () => void;
}

/**
 * The plain count ("59 rows") gives way only when Find would otherwise fall under its 104px floor (its 90px hint and padding), measured
 * with the count shown; a filter count ("8 of 43 rows", "46 slipped") never does (gs#1055). Find shrinks first, so its size changes with
 * the strip's.
 */
export function installRowCountFit(chrome: ChromeContext, rowCount: HTMLElement): RowCountFit {
  let findBox: HTMLElement | null = null;
  let plainCount = true;
  // The state is read with the count shown (a hidden count is shown for the read and put back), and it changes only when the read says so,
  // so the resize a change makes re-enters once, reads the same state and writes nothing.
  const fitCount = (): void => {
    if (findBox === null) return;
    const was = rowCount.hasAttribute("data-give-way");
    if (was && plainCount) rowCount.removeAttribute("data-give-way");
    const give = plainCount && findBox.getBoundingClientRect().width < FIND_FLOOR_PX;
    if (give) rowCount.setAttribute("data-give-way", "");
    else if (was && !plainCount) rowCount.removeAttribute("data-give-way");
  };
  const observer = new ResizeObserver(fitCount);
  chrome.owns(() => observer.disconnect());
  return {
    fit: (plain) => {
      if (plainCount === plain) return;
      plainCount = plain;
      fitCount();
    },
    watch: () => {
      observer.disconnect();
      findBox = chrome.root.querySelector<HTMLElement>("[data-part=find]");
      if (findBox !== null) observer.observe(findBox);
    },
  };
}
