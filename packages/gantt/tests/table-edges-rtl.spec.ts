// gantt#1875: in a right-to-left chart the frozen block's hairline and gap cover fall on the left, and the "more columns" fade sits at the left end of the pane.
import { expect, test } from "./support/gantt";

test("in a right-to-left chart the frozen block's shadows have a negative offset and the fade sits at the left", async ({ g }) => {
  await g.open({ n: 20 });
  const edges = await g.page.evaluate(() => {
    const table = document.getElementById("table")!;
    for (const mark of ["scrolled", "more", "progress"]) table.dataset[mark] = "true";
    document.getElementById("app")!.setAttribute("dir", "rtl");
    // The demo schedule has no activity IDs, so no ID cell exists: add the one the stylesheet names.
    const row = document.querySelector("#table-rows .row")!;
    row.append(Object.assign(document.createElement("span"), { className: "code" }));
    const x = (sel: string) => getComputedStyle(row.querySelector(sel)!).boxShadow.match(/(-?[\d.]+)px/)?.[1];
    return { name: x(".name"), code: x(".code"), fade: getComputedStyle(table, "::after").left };
  });
  expect(Number(edges.name), "the hairline offset").toBeLessThan(0);
  expect(Number(edges.code), "the gap cover offset").toBeLessThan(0);
  expect(edges.fade, "the fade sits at the left").toBe("0px");
});
