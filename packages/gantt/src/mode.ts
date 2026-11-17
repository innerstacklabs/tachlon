import { type Selected } from "./api.js";

/**
 * Where the keyboard is: in the `chart`, in the `grid` (the cell cursor takes the printable keys), in a `cell-edit` or in a
 * `link-edit`. `keys.ts` branches on it.
 */
export type Mode = "chart" | "grid" | "cell-edit" | "link-edit";

/** What the mode is computed from: the cursor's `entered`, the app's selection and visible row count, and the two editors. */
export interface ModeFacts {
  entered: boolean;
  selected: Selected | null;
  rowCount: number;
  cellEditing: boolean;
  linkEditing: boolean;
}

/** The selected row is drawn: a row hidden under a collapsed summary or by a filter has no table row (the app gives it a row past the last). */
export const drawn = (selected: Selected | null, rowCount: number): boolean => selected !== null && selected.row < rowCount;

/** The editors first; the grid only while the cursor was entered and its row is drawn, so a hidden row never holds the letter keys. */
export const modeOf = (f: ModeFacts): Mode =>
  f.cellEditing ? "cell-edit" : f.linkEditing ? "link-edit" : f.entered && drawn(f.selected, f.rowCount) ? "grid" : "chart";
