// What the host's edit commands answer (gs#1195): `{ ok: true }` or `{ ok: false, reason, message }`, the reason a name from a
// short public list that a host matches on, the message the sentence a person sees: `EditError`'s Display, which the chart's own
// alert line shows too (gs#1260). This is the one place the app's refusal kinds (`EditError::kind`, carried in the JSON of the
// `try_*` exports) are mapped to those names, so renaming a kind in the app changes this table, never what a host matches on.
import { refusal } from "./wasm/app.js";

/** Why an edit command changed nothing. */
export type EditRefusal =
  /** A cell is being edited, or a bar or a link is being pressed or dragged (`Gantt.busy()`). */
  | "busy"
  /** The host switched the chart to read-only for this person (`App.set_read_only`): nothing in it can be changed. */
  | "readOnly"
  /** The schedule has no outline (a generated stress fixture). */
  | "noOutline"
  /** `deleteRow` with no task selected, `deleteLink` with no link selected. */
  | "noSelection"
  /** `insertRow` under a row filter or a lookahead window: the new row would not pass it and would vanish. */
  | "filterActive"
  /** `deleteRow` on a summary: delete or move its sub-tasks first. */
  | "hasChildren"
  /** `deleteRow` with a range of several rows selected: it deletes one row. */
  | "severalRows"
  /** `clearBaseline` or `addNewToBaseline` with no baseline set. */
  | "noBaseline"
  /** `setDataDate` with text that is not a real day written `YYYY-MM-DD`. */
  | "badDate"
  /** `acceptProposal` with no proposal held, `acceptProposed` for a change not held. */
  | "noProposal"
  /** `assignKey` with a key of no characters, or that is not text. */
  | "emptyKey"
  /** `assignKey` on a task or link that already holds a different key. */
  | "hasKey"
  /** `assignKey` with a key another task, or another link, already holds (a deleted one counts: an undo brings it back). */
  | "keyTaken"
  /** `assignKey` on a task slot the chart never made, or not a whole number. */
  | "unknownTask"
  /** `assignKey` on a link slot the chart never made. */
  | "unknownLink"
  /** The page's before-edit function refused it (`message` is its reason, or the chart's own sentence). */
  | "refused"
  /** The page's before-edit function threw or answered with a promise: nothing changed, and `error` fired with the source `beforeEdit`. */
  | "beforeEditFailed"
  /** Anything else the schedule refused (rare: a solve that fails). */
  | "other";

/** Why nothing changed: `reason` to match on, `message` to show (the chart's own sentence for it). */
export type Refusal = { ok: false; reason: EditRefusal; message: string };

/** What `setDataDate`, `setBaseline`, `clearBaseline`, `addNewToBaseline` and, in a promise, `setBaselineFromFile` answer. */
export type EditResult = { ok: true } | Refusal;

/** What `setBaselineFromFile` answers in a promise: `fileOnly` names the tasks only the file has and `hereOnly` those only the open schedule has (`A1020 Pour slab`), `fileOnlyLinks` and `hereOnlyLinks` the links only in each (`A1010 → A1020 (FS)`), the first 200 of each and then `and N more`; or why nothing changed. */
export type CompareResult = { ok: true; fileOnly: readonly string[]; hereOnly: readonly string[]; fileOnlyLinks: readonly string[]; hereOnlyLinks: readonly string[] } | Refusal;

/** What `insertRow` and `deleteRow` answer: the task inserted or deleted, or why nothing changed. */
export type RowResult = { ok: true; task: number } | Refusal;

/** What `acceptProposal` answers: how many changes it applied, as one undo step, or why nothing changed. */
export type AcceptResult = { ok: true; changes: number } | Refusal;

/** What `deleteLink` answers: the link removed (a dependency slot), or why nothing changed. */
export type LinkResult = { ok: true; link: number } | Refusal;

/** Each app kind a host command can meet, and its public name. A kind not listed reads as `"other"`. */
const PUBLIC_REASON: Readonly<Record<string, EditRefusal>> = {
  busy: "busy",
  readOnly: "readOnly",
  refused: "refused",
  beforeEditFailed: "beforeEditFailed",
  noOutline: "noOutline",
  noSelection: "noSelection",
  noLinkSelected: "noSelection",
  filterActive: "filterActive",
  hasChildren: "hasChildren",
  severalRows: "severalRows",
  noBaseline: "noBaseline",
  badDataDate: "badDate",
  noProposal: "noProposal",
  emptyKey: "emptyKey",
  hasKey: "hasKey",
  keyTaken: "keyTaken",
  unknownTask: "unknownTask",
  unknownLink: "unknownLink",
};

/** Read the JSON a `try_*` export answered. */
export function readAnswer(json: string): { ok: true; task?: number; link?: number; changes?: number } | Refusal {
  const answer = JSON.parse(json) as { ok: true; task?: number; link?: number; changes?: number } | { ok: false; reason: string; message: string };
  if (answer.ok) return Object.freeze(answer);
  const reason = Object.hasOwn(PUBLIC_REASON, answer.reason) ? PUBLIC_REASON[answer.reason]! : "other";
  return Object.freeze({ ok: false, reason, message: answer.message });
}

/** A refusal the page makes before asking the app, worded by the app: `"busy"`, `"readOnly"`, or `"badDate"` for a day that is not one. */
export const refused = (reason: "busy" | "badDate" | "readOnly"): Refusal => readAnswer(refusal(reason === "badDate" ? "badDataDate" : reason)) as Refusal;
