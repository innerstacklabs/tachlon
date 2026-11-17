import { expect, test } from "./support/gantt";

// `exportImage({ page })` (gantt#1875): the picture is the printed page asked for, named for it, and a page that does not exist rejects with no download.
test("exportImage_takes_a_page_number_and_rejects_a_page_that_does_not_exist", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 200 });
  const downloads: string[] = [];
  page.on("download", (download) => downloads.push(download.suggestedFilename()));
  // `pagePng` draws the SVG it is handed through a blob URL: keep the text of every such blob.
  await page.evaluate(() => {
    const drawn: Promise<string>[] = [];
    (window as unknown as { drawn: Promise<string>[] }).drawn = drawn;
    const real = URL.createObjectURL.bind(URL);
    URL.createObjectURL = (blob: Blob | MediaSource) => {
      if (blob instanceof Blob && blob.type === "image/svg+xml") drawn.push(blob.text());
      return real(blob);
    };
  });
  const drawnPages = () => page.evaluate(async () => (await Promise.all((window as unknown as { drawn: Promise<string>[] }).drawn)).map((svg) => /Page (\d+) of (\d+)/.exec(svg)?.[0]));
  const exportImage = (options?: { page?: number }) =>
    page.evaluate((o) => window.gantt!.api!.exportImage(o).then(() => "downloaded", (error: Error) => error.message), options);

  expect(await exportImage({ page: 2 })).toBe("downloaded");
  expect(await drawnPages(), "the picture is page 2 of the print").toEqual([expect.stringMatching(/^Page 2 of \d+$/)]);
  await expect.poll(() => downloads).toEqual(["untitled-page-2.png"]);

  expect(await exportImage()).toBe("downloaded");
  await expect.poll(() => downloads, "no argument still downloads page 1 as <name>.png").toEqual(["untitled-page-2.png", "untitled.png"]);

  for (const bad of [0, 9999]) {
    expect(await exportImage({ page: bad }), `page ${bad} rejects, naming the page count`).toMatch(new RegExp(`page ${bad} is not one of the \\d+ printed pages`));
  }
  expect(await drawnPages(), "a rejected page draws nothing").toHaveLength(2);
  expect(downloads, "a rejected page downloads nothing").toHaveLength(2);
});

// `printedPages()` (gantt#2003): a host reads the page count instead of calling until a page rejects, and the error says the count in the singular and quotes a value that is not a whole number.
test("printedPages_counts_the_printed_pages_and_exportImage_names_a_bad_page_right", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 3 });
  const exportImage = (options: { page?: unknown }) =>
    page.evaluate((o) => window.gantt!.api!.exportImage(o as { page?: number }).then(() => "downloaded", (error: Error) => error.message), options);

  expect(await page.evaluate(() => window.gantt!.api!.printedPages()), "three tasks print on one page").toBe(1);
  expect(await exportImage({ page: 2 })).toBe("exportImage: page 2 is not one of the 1 printed page");
  expect(await exportImage({ page: 1.5 }), "a non-whole number rejects, quoted").toBe(`exportImage: page "1.5" is not one of the 1 printed page`);
  expect(await exportImage({ page: "2" }), "a string rejects, quoted").toBe(`exportImage: page "2" is not one of the 1 printed page`);

  await g.open({ n: 200 });
  const count = await page.evaluate(() => window.gantt!.api!.printedPages());
  expect(count, "a long schedule prints on more than one page").toBeGreaterThan(1);
  expect(await exportImage({ page: count + 1 })).toBe(`exportImage: page ${count + 1} is not one of the ${count} printed pages`);
});
