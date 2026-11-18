// The page's before-edit function (gantt#2085) is the host's code: a throw, a promise or an unknown answer fires `error` once and reaches the app as a failure, which refuses the action with `beforeEditFailed`.
import type { BeforeEdit, BeforeEditChange } from "./gantt-types.js";
export function guardBeforeEdit(before: BeforeEdit, fail: (message: string) => void): (change: BeforeEditChange) => unknown {
  return (change) => {
    let answer: unknown;
    try {
      answer = before(change);
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
