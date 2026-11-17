import { readFileSync } from "node:fs";
import { expect, test, type Gantt } from "./support/gantt";

// The task list opens at half of the chart's own box (gs#411, calm look S3; 40% until gs#821 gave the Task column room for the office
// fit-out's names): `--gantt-table-w` is `clamp(420px, 50%, 800px)`, and Start and Finish are 100 px. The percentage is of the host's box, never the window.

const SIZE = 2000;
const HEIGHT = 500;

/** Move the chart root into a host box `width` wide, as another page would place it. */
async function embed(g: Gantt, width: number): Promise<void> {
  await g.page.evaluate(
    ([w, h]) => {
      const host = document.createElement("div");
      host.id = "host";
      host.style.cssText = `position: absolute; left: 0; top: 0; width: ${w}px; height: ${h}px`;
      const root = document.getElementById("app")!;
      document.body.append(host);
      host.append(root);
    },
    [width, HEIGHT] as const,
  );
}

async function resizeHost(g: Gantt, width: number): Promise<void> {
  await g.page.evaluate((w) => {
    document.getElementById("host")!.style.width = `${w}px`;
  }, width);
  await g.nextFrame(2);
}

async function tableWidth(g: Gantt): Promise<number> {
  return g.page.evaluate(() => document.getElementById("table")!.getBoundingClientRect().width);
}

test("the list opens at half of the host's box, between 420 and 800 px", async ({ g }) => {
  await g.open({ n: SIZE });
  await embed(g, 1280);
  await g.nextFrame(2);
  const at1280 = await tableWidth(g);
  expect(at1280, "about 640 px at 1280").toBeGreaterThanOrEqual(635);
  expect(at1280, "about 640 px at 1280").toBeLessThanOrEqual(645);

  // The window is not the box: a wider window leaves the host's list where it was.
  await g.page.setViewportSize({ width: 1600, height: 800 });
  await g.nextFrame(3);
  expect(await tableWidth(g), "the window alone moves nothing").toBeCloseTo(at1280, 0);
  await g.page.setViewportSize({ width: 1280, height: 800 });

  for (const [host, wanted] of [
    [800, 420],
    [1200, 600],
    [1600, 800],
    [2400, 800],
  ] as const) {
    await resizeHost(g, host);
    expect(await tableWidth(g), `a ${host} px host`).toBeCloseTo(wanted, 0);
  }
});

test("no Start or Finish date is cut off in the first 20 rows at 1280", async ({ g }) => {
  await g.open({ n: 50_000 });
  await embed(g, 1280);
  await g.page.waitForFunction(() => document.querySelectorAll("#table-rows .row").length >= 20);
  await g.nextFrame(2);
  const cut = await g.page.evaluate(() => {
    const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row")].slice(0, 20);
    const cells = rows.flatMap((row) => [...row.querySelectorAll<HTMLElement>(".start, .finish")]);
    return {
      count: cells.length,
      cut: cells.filter((cell) => cell.scrollWidth > cell.clientWidth).map((cell) => cell.textContent ?? ""),
    };
  });
  expect(cut.count, "twenty rows of two date cells").toBe(40);
  expect(cut.cut, "no date cell is wider than its column").toEqual([]);
});

test("a long duration such as 7d 2h 30m fits the Dur cell on the ledger fixture", async ({ g }) => {
  await g.open({ n: 20 });
  await g.page.setInputFiles("#open-input", {
    name: "ledger-timeline.xer",
    mimeType: "application/octet-stream",
    buffer: readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url)),
  });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row .dur") !== null);
  await g.nextFrame(3);
  const row = g.page.locator("#table-rows .row .dur").nth(2);
  await row.dblclick();
  await g.page.keyboard.press("Control+A");
  await g.page.keyboard.type("7d 2h 30m");
  await g.page.keyboard.press("Enter");
  await g.nextFrame(3);
  const shown = await g.page.evaluate(() => {
    const cells = [...document.querySelectorAll<HTMLElement>("#table-rows .row .dur")];
    return {
      texts: cells.map((cell) => cell.textContent ?? ""),
      cut: cells.filter((cell) => cell.scrollWidth > cell.clientWidth).map((cell) => cell.textContent ?? ""),
    };
  });
  expect(shown.texts.some((text) => /7d 2h 30m/.test(text)), "the typed duration is shown").toBe(true);
  expect(shown.cut, "no Dur cell is wider than its column").toEqual([]);
});
