// The page's before-edit function (gantt#2085) is the host's code: a throw, a promise or an unknown answer fires `error` once and reaches the app as a failure, which refuses the action with `beforeEditFailed`.
import { foldChanges, type FieldChange, type Stamped } from "./change-set.ts"; // `.ts`, so the Node tier can load this file
import type { BeforeEdit, BeforeEditChange } from "./gantt-types.js";
import type { Change } from "./events.js";
/** `form` puts the changes in the host's unit and date form (`change-form.ts`). */
export function guardBeforeEdit(before: BeforeEdit, fail: (message: string) => void, form: (changes: readonly Stamped[]) => Change[]): (change: Omit<BeforeEditChange, "changes"> & { changes: readonly FieldChange[]; needs?: readonly string[] }) => unknown {
  return ({ needs, ...change }) => { // `needs` is the chart's own gate's: the host's function is not shown it
    let answer: unknown;
    try {
      answer = before({ ...change, changes: form(foldChanges(change.changes)) });
    } catch (err) {
      fail(`the before-edit function threw: ${err instanceof Error ? err.message : String(err)}`);
      throw err;
    }
    if (answer === undefined || answer === null || answer === true) return undefined;
    if (answer === false || typeof answer === "string") return answer;
    fail(`the before-edit function answered with ${typeof (answer as { then?: unknown }).then === "function" ? "a promise" : `a ${typeof answer}`}: answer at once`);
    return answer;
  };
}
