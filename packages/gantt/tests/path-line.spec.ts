import { expect, openFilters, test } from "./support/gantt";

// With Longest path on, a line at the canvas's bottom left names the path (gantt#703): the status's `path`, which the Rust test holds is the longest path's.
const number = new Intl.NumberFormat("en-US");

test("with Longest path on, a line names the path at the canvas's bottom left", async ({ g }) => {
  await g.open({ n: 300 });
  const line = g.page.locator("#path-line");
  await expect(line, "no filter: no line").toBeHidden();
  await openFilters(g.page);
  await g.page.locator("#longest-toggle").click();
  await g.nextFrame(3);
  const { first, last, count, days } = (await g.status()).path!;
  await expect(line).toHaveText(`Longest path: ${first} to ${last} · ${number.format(count)} activities · ${number.format(days)} days`);
  const [chip, canvas] = [(await line.boundingBox())!, (await g.page.locator("#timeline").boundingBox())!];
  expect(Math.max(chip.x - canvas.x, canvas.y + canvas.height - chip.y - chip.height), "in the bottom-left corner").toBeLessThan(16);
  await g.page.locator("#longest-toggle").click();
  await g.nextFrame(3);
  await expect(line, "Longest path off: no line").toBeHidden();
});
