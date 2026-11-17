// The DHTMLX data adapter (gs#1097): the fixture is DHTMLX Gantt JSON (tests/fixtures/dhtmlx.json). It loads into the chart as
// a document and the chart gives back its tasks, links, dates and durations. What the spec expects is worked out here from the
// fixture's own text, not by calling the adapter, so a mistake in the adapter cannot agree with itself.
import { readFileSync } from "node:fs";
import { fromDhtmlx } from "../src/adapters/dhtmlx";
import { expect, test } from "./support/gantt";

interface FixtureTask {
  id: number;
  text: string;
  type?: string;
  start_date?: string;
  duration?: number;
  progress?: number;
  parent?: number;
}
interface FixtureLink {
  source: number;
  target: number;
  type: string;
  lag?: number;
}
const fixture = JSON.parse(readFileSync(new URL("./fixtures/dhtmlx.json", import.meta.url), "utf8")) as { data: FixtureTask[]; links: FixtureLink[] };
const SUMMARIES = [1, 6];
const KIND: Record<string, string> = { "0": "FS", "1": "SS", "2": "FF", "3": "SF" };
const DAY_MINUTES = 480;

/** `DD-MM-YYYY[ HH:mm]` as `YYYY-MM-DD HH:mm`. */
function isoOf(text: string): string {
  const [date = "", time = "00:00"] = text.split(" ");
  const [day, month, year] = date.split("-");
  return `${year}-${month}-${day} ${time}`;
}

/** The wall-clock `YYYY-MM-DD HH:mm` of `snet` minutes after the origin day (days since 1970-01-01). */
function stampOf(origin: number, snet: number): string {
  return new Date((origin * 1440 + snet) * 60_000).toISOString().slice(0, 16).replace("T", " ");
}

test("a_dhtmlx_file_loads_and_comes_back_with_its_tasks_links_dates_and_durations", async ({ g }) => {
  const { document } = fromDhtmlx(fixture, { name: "fit-out" });
  await g.open({ n: 20 });
  const result = await g.page.evaluate((doc) => window.gantt!.api!.load({ document: doc }), document);
  expect(result).toEqual({ ok: true });
  expect((await g.status()).n).toBe(fixture.data.length);
  const back = (await g.page.evaluate(() => window.gantt!.api!.toDocument())) as typeof document;
  expect(back, "the chart holds exactly what the adapter wrote").toEqual(document);

  const byCode = new Map(back.tasks.map((task, at) => [task.code, { task, at }]));
  expect(byCode.size).toBe(fixture.data.length);
  for (const source of fixture.data) {
    const { task } = byCode.get(String(source.id))!;
    expect(task.name).toBe(source.text);
    const parent = task.parent === null ? undefined : back.tasks[task.parent]!.code;
    expect(parent, `parent of ${source.text}`).toBe(source.parent === undefined || source.parent === 0 ? undefined : String(source.parent));
    if (SUMMARIES.includes(source.id)) continue;
    expect(task.duration, `duration of ${source.text}`).toBe((source.duration ?? 0) * DAY_MINUTES);
    expect(task.progress, `progress of ${source.text}`).toBe(Math.round((source.progress ?? 0) * 100));
    expect(stampOf(back.origin, task.snet!), `start of ${source.text}`).toBe(isoOf(source.start_date!));
  }

  const carried = fixture.links.filter((link) => !SUMMARIES.includes(link.source) && !SUMMARIES.includes(link.target));
  const read = back.links.map((link) => ({ source: back.tasks[link.pred]!.code, target: back.tasks[link.succ]!.code, kind: link.kind, lag: link.lag }));
  expect(read).toEqual(carried.map((link) => ({ source: String(link.source), target: String(link.target), kind: KIND[link.type], lag: (link.lag ?? 0) * DAY_MINUTES })));

  const details = await g.page.evaluate((at) => window.gantt!.app.task_details(at) as { start: string; finish: string }, byCode.get("2")!.at);
  expect(details.start, "the first task starts on its date, at the start of the working day").toBe("2027-03-01 08:00");
});
