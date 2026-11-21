import type { Page } from "@playwright/test";
import { SCENARIOS, type Form } from "./scenarios.ts";
import { startServer } from "./server.ts";

const FORMS: Form[] = ["handlers", "url", "function"];
/** The scenarios a form does not run (decision 0022, rule 11.1). The cases inside a scenario that a form skips are left to the row that writes the scenario. */
const SKIP: Record<Form, number[]> = {
  handlers: [6, 18, 19, 20],
  url: [6, 7, 8, 9, 25, 26],
  function: [18, 19, 20, 24, 25, 26],
};
/** The forms the example has so far; the row that adds the URL and the function form adds them here. */
const BUILT: Form[] = ["handlers"];
/** The scenarios that fail until what they test is built. Each is expected to fail, and the run fails, naming it, when one passes: the row that makes it pass deletes its name here. A name ending in `(keys chart)` or `(keys server)` is pending in that key mode only. */
const PENDING = new Set([
  "6 a server recalculation", "7 an outside change during a save", "8 a late result", "9 an old read", "10 a lost response", "11 a failure part of the way",
  "12 undo across the server", "13 the server is away", "14 discard", "15 repeats off", "16 the host's own names",
  "17 a server that ignores the key, keys chart", "17 a server that ignores the key, keys server", "21 a 500 that applied nothing", "22 an expired login",
  "24 a 503 part of the way", "25 a create the server cannot repeat",
]);

const scenarios = SCENARIOS.map((s) => ({ ...s, forms: FORMS.filter((f) => !SKIP[f].includes(s.n) && !s.not?.includes(f)) }));

// The example app is the page, and the fake server is the host's API: the page asks its own origin for /api/..., and each request goes to the server of the run.
export default async function (page: Page, url: string): Promise<void> {
  for (const s of scenarios) {
    for (const form of s.forms.filter((f) => BUILT.includes(f))) {
      for (const keys of s.keys ? [s.keys] : (["chart", "server"] as const)) {
        const label = `scenario ${s.name} (${form} form, keys ${keys})`;
        const server = await startServer({ ...s.script, keys });
        const context = await page.context().browser()!.newContext(); // the page check.ts hands over owns its context, so each run gets its own
        const run = await context.newPage();
        try {
          await run.route((u) => u.pathname.startsWith("/api/"), (route) => {
            const u = new URL(route.request().url());
            return route.continue({ url: server.url + u.pathname + u.search });
          });
          // The page's chart records every `error` event in `window.errors` and gets the setting a scenario names (`props`); the page's own code has neither.
          await run.route((u) => /^\/src\/main[\w-]*\.tsx$/.test(u.pathname), async (route) => {
            const response = await route.fetch();
            const [code, chart, own, record] = [await response.text(), "_jsxDEV(Gantt, {", "onError: (e) =>", "void (window.errors ??= []).push(e)"];
            if (![chart, own].every((a) => code.includes(a))) throw new Error("the page's chart call is not where the suite expects it"); // a page with no onError of its own gets a branch here from the row that writes it
            return route.fulfill({ response, body: code.replace(own, `${s.props ? `${s.props}, ` : ""}${own} ${record} ||`) }); // after the page's own props, so a scenario's setting wins over the page's
          });
          const crashed = new Promise<never>((_, reject) => run.on("pageerror", reject)); // a page that throws (the chart refusing to mount) ends the scenario at once, not after its waits
          crashed.catch(() => undefined);
          run.setDefaultTimeout(10_000);
          await run.goto(`${url}?keys=${keys}`);
          const failure = await Promise.race([s.run(run, server, keys, form), crashed]).then(() => undefined, (e: Error) => e);
          if (!PENDING.has(s.name) && !PENDING.has(`${s.name} (keys ${keys})`)) {
            if (failure) throw new Error(`${label}: ${failure.message}`);
            console.log(`  pass ${label}`);
          } else if (failure) console.log(`  pending ${label}: failed as expected, ${failure.message.split("\n")[0]}`);
          else throw new Error(`${label} is pending and passed: delete its name from PENDING`);
        } finally {
          await context.close();
          await server.close();
        }
      }
    }
  }
}
