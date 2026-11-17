// gantt#1875: in a right-to-left chart the table scrolled sideways marks itself scrolled, hides the column the frozen block cuts, and clears the "more columns" mark at its end.
import { expect, test } from "./support/gantt";

test("in a right-to-left chart a sideways scroll sets data-scrolled, clears data-more at its end and hides the cut column", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 200 });
  await page.evaluate(() => document.getElementById("app")!.setAttribute("dir", "rtl"));
  if ((await page.locator("#table").getAttribute("data-progress")) !== "true") await page.click("#progress-toggle");
  const at = (to: number) =>
    page.evaluate(async (left) => {
      const body = document.getElementById("table-body")!;
      body.scrollLeft = left;
      await new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done())));
      const table = document.getElementById("table")!;
      const edge = document.querySelector("#table-head .cols > .name")!.getBoundingClientRect().left;
      let cut = 0;
      let hidden = 0;
      for (const cell of document.querySelectorAll<HTMLElement>("#table-head .cols > *, #table-rows .row > *")) {
        const box = cell.getBoundingClientRect();
        if (cell.matches(".name, .code, .wbs") || !(box.left < edge && edge < box.right)) continue;
        if (getComputedStyle(cell).opacity === "0") hidden += 1;
        else cut += 1;
      }
      return { at: body.scrollLeft, end: body.clientWidth - body.scrollWidth, scrolled: table.dataset.scrolled, more: table.dataset.more, cut, hidden };
    }, to);
  const first = await at(0);
  expect(first.end, "the table scrolls sideways").toBeLessThan(-100);
  expect(first, "at rest: not scrolled, columns to come").toMatchObject({ scrolled: undefined, more: "true" });
  expect(await at(-100), "scrolled to -100").toMatchObject({ scrolled: "true", more: "true", cut: 0 });
  let hidden = 0;
  for (let left = -1; left >= first.end; left -= 7) {
    const r = await at(left);
    expect(r.cut, `visible cells cut by the frozen edge at scrollLeft ${r.at}`).toBe(0);
    hidden += r.hidden;
  }
  expect(hidden, "the scan put a column under the frozen edge").toBeGreaterThan(0);
  expect(await at(first.end), "scrolled to its end").toMatchObject({ scrolled: "true", more: undefined, cut: 0 });
});
