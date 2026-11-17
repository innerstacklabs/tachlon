import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { isMenuQuestion, KEY, type MenuState } from "./support/typed-search";

// gantt#1250, the typed-model experiment's scorer: it reads a model's answers to the questions of the key
// (docs/research/typed-search-questions.json) and scores each one by the exact chart state the key says, so every model is scored by this one rule.
// TYPED_SEARCH_ANSWERS names the answers file: a JSON array with one { "id": "q01", "state": { "row": ..., "lookahead": ... } } for each question,
// the state in the shape of the key's, "cannot" where the model says no menu state answers it, or null where the model declined. Verdicts for a menu
// question: "exact" (the key's state), "same-set" (another menu state that keeps the same activities, the key's sameSet), "wrong" (any other state)
// and "declined" (null or "cannot": the menu can answer it). For a question the menu cannot answer (key state "cannot"): "cannot-right" ("cannot"),
// "wrong-confident" (any filter, so a model that guesses is caught) and "declined" (null). Nothing here calls a model or opens the chart.

type Verdict = "exact" | "same-set" | "wrong" | "declined" | "cannot-right" | "wrong-confident";

/** A state as one string, so the order of an answer's keys never counts and a malformed state matches no key. */
const canon = (state: Partial<MenuState> | null | undefined): string =>
  JSON.stringify([
    typeof state?.row === "string" ? state.row : Object.entries(state?.row ?? {}).sort(),
    state?.lookahead ? [state.lookahead.start, state.lookahead.weeks] : null,
  ]);

function score(text: string): { id: string; verdict: Verdict }[] {
  const answers: { id: string; state?: MenuState | "cannot" | null }[] = JSON.parse(text);
  expect(answers.map((answer) => answer.id).sort(), "one answer for each question").toEqual(KEY.questions.map((question) => question.id).sort());
  return KEY.questions.map(({ id, state, sameSet }) => {
    const given = answers.find((answer) => answer.id === id)!.state;
    if (given == null) return { id, verdict: "declined" };
    if (state === "cannot") return { id, verdict: given === "cannot" ? "cannot-right" : "wrong-confident" };
    if (given === "cannot") return { id, verdict: "declined" };
    if (canon(given) === canon(state)) return { id, verdict: "exact" };
    return { id, verdict: sameSet.some((other) => canon(other) === canon(given)) ? "same-set" : "wrong" };
  });
}

test("the answers file is scored question by question", () => {
  const file = process.env.TYPED_SEARCH_ANSWERS;
  test.skip(!file, "TYPED_SEARCH_ANSWERS names the answers file to score");
  const verdicts = score(readFileSync(file!, "utf8"));
  const count = (which: Verdict): number => verdicts.filter((one) => one.verdict === which).length;
  console.log(
    `typed-search score: exact ${count("exact")}, same-set ${count("same-set")}, wrong ${count("wrong")}, declined ${count("declined")}, ` +
      `cannot-right ${count("cannot-right")}, wrong-confident ${count("wrong-confident")} of ${verdicts.length}`,
  );
  for (const { id, verdict } of verdicts) console.log(`${id} ${verdict}`);
});

test("canned answers score exact, same-set, wrong, declined, cannot-right and wrong-confident by the key's own states", () => {
  const menu = KEY.questions.filter(isMenuQuestion);
  const [first, second, third] = menu;
  const sameSetQuestion = menu.find((question) => question.sameSet.length > 0)!;
  const answers = KEY.questions.map(({ id, state }) => ({ id, state: (state === "cannot" ? state : { lookahead: state.lookahead, row: state.row }) as MenuState | "cannot" | null }));
  const give = (id: string, state: MenuState | "cannot" | null): void => void (answers.find((answer) => answer.id === id)!.state = state);
  give(sameSetQuestion.id, sameSetQuestion.sameSet[0]!);
  give(second!.id, first!.state); // the second question answered with the first one's state
  give(third!.id, null);
  give("q06", "cannot"); // "cannot" to a question the menu can answer: declined
  give("q22", first!.state); // a filter for a question the menu cannot answer: wrong-confident
  give("q23", null);
  const expected: Record<string, Verdict> = { [sameSetQuestion.id]: "same-set", [second!.id]: "wrong", [third!.id]: "declined", q06: "declined", q22: "wrong-confident", q23: "declined" };
  const want = KEY.questions.map(({ id, state }): Verdict => expected[id] ?? (state === "cannot" ? "cannot-right" : "exact")); // q21 and q24 answered "cannot"
  expect(score(JSON.stringify(answers)).map((one) => one.verdict)).toEqual(want);
});

test("an answer missing a question is refused, not scored", () => {
  expect(() => score(JSON.stringify(KEY.questions.slice(1).map(({ id, state }) => ({ id, state }))))).toThrow(/one answer for each question/);
});
