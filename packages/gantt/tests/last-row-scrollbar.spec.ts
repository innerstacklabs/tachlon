import { expect, test } from "./support/gantt";
import { chromePath } from "./support/knobs";

// With classic (always-visible) scrollbars, the table's sideways bar takes its height from the body; the last row used to stay partly
// under it at the bottom of the scroll (gs#811). Headless Chromium hides its scrollbars by default, so this file keeps them.
test.use({
  launchOptions: {
    executablePath: chromePath(),
    ignoreDefaultArgs: ["--hide-scrollbars"],
    args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"],
  },
});

test("with classic scrollbars the last row is whole at the bottom of the table", async ({ g }) => {
  await g.page.setViewportSize({ width: 700, height: 600 });
  await g.open({ n: 120 });
  await g.nextFrame(3);
  const seen = await g.page.evaluate(async () => {
    const body = document.getElementById("table-body")!;
    const bar = body.offsetHeight - body.clientHeight;
    const sideways = body.scrollWidth > body.clientWidth;
    body.scrollTop = body.scrollHeight;
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const rows = [...body.querySelectorAll<HTMLElement>("[data-task]")];
    const last = rows.reduce((a, b) => (b.getBoundingClientRect().bottom > a.getBoundingClientRect().bottom ? b : a));
    return {
      bar,
      sideways,
      lastBottom: last.getBoundingClientRect().bottom,
      visibleBottom: body.getBoundingClientRect().top + body.clientHeight,
    };
  });
  expect(seen.sideways, "the columns scroll sideways").toBe(true);
  expect(seen.bar, "the sideways scrollbar takes room").toBeGreaterThan(0);
  expect(seen.lastBottom, "the last row's bottom edge is inside the visible table body").toBeLessThanOrEqual(seen.visibleBottom + 0.5);
});
