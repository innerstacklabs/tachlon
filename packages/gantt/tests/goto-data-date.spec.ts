import { readFileSync } from "node:fs";
import { expect, test, type Gantt } from "./support/gantt";

// `t` goes to the data date (gs#703): with the chart focused and the cell cursor not in the list, `t` does what the find
// box's date jump does for the data date's day, and with no data date it does nothing. The oracle is the position
// app.go_to_date gives for that same day, read as go-to-date.spec.ts reads it.

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

/** The view the specs compare: the left edge, the right edge, the zoom and the scroll. */
async function view(g: Gantt): Promise<{ t: [number, number]; scale: number; scrollY: number }> {
  const s = await g.status();
  return { t: s.t, scale: s.pxPerMinute, scrollY: s.scrollY };
}

async function goTo(g: Gantt, day: string): Promise<void> {
  expect(await g.page.evaluate((text) => window.gantt!.app.go_to_date(text), day)).toBe(true);
  await g.nextFrame(3);
}

test("t pans to the data date and does nothing without one", async ({ g }) => {
  await g.open({ n: 200 });
  expect((await g.status()).dataDate, "the demo has no data date").toBeNull();
  await g.page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: XER });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  expect(await g.page.evaluate(() => window.gantt!.app.data_date_text())).toBe("2024-01-10 08:00");

  // The tooltip names the key: hovering the data date teaches `t`.
  await expect(g.page.locator("#data-date")).toHaveAttribute("title", "Data date 2024-01-10 08:00. Click to change it; press T to go to it.");

  // The oracle: where the date jump puts the data date's day.
  await goTo(g, "2024-01-10");
  const want = await view(g);

  await goTo(g, "2030-01-01");
  expect(await view(g), "moved away").not.toEqual(want);
  await g.page.keyboard.press("t");
  await g.nextFrame(3);
  const got = await view(g);
  expect(got.t[0], "left edge").toBeCloseTo(want.t[0], 3);
  expect(got.t[1], "right edge").toBeCloseTo(want.t[1], 3);
  expect(got.scale, "the zoom stays").toBe(want.scale);
  expect(got.scrollY, "the first task on or after the day is on top").toBe(want.scrollY);

  // No data date: open the demo again, and t leaves the view where it is.
  await g.open({ n: 200 });
  expect((await g.status()).dataDate).toBeNull();
  await goTo(g, "2026-03-02");
  const before = await view(g);
  await g.page.keyboard.press("t");
  await g.nextFrame(3);
  expect(await view(g), "no data date: t does nothing").toEqual(before);
});
