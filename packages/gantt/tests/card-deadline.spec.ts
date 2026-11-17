import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// The hover card's Deadline row: shown after Finish for a task with an MS Project Deadline, with the days the task misses it by; a task with none shows no row.

/** From Monday 2026-01-05 08:00: Missed works five days (finish Friday 12:00) against a Wednesday 12:00 deadline, Met one day against a later one, Open has none. */
const task = (uid: number, name: string, hours: number, deadline: string): string =>
  `<Task><UID>${uid}</UID><Name>${name}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT${hours}H0M0S</Duration><DurationFormat>7</DurationFormat>${deadline}</Task>`;
const FILE = Buffer.from(
  `<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-01-05T08:00:00</StartDate><Tasks>` +
    task(1, "Missed", 40, "<Deadline>2026-01-07T12:00:00</Deadline>") + task(2, "Met", 8, "<Deadline>2026-01-12T12:00:00</Deadline>") + task(3, "Open", 8, "") +
    `</Tasks></Project>`,
);

/** The card's value for `label`, or null when it has no such row. */
const valueOf = (page: Page, label: string): Promise<string | null> =>
  page.evaluate((name) => {
    const cells = [...document.querySelectorAll<HTMLElement>("#hover-card .details > *")];
    const at = cells.findIndex((el) => el.textContent === name);
    return at < 0 ? null : (cells[at + 1]?.textContent ?? null);
  }, label);

/** Hover the bar of the leaf whose table row holds `name`, until the card is for it. */
async function hover(g: Gantt, name: string): Promise<void> {
  const { row, task } = await g.page.evaluate((label) => {
    const leaves = [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")];
    const el = leaves.find((r) => [...r.children].some((cell) => cell.textContent === label));
    if (el === undefined) throw new Error(`no row named ${label}`);
    return { row: Number(el.dataset.row), task: Number(el.dataset.task) };
  }, name);
  const [s, canvas] = [await g.status(), await g.box()];
  const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;
  for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3; x += 4) {
    await g.page.mouse.move(x, y);
    await g.nextFrame(1);
    if ((await g.status()).hover?.task === task) return expect(g.page.locator("#hover-card")).toBeVisible();
  }
  throw new Error("no bar of the task under the pointer");
}

test("the_card_shows_the_deadline_and_the_days_a_task_misses_it_by", async ({ page, g }) => {
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "deadlines.xml", mimeType: "text/xml", buffer: FILE });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  await hover(g, "Missed");
  expect(await valueOf(page, "Finish")).toBe("2026-01-09 12:00");
  expect(await valueOf(page, "Deadline")).toBe("2026-01-07 12:00 (missed by 2d)");
  await hover(g, "Met");
  expect(await valueOf(page, "Deadline"), "in time: the date alone").toBe("2026-01-12 12:00");
  await hover(g, "Open");
  expect(await valueOf(page, "Deadline"), "no deadline, no row").toBeNull();
});
