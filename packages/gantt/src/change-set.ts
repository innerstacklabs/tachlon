// One change per record (decision 0022, section 2): the engine writes one change per field of an update, and a create's or a delete's whole record
// in `fields`; the host reads `{ record, op, key, set, was }`. The `edit` event and the before-edit function both fold here.
import type { Change } from "./events.js";

/** A change as the engine writes it: an update names one `field` with its `old` and `new` value; a create or a delete holds the record in `fields`. */
export interface FieldChange {
  record: "task" | "link" | "schedule";
  op: "update" | "create" | "delete";
  key?: string | null;
  task?: number;
  link?: number;
  field?: string;
  old?: unknown;
  new?: unknown;
  fields?: Record<string, unknown>;
}

/**
 * Fold the changes of one commit into one change per record, in the order each record first appears (rule 2.6 holds: the engine writes a
 * child's delete before its parent's). A create's `fields` are its `set`, a delete's `fields` its `was` (any member the same commit also
 * updated read as it was before), and a link delete with an end that a task delete in the same commit removed is `cascade: true` (rule 2.7).
 */
export function foldChanges(changes: readonly FieldChange[]): Change[] {
  const records = new Map<string, FieldChange[]>();
  for (const c of changes) {
    const id = `${c.record}:${c.task ?? c.link ?? ""}`;
    records.set(id, [...(records.get(id) ?? []), c]);
  }
  const gone = new Set<unknown>(changes.filter((c) => c.record === "task" && c.op === "delete").flatMap((c) => [c.key ?? c.task, c.task]));
  const endOf = (end: unknown): unknown => (typeof end === "string" ? end : (end as { task?: number } | null)?.task);
  const cut = (r: FieldChange): boolean => r.op === "delete" && [r.fields?.from, r.fields?.to].some((end) => gone.has(endOf(end)));
  return [...records.values()].map((rows) => {
    const [first] = rows as [FieldChange];
    const made = rows.find((r) => r.op === "create");
    const lost = rows.find((r) => r.op === "delete");
    const updates = rows.filter((r) => r.op === "update");
    const was = Object.fromEntries(updates.map((r) => [r.field, r.old]));
    const change: Change = { record: first.record, op: made ? "create" : lost ? "delete" : "update", key: first.key ?? null };
    if (first.task !== undefined) change.task = first.task;
    if (first.link !== undefined) change.link = first.link;
    if (made) change.set = { ...made.fields };
    else if (lost) change.was = { ...lost.fields, ...was };
    else Object.assign(change, { set: Object.fromEntries(updates.map((r) => [r.field, r.new])), was });
    if (first.record === "link" && rows.some(cut)) change.cascade = true;
    return change;
  });
}
