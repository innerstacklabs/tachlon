import type { CompareResult, EditResult } from "gantt-core";
import { type ChromeContext, refocus } from "./parts.js";

/**
 * Compare with a file (gs#703): a second file, such as last month's, becomes the baseline through `commands.setBaselineFromFile`, one undo step.
 * The toolbar's baseline summary then says what slipped. A refusal (a file of another format, or of another project) goes to `refuse`, which
 * shows it; nothing changed. The control that chooses the file is the host's (the hosted app's File menu); this is what it calls.
 */
export function installCompareFile({ gantt, root }: ChromeContext, refuse: (answer: EditResult | undefined) => boolean): (file: File) => Promise<CompareResult | undefined> {
  return async (file) => {
    const answer = await gantt.commands.setBaselineFromFile(file);
    refuse(answer);
    refocus(root);
    return answer;
  };
}
