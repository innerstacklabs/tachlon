import type { ViewState } from "gantt-core";
import type { Piece } from "./parts.js";

/** The dot beside the open file's name (gs#703): shown while `state().unsaved`. */
export function installSaveMark(tools: Piece): (state: ViewState) => void {
  const dot = tools.part("unsaved-mark");
  return (state) => {
    dot.hidden = !state.unsaved;
  };
}
