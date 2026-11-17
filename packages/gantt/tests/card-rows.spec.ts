import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// The hover card's Status, actual dates and neighbour lines (gs#703): a finished leaf reads Status Complete then Actual start and
// Actual finish after Critical; a leaf not started reads Not started and none of the actuals or % complete; Predecessors and
// Successors show up to three lines, then "+ N more" only when there are more, and "—" when there are none. The spec reads the card's DOM.

const fixture = (name: string): Buffer => readFileSync(new URL(`./fixtures/${name}`, import.meta.url));

const labels = (page: Page): Promise<string[]> =>
  page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#hover-card .details > :nth-child(odd)")].map((el) => el.textContent ?? ""));

const valueOf = (page: Page, label: string): Promise<string | null> =>
  page.evaluate(
    (name) => {
      const cells = [...document.querySelectorAll<HTMLElement>("#hover-card .details > *")];
      const at = cells.findIndex((el) => el.textContent === name);
      return at < 0 ? null : (cells[at + 1]?.textContent ?? null);
    },
    label,
  );

/** The text of the card's label that starts with `prefix`. */
const labelStarting = async (page: Page, prefix: string): Promise<string | undefined> => (await labels(page)).find((l) => l.startsWith(prefix));

/** The drawn leaf rows whose card status is `status`, or whose task name ends with `name`: the first one's row and task. */
const findRow = (page: Page, want: { status: string } | { name: string }): Promise<{ row: number; task: number }> =>
  page.evaluate((w) => {
    for (const el of document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")) {
      const task = Number(el.dataset.task);
      const app = window.gantt!.app;
      const hit = "status" in w ? (app.task_details(task) as { status: string }).status === w.status : app.task_name(task).split(" / ").at(-1) === w.name;
      if (hit) return { row: Number(el.dataset.row), task };
    }
    throw new Error("no such row");
  }, want);

/** Move the pointer along the row until the card is for `task`. */
async function hoverTask(g: Gantt, row: number, task: number): Promise<void> {
  const { page } = g;
  const s = await g.status();
  const canvas = await g.box();
  const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;
  await page.mouse.move(canvas.x + 1, y);
  for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3; x += 4) {
    await page.mouse.move(x, y);
    await g.nextFrame(1);
    if ((await g.status()).hover?.task === task) {
      await expect(page.locator("#hover-card")).toBeVisible();
      return;
    }
  }
  throw new Error("no bar of the task under the pointer");
}

/** Open a file from the page and wait for the new schedule to be drawn. */
async function openFile(g: Gantt, name: string, mimeType: string, buffer: Buffer = fixture(name)): Promise<void> {
  await g.open({ n: 50 });
  await g.page.setInputFiles("#open-input", { name, mimeType, buffer });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
}

test("a finished leaf of a P6 file reads its activity ID, Status Complete, then Actual start and Actual finish after Critical", async ({ g }) => {
  const { page } = g;
  await openFile(g, "one-project.xer", "application/octet-stream");
  const { row, task } = await findRow(page, { status: "Complete" });
  const activityId = await page.evaluate((t) => (window.gantt!.app.task_details(t) as { activityId: string }).activityId, task);
  expect(activityId, "the fixture's task has an activity ID").not.toBe("");

  await hoverTask(g, row, task);
  const title = await page.locator("#hover-card .title").textContent();
  expect(title?.startsWith(`${activityId} `), "the title starts with the activity ID").toBe(true);
  const got = await labels(page);
  const critical = got.indexOf("Critical");
  expect(critical).toBeGreaterThanOrEqual(0);
  expect(got.slice(critical, critical + 4)).toEqual(["Critical", "Status", "Actual start", "Actual finish"]);
  expect(await valueOf(page, "Status")).toBe("Complete");
});

test("a leaf not started on the generated demo reads Not started and has no actuals or % complete", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 200 });
  const { row, task } = await findRow(page, { status: "Not started" });
  await hoverTask(g, row, task);
  const got = await labels(page);
  expect(await valueOf(page, "Status")).toBe("Not started");
  expect(got).not.toContain("Actual start");
  expect(got).not.toContain("Actual finish");
  expect(got).not.toContain("% complete");
});

test("a task with five predecessors reads Predecessors (5), three lines, + 2 more, and a lag line ends (SS+2d)", async ({ g }) => {
  const { page } = g;
  await openFile(g, "fan-in.xml", "application/xml");
  const { row, task } = await findRow(page, { name: "Fan" });
  await hoverTask(g, row, task);
  expect(await labelStarting(page, "Predecessors")).toBe("Predecessors (5)");
  const lines = (await valueOf(page, "Predecessors (5)"))!.split("\n");
  expect(lines, "three lines then the rest counted").toHaveLength(4);
  expect(lines[3]).toBe("+ 2 more");
  expect(lines.some((line) => line.endsWith("(SS+2d)")), `one line ends (SS+2d): ${JSON.stringify(lines)}`).toBe(true);
});

test("a task with no links reads — for Predecessors and for Successors", async ({ g }) => {
  const { page } = g;
  await openFile(g, "fan-in.xml", "application/xml");
  const { row, task } = await findRow(page, { name: "Solo" });
  await hoverTask(g, row, task);
  expect(await valueOf(page, "Predecessors (0)")).toBe("—");
  expect(await valueOf(page, "Successors (0)")).toBe("—");
});

test("a task with exactly two predecessors reads Predecessors (2), two lines and no + N more", async ({ g }) => {
  const { page } = g;
  await openFile(g, "fan-in.xml", "application/xml");
  const { row, task } = await findRow(page, { name: "Pair" });
  await hoverTask(g, row, task);
  expect(await labelStarting(page, "Predecessors")).toBe("Predecessors (2)");
  const lines = (await valueOf(page, "Predecessors (2)"))!.split("\n");
  expect(lines).toHaveLength(2);
  expect(lines.some((line) => line.startsWith("+")), `no "+ N more" line: ${JSON.stringify(lines)}`).toBe(false);
});

// Three tasks (gs#703): Five has five resources, Two has two, None has none.
const resourceProject = (): Buffer => {
  const task = (uid: number, name: string): string => `<Task><UID>${uid}</UID><Name>${name}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>`;
  const crews = ["A", "B", "C", "D", "E"].map((c, i) => `<Resource><UID>${i + 1}</UID><Name>Crew ${c}</Name></Resource>`).join("");
  const assign = (task: number, count: number): string => Array.from({ length: count }, (_, i) => `<Assignment><TaskUID>${task}</TaskUID><ResourceUID>${i + 1}</ResourceUID></Assignment>`).join("");
  const xml = `<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-01-05T08:00:00</StartDate><Tasks>${task(1, "Five")}${task(2, "Two")}${task(3, "None")}</Tasks><Resources>${crews}</Resources><Assignments>${assign(1, 5)}${assign(2, 2)}</Assignments></Project>`;
  return Buffer.from(xml);
};

test("the card lists a task's resource names, the first three then +N, and no row for a task with none", async ({ g }) => {
  const { page } = g;
  await openFile(g, "resources.xml", "application/xml", resourceProject());
  for (const [name, want] of [["Five", "Crew A, Crew B, Crew C, +2"], ["Two", "Crew A, Crew B"], ["None", null]] as const) {
    const { row, task } = await findRow(page, { name });
    await hoverTask(g, row, task);
    expect(await valueOf(page, "Resources"), `the Resources row of ${name}`).toBe(want);
  }
});
