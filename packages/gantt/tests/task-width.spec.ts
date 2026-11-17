// The Task column is as wide as the file's longest name needs, not a fixed 280 px (gs#959, ruled by karpathy): measured once per opened
// file from the longest name at each outline level with its indent and chevron, capped so Dur, Start and Finish stay whole. Both
// files are MS Project XML built here at 1280 px: one whose longest name needs well over 280 px, one that needs about 150 px.
import { expect, test, type Gantt } from "./support/gantt";

/** A project with one summary and these leaf names under it. */
function project(names: string[]): Buffer {
  const task = (uid: number, name: string, level: number, summary: boolean): string =>
    `<Task><UID>${uid}</UID><ID>${uid}</ID><Name>${name}</Name><OutlineLevel>${level}</OutlineLevel><Summary>${summary ? 1 : 0}</Summary>` +
    `<Milestone>0</Milestone><Duration>PT${summary ? 0 : 40}H0M0S</Duration><PercentComplete>${summary ? 0 : 50}</PercentComplete></Task>`;
  const tasks = [task(1, "Works", 1, true), ...names.map((name, i) => task(i + 2, name, 2, false))].join("");
  return Buffer.from(
    `<?xml version="1.0" encoding="UTF-8"?><Project xmlns="http://schemas.microsoft.com/project"><Name>Width</Name>` +
      `<StartDate>2027-01-04T08:00:00</StartDate><Tasks>${tasks}</Tasks></Project>`,
  );
}

async function openAt1280(g: Gantt, buffer: Buffer): Promise<void> {
  await g.page.setViewportSize({ width: 1280, height: 800 });
  await g.open({ n: 200 });
  await g.page.setInputFiles("#open-input", { name: "width.xml", mimeType: "application/xml", buffer });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
}

test("a file with a long name shows every name whole", async ({ g }) => {
  const longest = "Commission and balance the building management system";
  await openAt1280(g, project(["Site survey", longest, "Handover"]));
  const seen = await g.page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("#table-rows .row > .name")].map((cell) => ({
      text: cell.textContent,
      cut: cell.scrollWidth - cell.clientWidth,
      width: cell.getBoundingClientRect().width,
    })),
  );
  expect(seen.map((cell) => cell.text)).toContain(longest);
  expect(Math.max(...seen.map((cell) => cell.width)), "this name needs well over the old 280 px").toBeGreaterThan(290);
  for (const cell of seen) expect(cell.cut, `"${cell.text}" is cut by`).toBeLessThanOrEqual(0);
});

test("a file with short names leaves Actual start inside the pane", async ({ g }) => {
  await openAt1280(g, project(["Site survey", "Order glazing", "Fit out the lobby"]));
  await g.page.locator("#progress-toggle").evaluate((el) => (el as HTMLElement).click());
  await g.nextFrame(3);
  const seen = await g.page.evaluate(() => {
    const pane = document.getElementById("table-body")!;
    const edge = pane.getBoundingClientRect().left + pane.clientWidth;
    const head = [...document.querySelectorAll<HTMLElement>("#table-head .cols > span")].find((cell) => cell.textContent === "Actual start");
    const name = document.querySelector<HTMLElement>("#table-rows .row > .name")!;
    return {
      progress: document.getElementById("table")!.dataset.progress,
      actualRight: head?.getBoundingClientRect().right ?? Infinity,
      edge,
      task: name.getBoundingClientRect().width,
    };
  });
  expect(seen.progress, "the Progress button turned the columns on").toBe("true");
  expect(seen.actualRight, "Actual start ends inside the pane").toBeLessThanOrEqual(seen.edge);
  expect(seen.task, "short names need far less than the old 280 px").toBeLessThan(220);
});

test("opening a file with short names after one with a long name shrinks Task back", async ({ g }) => {
  await openAt1280(g, project(["Site survey", "Commission and balance the building management system"]));
  const taskWidth = (): Promise<number> => g.page.locator("#table-rows .row > .name").first().evaluate((el) => el.getBoundingClientRect().width);
  const wide = await taskWidth();
  await g.page.setInputFiles("#open-input", { name: "short.xml", mimeType: "application/xml", buffer: project(["Site survey", "Handover"]) });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 2, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.locator("#progress-toggle").evaluate((el) => (el as HTMLElement).click());
  await g.nextFrame(3);
  expect(await taskWidth(), "Task shrinks back to the short names").toBeLessThan(wide - 100);
});

test("a name far past the cap is cut, and Start and Finish stay whole", async ({ g }) => {
  const huge = "Commission, balance and certify the building management system across every floor of the east and west wings, ".repeat(2).slice(0, 150);
  await openAt1280(g, project(["Site survey", huge]));
  const seen = await g.page.evaluate(() => {
    const pane = document.getElementById("table-body")!;
    const finish = [...document.querySelectorAll<HTMLElement>("#table-head .cols > span")].find((cell) => cell.textContent === "Finish");
    const cell = [...document.querySelectorAll<HTMLElement>("#table-rows .row > .name")].find((el) => el.textContent?.startsWith("Commission"));
    return {
      finishRight: finish?.getBoundingClientRect().right ?? Infinity,
      edge: pane.getBoundingClientRect().left + pane.clientWidth,
      cut: cell === undefined ? -1 : cell.scrollWidth - cell.clientWidth,
    };
  });
  expect(seen.finishRight, "Finish ends inside the pane").toBeLessThanOrEqual(seen.edge);
  expect(seen.cut, "the 150-character name is cut").toBeGreaterThan(0);
});
