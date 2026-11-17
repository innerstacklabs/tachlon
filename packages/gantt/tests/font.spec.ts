import { expect, test, type Gantt } from "./support/gantt";

// The page ships its own UI font (gs#529): Liberation Sans as "Gantt Sans", from packages/gantt/src/fonts/. `waitReady` (support/gantt.ts)
// already fails a spec whose bundled face did not load; these checks pin the widths that come from that file, so a machine
// with another sans font installed (Loma on the hub, DejaVu Sans on the ThinkPad) gets the same layout.

interface Measured {
  family: string;
  /** Width of the first row's Start text, which is "2026-01-05 07:00" in the 116 px Start column. */
  startText: string;
  startWidth: number;
  /** Width of the same text drawn in the "Gantt Sans" face directly, outside the table. */
  probeWidth: number;
}

async function measure(g: Gantt): Promise<Measured> {
  return g.page.evaluate(() => {
    const row = document.querySelector("#table-rows .row")!;
    const start = row.querySelector<HTMLElement>(".start")!;
    const range = document.createRange();
    range.selectNodeContents(start);
    const style = getComputedStyle(start);
    const probe = document.createElement("span");
    probe.textContent = start.textContent;
    probe.style.cssText = `position: absolute; visibility: hidden; white-space: pre; font: ${style.fontWeight} ${style.fontSize} "Gantt Sans"`;
    document.body.append(probe);
    const probeWidth = probe.getBoundingClientRect().width;
    probe.remove();
    return {
      family: style.fontFamily,
      startText: start.textContent ?? "",
      startWidth: range.getBoundingClientRect().width,
      probeWidth,
    };
  });
}

test("the task list draws in the bundled font, at the width that file gives", async ({ g }) => {
  await g.open({ n: 2000 });
  await g.page.waitForFunction(() => document.querySelectorAll("#table-rows .row").length > 0);
  const m = await measure(g);
  expect(m.family, "the table's font family").toMatch(/^"?Gantt Sans"?,/);
  expect(m.startText).toBe("2026-01-05 07:00");
  // The row's text is as wide as the bundled face makes it. This hub's system sans (Loma) made the same text 115.1 px.
  expect(m.startWidth, "the row uses the bundled face").toBeCloseTo(m.probeWidth, 1);
  // Liberation Sans at the cell's size: 95.4 px in Chromium. Another machine's font would move it by many pixels.
  expect(m.startWidth).toBeGreaterThan(94.4);
  expect(m.startWidth).toBeLessThan(96.4);
});

test("the toolbar does not overflow its strip at 1280 px with the bundled font", async ({ g }) => {
  await g.open({ n: 2000 });
  const over = await g.page.evaluate(() => {
    const toolbar = document.getElementById("toolbar")!;
    return { toolbar: toolbar.scrollWidth - toolbar.clientWidth };
  });
  expect(over.toolbar, "the toolbar overflows its strip by").toBeLessThanOrEqual(0);
});

test("the page body uses the bundled mono font, not a system one", async ({ g }) => {
  await g.open({ n: 2000 });
  const body = await g.page.evaluate(() => getComputedStyle(document.body).fontFamily);
  expect(body, "the body's font family").toMatch(/^"?Gantt Mono"?,/);
});
