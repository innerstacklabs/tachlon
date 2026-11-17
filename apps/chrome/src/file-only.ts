import type { CompareResult } from "gantt-core";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

/**
 * The tasks and links only last month's file has, and those only this month's (the open schedule) has (gs#703): after "Compare with a file...", a closable
 * panel names them in four lists, one per line in that file's order, as `commands.setBaselineFromFile` answers (`fileOnly` and `hereOnly`:
 * the ID and the name; `fileOnlyLinks` and `hereOnlyLinks`: `pred → succ (type)`; each the first 200 and then "and N more"); a list with none reads "None".
 * A comparison with none in any of the four leaves the panel hidden; a refusal leaves it as it was; opening a file hides it. The × and Escape (with focus in the panel) close it and return focus to the chart.
 * Returns the function that shows the answer of a comparison, and the one that hides the panel.
 */
export function installFileOnly(chrome: ChromeContext, dialogs: Piece): { show: (answer: CompareResult) => void; hide: () => void } {
  const { root } = chrome;
  const panel = dialogs.part("file-only");
  const hide = (): void => void (panel.hidden = true);
  const fold = (): void => {
    hide();
    refocus(root);
  };
  dialogs.part("file-only-close").addEventListener("click", fold);
  panel.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !panel.hidden) {
      event.stopPropagation();
      fold();
    }
  });
  return {
    hide,
    show: (answer) => {
      if (!answer.ok) return;
      const lists = [
        ["file-only-list", answer.fileOnly],
        ["here-only-list", answer.hereOnly],
        ["file-only-links-list", answer.fileOnlyLinks],
        ["here-only-links-list", answer.hereOnlyLinks],
      ] as const;
      for (const [part, lines] of lists) dialogs.part(part).textContent = lines.length === 0 ? "None" : lines.join("\n");
      panel.hidden = lists.every(([, lines]) => lines.length === 0);
    },
  };
}
