// The Bryntum Gantt adapter (gs#1098): a developer who runs Bryntum Gantt today reads its task JSON into the
// document (decision 0010) with `fromBryntum` and feeds that to the chart through `load` (the data API, gs#1084).
// The fixture is a Bryntum load response (tasks with children, dependencies, and the tables the adapter leaves out).
// The dates below are plain calendar arithmetic on the document's one calendar: Monday to Friday, 08:00 to 12:00
// and 13:00 to 17:00, so a day is 480 working minutes.
import { readFileSync } from "node:fs";
import { fromBryntum } from "../src/adapters/bryntum";
import { expect, test } from "./support/gantt";

const fixture: unknown = JSON.parse(readFileSync(new URL("./fixtures/bryntum-gantt.json", import.meta.url), "utf8"));

/** The adapter's document for the fixture, or a failure naming why there is none. */
function read(data: unknown = fixture) {
  const result = fromBryntum(data);
  if (!result.ok) throw new Error(`the fixture was refused: ${result.error}`);
  return result;
}

test("the_fixture_loads_into_the_chart_and_comes_back_as_the_same_document", async ({ g }) => {
  const { document } = read();
  await g.open({ n: 20 });
  const result = await g.page.evaluate((doc) => window.gantt!.api!.load({ document: doc }), document);
  expect(result).toEqual({ ok: true });
  expect(await g.page.evaluate(() => window.gantt!.api!.toDocument())).toEqual(document);
});

test("the_loaded_chart_shows_the_dates_of_the_fixture", async ({ g }) => {
  const { document } = read();
  await g.open({ n: 20 });
  await g.page.evaluate((doc) => window.gantt!.api!.load({ document: doc }), document);
  await g.page.evaluate(() => window.gantt!.app.select_all_rows());
  const text = await g.page.evaluate(() => window.gantt!.app.range_text() ?? "");
  const [header, ...lines] = text.split("\n").map((line) => line.split("\t"));
  const [name, start, finish] = ["Task", "Start", "Finish"].map((column) => header!.indexOf(column));
  const day = (cell: string | undefined) => /\d{4}-\d{2}-\d{2}/.exec(cell ?? "")?.[0];
  const dates = Object.fromEntries(lines.map((cells) => [cells[name!], [day(cells[start!]), day(cells[finish!])]]));
  expect(dates["Design"]).toEqual(["2026-01-05", "2026-01-07"]);
  expect(dates["Build"]).toEqual(["2026-01-08", "2026-01-21"]);
  expect(dates["Operate"]).toEqual(["2026-02-02", "2026-02-06"]);
});

test("a_task_pinned_to_a_date_opens_with_the_pin_and_no_error", async ({ g }) => {
  const { document } = read({ tasks: [{ id: 1, name: "Pinned", duration: 2, constraintType: "muststarton", constraintDate: "2026-01-07T08:00:00" }], dependencies: [] });
  expect(document.tasks[0]).toMatchObject({ snet: null, limit: { pin_start: 2 * 1440 + 480 } });
  await g.open({ n: 20 });
  await g.page.evaluate(() => {
    const w = window as unknown as { __errors: unknown[] };
    w.__errors = [];
    window.gantt!.on("error", (detail: unknown) => w.__errors.push(detail));
  });
  const result = await g.page.evaluate((doc) => window.gantt!.api!.load({ document: doc }), document);
  expect(result).toEqual({ ok: true });
  expect(await g.page.evaluate(() => window.gantt!.api!.toDocument())).toEqual(document);
  expect(await g.page.evaluate(() => (window as unknown as { __errors: unknown[] }).__errors)).toEqual([]);
});
