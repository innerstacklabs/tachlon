import type { Page } from "@playwright/test";
import { SCENARIOS } from "./scenarios.ts";
import { startServer } from "./server.ts";

type Form = "handlers" | "url" | "function";
const FORMS: Form[] = ["handlers", "url", "function"];
/** The scenarios a form does not run (decision 0022, rule 11.1). The cases inside a scenario that a form skips are left to the row that writes the scenario. */
const SKIP: Record<Form, number[]> = {
  handlers: [6, 18, 19, 20],
  url: [6, 7, 8, 9, 25, 26],
  function: [18, 19, 20, 24, 25, 26],
};
/** The forms the example has so far; the row that adds the URL and the function form adds them here. */
const BUILT: Form[] = ["handlers"];
/** The scenarios that fail until what they test is built. Each is expected to fail, and the run fails, naming it, when one passes: the row that makes it pass deletes its name here. */
const PENDING = new Set(["1 slow saves"]);

const scenarios = SCENARIOS.map((s) => ({ ...s, forms: FORMS.filter((f) => !SKIP[f].includes(s.n)) }));

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
          await run.goto(`${url}?keys=${keys}`);
          const failure = await s.run(run, server).then(() => undefined, (e: Error) => e);
          if (!PENDING.has(s.name)) {
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
