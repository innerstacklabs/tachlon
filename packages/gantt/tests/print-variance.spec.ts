import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// The printed columns follow the screen (gantt#1747, Part of #703): the Variance button's hide, the Progress columns right after Task, and the
// picture `exportImage` saves. The spec fires `beforeprint` itself, so no print dialog opens.

/** The column heads page 1 prints, from the first Task head on: the list cells that share the first cell's baseline are the head row. */
const heads = async (page: Page): Promise<string[]> => {
  await page.evaluate(() => window.dispatchEvent(new Event("beforeprint")));
  const row = await page
    .locator(".gantt-print-pages > svg")
    .first()
    .evaluate((svg) => {
      const cells = [...svg.querySelectorAll("g[clip-path] > text")];
      const y = cells[0]!.getAttribute("y");
      return cells.filter((cell) => cell.getAttribute("y") === y).map((cell) => cell.textContent ?? "");
    });
  await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  return row.slice(row.indexOf("Task"));
};

test("Variance off prints no variance column, Progress on prints its four right after Task, and the PNG asks for the same columns", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 200 });
  // The app leaves out a progress column the file has no progress for: finish one leaf.
  await page.evaluate(() => {
    const leaf = document.querySelector<HTMLElement>("#table-rows .row[data-kind='0']")!;
    window.gantt!.app.set_progress(Number(leaf.dataset.task), "2026-10-05", "2026-10-05", "");
  });
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);

  const on = await heads(page);
  expect(on, "the variance columns follow Float, every head whole").toEqual(["Task", "Dur", "Start", "Finish", "Float", "Start var.", "Finish var.", "Float var."]);

  await page.locator("#variance-toggle").click();
  await g.nextFrame(2);
  expect(await heads(page), "the Variance button's hide reaches the page").toEqual(["Task", "Dur", "Start", "Finish", "Float"]);

  await page.locator("#progress-toggle").click();
  await g.nextFrame(2);
  const progress = ["% complete", "Actual start", "Actual finish", "Remaining"];
  expect(await heads(page), "the progress columns come after Task, as on screen, and every head prints whole").toEqual(["Task", ...progress, "Dur", "Start", "Finish", "Float"]);

  // The picture is page 1 of the same print: spy on the call `exportImage` makes.
  const asked = await page.evaluate(async () => {
    const app = window.gantt!.app;
    const real = app.print_pages.bind(app);
    let columns: unknown;
    app.print_pages = (...args: Parameters<typeof real>) => {
      columns = args[4];
      return real(...args);
    };
    await window.gantt!.api!.exportImage();
    return columns;
  });
  expect(asked, "exportImage passes the columns the screen shows").toEqual(["ID", "WBS", "Task", ...progress, "Dur", "Start", "Finish", "Float"]);
});
