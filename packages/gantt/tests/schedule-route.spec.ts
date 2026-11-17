import { readFileSync } from "node:fs";
import { acrossReload, expect, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { backend, url } from "./support/knobs";

// The developer page's ?schedule=1 route (gs#541): the four checks of j1-open.spec.ts that the hosted app cannot carry,
// because it has no schedule.dat (that spec moved to the hosted app's own tests). They stay here, on the page that
// still serves the route. The assertions are the old ones, unchanged.

const summaryFile = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url), "utf8");
// The file has no line for the panel on its own (the app rolls summaries up). A link into the second summary task is
// dropped and listed, which gives the panel its one line: "dependency 3 -> 4 (FS) dropped: task 4 is a summary task".
const summaryEnd = "<Duration>PT0S</Duration><DurationFormat>7</DurationFormat></Task>";
const summaryLink =
  "<Duration>PT0S</Duration><DurationFormat>7</DurationFormat><PredecessorLink><PredecessorUID>3</PredecessorUID><Type>1</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink></Task>";
const fixture = summaryFile.replace(`<Name>Second summary</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary>${summaryEnd}`, `<Name>Second summary</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary>${summaryLink}`);
if (fixture === summaryFile) {
  throw new Error("the summary fixture no longer has the second summary task this spec patches");
}
const errorPrefix = "could not load schedule:";

interface ImportedState {
  n: number | null;
  labels: string[];
  table: string;
  panelHidden: boolean;
  title: string;
  unsupported: string;
}

test("imported schedule shows rows, names and a closable unsupported panel", async ({ page }) => {
  const logs = trackLogs(page);
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/xml", body: fixture }),
  );
  await page.goto(`${url}?schedule=1${backend === "webgl" ? "&backend=webgl" : ""}`);

  // A WebGPU failure reloads the page on WebGL2 and can destroy the execution context under the read, so the wait
  // and the read retry together.
  let state: ImportedState = { n: null, labels: [], table: "", panelHidden: true, title: "", unsupported: "" };
  try {
    state = await acrossReload(page, async () => {
      await waitReady(page, { frames: 3, timeout: 60_000 });
      return page.evaluate((): ImportedState => {
        const labels = Array.from({ length: 5 }, (_, i) => window.gantt!.app.task_label(i));
        const panel = document.getElementById("unsupported");
        return {
          n: window.gantt!.status().n,
          labels,
          table: document.getElementById("table-rows")?.textContent ?? "",
          panelHidden: panel === null || Boolean(panel.hidden),
          title: document.getElementById("unsupported-title")?.textContent ?? "",
          unsupported: document.getElementById("unsupported-list")?.textContent ?? "",
        };
      });
    });
  } catch {
    // The checks below report the empty state with their own messages.
  }

  expect.soft(state.n === 5, `imported schedule row count is 5 (measured ${state.n})`).toBe(true);
  expect
    .soft(
      state.labels.includes("Second child") && state.table.includes("Second child"),
      "imported task name Second child is available from the app and in the table DOM",
    )
    .toBe(true);
  expect
    .soft(
      state.panelHidden && state.title.startsWith("1 import note: what was not read from this file") && state.unsupported === "dependency 3 -> 4 (FS) dropped: task 4 is a summary task",
      `imported unsupported list is behind the notice, with the one dropped summary link (${JSON.stringify(state.title)}; ${JSON.stringify(state.unsupported)})`,
    )
    .toBe(true);

  // The notice opens the list, and the list's close button folds it away: a missing or broken notice fails here.
  await page.locator("#import-notes").click();
  await expect(page.locator("#unsupported"), "the list is visible after the notice is clicked").toBeVisible();
  await expect(page.locator("#unsupported-list")).toHaveText("dependency 3 -> 4 (FS) dropped: task 4 is a summary task");
  await page.locator("#unsupported-close").click();
  await expect(page.locator("#unsupported"), "the list is hidden after its close button is clicked").toBeHidden();

  const importedErrors = pageErrors(logs);
  expect
    .soft(
      importedErrors.length === 0,
      `imported schedule page logged no page errors${importedErrors.length ? `: ${importedErrors.join(" | ")}` : ""}`,
    )
    .toBe(true);
});

test("malformed schedule shows the importer error", async ({ page }) => {
  const malformedErrors: string[] = [];
  page.on("pageerror", (error) => malformedErrors.push(error.message));
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/xml", body: "<Project><Tasks>" }),
  );
  await page.goto(`${url}?schedule=1&backend=webgl`);
  await page.waitForFunction(
    () => document.getElementById("overlay")?.textContent?.startsWith("could not load schedule:"),
    null,
    { timeout: 30_000 },
  );
  const state = await page.evaluate(() => {
    const text = document.getElementById("overlay")?.textContent ?? "";
    return {
      text,
      ganttUndefined: window.gantt === undefined,
      panelHidden: document.getElementById("unsupported")?.hidden ?? true,
    };
  });

  expect
    .soft(
      state.text.startsWith(errorPrefix) && state.text.length > errorPrefix.length,
      `malformed schedule shows importer error text (${JSON.stringify(state.text)})`,
    )
    .toBe(true);
  expect.soft(state.ganttUndefined, "malformed schedule creates no chart hook").toBe(true);
  expect.soft(state.panelHidden, "malformed schedule keeps the unsupported panel hidden").toBe(true);
  expect
    .soft(
      malformedErrors.length === 0,
      `malformed schedule page logged no pageerror${malformedErrors.length ? `: ${malformedErrors.join(" | ")}` : ""}`,
    )
    .toBe(true);
});

test("a truncated XER shows the XER reader's error", async ({ page }) => {
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: "ERMHDR\t19.12\n%T\tTASK\n" }),
  );
  await page.goto(`${url}?schedule=1&backend=webgl`);
  await page.waitForFunction(
    () => document.getElementById("overlay")?.textContent?.startsWith("could not load schedule:"),
    null,
    { timeout: 30_000 },
  );
  const brokenXerText = (await page.locator("#overlay").textContent()) ?? "";
  expect
    .soft(
      brokenXerText.length > errorPrefix.length + 1 && !/XML error/i.test(brokenXerText),
      `a truncated XER shows the XER reader's error, not an XML one (${JSON.stringify(brokenXerText)})`,
    )
    .toBe(true);
});

test("missing schedule shows the serving error", async ({ page }) => {
  await page.goto(`${url}?schedule=1&backend=webgl`);
  await page.waitForFunction(
    () => document.getElementById("overlay")?.textContent?.startsWith("could not load schedule:"),
    null,
    { timeout: 30_000 },
  );
  const missingText = await page.locator("#overlay").textContent();
  expect
    .soft(
      missingText?.startsWith(errorPrefix) === true && missingText.includes("not being served"),
      `missing schedule shows the serving error (${JSON.stringify(missingText)})`,
    )
    .toBe(true);
});
