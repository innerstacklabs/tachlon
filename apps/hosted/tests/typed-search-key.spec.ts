import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";
import { applyAll, isMenuQuestion, KEY, menuStates, openFitout, type Applied, type MenuState } from "./support/typed-search";

// gantt#1250, the typed-model experiment's answer key (docs/research/typed-search-questions.json): twenty-four "find tasks by what I mean"
// questions, each with the one menu state that answers it, the activities the chart keeps there and the other menu states that keep that same set. This opens the fit-out file (the page starts on a copy without crews) and applies all 512 menu
// states of the office fit-out through the chart and holds the key to what it reads back. Four more questions (q21 to q24) are ones the menu cannot answer; they name no menu state. No model is called.

test.use({ viewport: { width: 1280, height: 1000 } });

const same = (left: MenuState, right: MenuState): boolean => JSON.stringify(left) === JSON.stringify(right);

let states: MenuState[];
let applied: Applied[];

test.beforeAll(async ({ browser }) => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await openFitout(page);
  states = menuStates();
  applied = await applyAll(page, states);
  await page.close();
});

test("the chart takes all 512 menu states and reads each one back as it was asked", () => {
  expect(states).toHaveLength(512);
  const refused = states.filter((_, index) => !applied[index]!.taken).map((state) => JSON.stringify(state));
  expect(refused, "states the chart refused").toEqual([]);
  expect(applied.map((one) => one.readback)).toEqual(states);
});

test("each menu question's state keeps the stored activities, the same-set states match, and no two questions share a set", () => {
  expect(KEY.questions.filter(isMenuQuestion)).toHaveLength(24);
  const seen = new Set<string>();
  for (const { id, state, keeps, sameSet } of KEY.questions.filter(isMenuQuestion)) {
    const at = states.findIndex((one) => same(one, state));
    expect(at, `${id}: the key state is a menu state`).toBeGreaterThanOrEqual(0);
    expect(applied[at]!.kept, `${id}: the activities the chart keeps in the key state`).toEqual(keeps);
    const others = states.filter((_, index) => index !== at && applied[index]!.kept.join() === keeps.join());
    expect(others, `${id}: the other menu states that keep the same set`).toEqual(sameSet);
    expect(keeps.length, `${id}: some of the 50 activities, not none or all`).toBeGreaterThan(0);
    expect(keeps.length).toBeLessThan(50);
    expect(seen.has(keeps.join()), `${id}: no other question keeps this set`).toBe(false);
    seen.add(keeps.join());
  }
});

test("each question the menu cannot answer names no menu state, keeps nothing and says why", () => {
  const cannot = KEY.questions.filter((question) => !isMenuQuestion(question));
  expect(cannot.map((question) => question.id)).toEqual(["q21", "q22", "q23", "q24"]);
  for (const { id, state, keeps, sameSet, reason } of cannot) {
    expect(state, `${id}: "cannot", not a menu state`).toBe("cannot");
    expect(keeps, `${id}: keeps nothing`).toEqual([]);
    expect(sameSet, `${id}: no same-set states`).toEqual([]);
    expect(reason?.trim(), `${id}: the one-line reason`).toBeTruthy();
  }
});
