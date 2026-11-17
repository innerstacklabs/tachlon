import { expect, test, type Gantt } from "./support/gantt";

// The level keys: `1` to `9` collapse every summary to that outline level, `0` expands all. A four-level file tells `3` from `0`:
// Top (1) holds A (2) and B (2); A holds A1 (3, a summary) and A2 (3); A1 holds X and Y (4).
const task = (uid: number, name: string, level: number, summary: boolean): string =>
  `<Task><UID>${uid}</UID><Name>${name}</Name><OutlineLevel>${level}</OutlineLevel><Summary>${summary ? 1 : 0}</Summary><Duration>PT${summary ? 0 : 8}H0M0S</Duration><DurationFormat>7</DurationFormat></Task>`;
const FILE = Buffer.from(
  `<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-01-05T08:00:00</StartDate><Tasks>` +
    [task(1, "Top", 1, true), task(2, "A", 2, true), task(3, "A1", 3, true), task(4, "X", 4, false), task(5, "Y", 4, false), task(6, "A2", 3, false), task(7, "B", 2, false)].join("") +
    `</Tasks></Project>`,
);

async function openFourLevels(g: Gantt): Promise<void> {
  await g.open({ n: 50 });
  await g.page.setInputFiles("#open-input", { name: "levels.xml", mimeType: "application/xml", buffer: FILE });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.evaluate(() => document.getElementById("app")!.focus());
}

const rows = async (g: Gantt): Promise<number> => (await g.status()).rowCount;

test("the_key_3_collapses_to_level_3", async ({ g }) => {
  await openFourLevels(g);
  expect(await rows(g), "the file has seven rows").toBe(7);
  await g.page.keyboard.press("3");
  await g.nextFrame(3);
  expect(await rows(g), "level 3 hides the two level-4 tasks and keeps the rest").toBe(5);
});

test("the_keys_4_to_9_show_every_row_of_a_four_level_file", async ({ g }) => {
  await openFourLevels(g);
  for (const key of ["4", "5", "6", "7", "8", "9"]) {
    await g.page.keyboard.press("2");
    await g.nextFrame(3);
    expect(await rows(g), `after 2, before ${key}`).toBe(3);
    await g.page.keyboard.press(key);
    await g.nextFrame(3);
    expect(await rows(g), `level ${key} shows all seven rows`).toBe(7);
  }
});
