// The data adapters of the package, re-exported by the page entry `index.ts`.
// The DHTMLX Gantt data adapter (gs#1097); docs/DHTMLX.md.
export { fromDhtmlx } from "./dhtmlx.js";
export type { DhtmlxOptions, DhtmlxResult } from "./dhtmlx.js";
// The plain task adapter (tl#1079): tasks and links as plain JSON into a document; `validate` names every mistake in them (tl#1249).
export { fromTasks } from "./plain.js";
export type { PlainCalendar, PlainLink, PlainOptions, PlainResult, PlainTask } from "./plain.js";
export { toPlain } from "./to-plain.js";
export { PlainDataError, validate } from "./validate.js";
export type { PlainIssue, PlainValidation } from "./validate.js";
// The Bryntum Gantt adapter (gs#1098): pure TypeScript beside the chart, so a host can read Bryntum JSON into a document.
export { fromBryntum, type BryntumResult, type ScheduleDocument } from "./bryntum.js";
