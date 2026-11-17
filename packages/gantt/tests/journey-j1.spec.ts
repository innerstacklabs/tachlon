// Journey J1 ("J1. Open a P6 XER instantly"), walked on one page, one step per journey step: the
// reader chooses Open with Ctrl+O, picks all the projects of a two-project XER, reads the grid, the timeline and the
// data date, closes the import notes and reads the open time. The detail of each step lives in its own spec (open,
// picker, import-notes, id-column, wbs-column); this one checks that the steps work in order, as a reader meets them.
import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

const FILE = { name: "two-projects.xer", buffer: readFileSync(new URL("./fixtures/two-projects.xer", import.meta.url)) };
// two-projects.xer: projects 1000 ("Café Project 1") and 1001 ("Project 2"), 8 activities each; "all" opens both, each
// as a root summary, a work package and its activities.
const ALL_TASKS = 20;

test("J1: a reader opens a two-project XER with Ctrl+O, picks all, reads the schedule and closes the notes", async ({ g }) => {
  const { page } = g;
  const requests: string[] = [];

  await test.step("1. opens the page and chooses Open with Ctrl+O", async () => {
    await g.open({ n: 300 });
    const before = await g.status();
    expect(before.epoch, "the page starts on its demo").toBe(0);
    expect(before.slide.copyMs, "no draw-in has run yet").toBeNull();
    expect(await page.evaluate(() => window.gantt!.opened)).toBeUndefined();
    page.on("request", (request) => requests.push(`${request.method()} ${request.url()}`));

    await page.focus("#app");
    const chooser = page.waitForEvent("filechooser");
    await page.keyboard.press("Control+o");
    await (await chooser).setFiles({ name: FILE.name, mimeType: "application/octet-stream", buffer: FILE.buffer });
  });

  await test.step("2. the file holds two projects: picks Open all", async () => {
    await expect(page.locator("#picker")).toBeVisible();
    await expect(page.locator("#picker-title")).toHaveText("two-projects.xer holds 2 projects");
    expect((await g.status()).epoch, "nothing opens before the choice").toBe(0);
    await page.locator('#picker-list button[data-project="all"]').click();
    await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
    await g.nextFrame(3);
    await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

    await expect(page.locator("#picker")).toBeHidden();
    expect(await page.evaluate(() => document.activeElement?.id), "focus returns to the chart").toBe("app");
    const s = await g.status();
    expect(s.n, "both projects are open").toBe(ALL_TASKS);
    expect(s.rowCount).toBe(ALL_TASKS);
    expect(s.slide.copyMs, "the open drew the bars in").not.toBeNull();
    expect(s.slide.copyMs!).toBeGreaterThanOrEqual(0);
    const opened = await page.evaluate(() => window.gantt!.opened!);
    expect(opened).toMatchObject({ name: FILE.name, format: "xer", tasks: ALL_TASKS, bytes: FILE.buffer.length });
    expect(typeof opened.probeMs, "the first pass, which read the project list, is timed apart").toBe("number");
    await expect(page.locator("#open-info")).toContainText(FILE.name);
    await expect(page.locator("#open-error")).toBeHidden();
  });

  await test.step("3. reads the schedule: the grid on the left, the timeline fitted to the project on the right, critical work in red, the data date", async () => {
    const head = await page.locator("#table-head .cols > span").allTextContents();
    for (const column of ["ID", "WBS", "Task", "Start", "Finish", "Float"]) expect(head, `the grid has a ${column} column`).toContain(column);
    const names = await page.locator("#table-rows .row .name").allTextContents();
    expect(names.some((name) => name.includes("Café Project 1")), "the first project is in the grid").toBe(true);
    expect(names.some((name) => name.includes("Project 2")), "the second project is in the grid").toBe(true);
    expect(await page.locator("#table-rows .row .code").first().textContent(), "activities show their IDs").not.toBeNull();

    const table = (await page.locator("#table-body").boundingBox())!;
    const timeline = await g.box();
    expect(table.x + table.width, "the grid is left of the timeline").toBeLessThanOrEqual(timeline.x + 1);

    const s = await g.status();
    expect(s.bars, "the timeline draws bars").toBeGreaterThan(0);
    // The view is already the project's span: the Fit button, which fits the view to the project, moves nothing.
    await page.locator("#fit-button").click();
    await g.nextFrame(3);
    expect((await g.status()).t, "the open fitted the view to the project").toEqual(s.t);

    // Critical activities are red: a red bar on the canvas, and a red float and diamond in the grid.
    expect(s.criticalBars, "critical bars are drawn").toBeGreaterThan(0);
    const critical = page.locator("#table-rows .row.critical").first();
    await expect(critical).toBeVisible();
    const float = await critical.locator(".float").evaluate((cell) => getComputedStyle(cell).color);
    const [r, gr, b] = float.match(/\d+/g)!.map(Number);
    expect(r! > gr! + 60 && r! > b! + 60, `the critical float is red (${float})`).toBe(true);

    expect(s.dataDate, "the file's data date is read").not.toBeNull();
    expect(s.dataDateX, "the data date is a line on the timeline").not.toBeNull();
    expect(s.dataDateX!).toBeGreaterThan(0);
    expect(s.dataDateX!).toBeLessThan(s.surface[0]);
    await expect(page.locator("#data-date")).toContainText("Data date");
  });

  await test.step("4. lists what was not imported, then closes the list", async () => {
    const notice = page.locator("#import-notes");
    await expect(notice).toHaveText(/^\d+ import notes?$/);
    await notice.click();
    const panel = page.locator("#unsupported");
    await expect(panel).toBeVisible();
    const notes = await page.evaluate(() => window.gantt!.unsupported!);
    expect(notes.length, "the file has notes").toBeGreaterThan(0);
    expect(await page.locator("#unsupported-list").evaluate((list) => list.textContent!.split("\n"))).toEqual(notes);
    await page.locator("#unsupported-close").click();
    await expect(panel).toBeHidden();
  });

  await test.step("the overlay shows the open time, split into read, parse, compute and first frame", async () => {
    await page.waitForFunction(() => window.gantt!.opened?.timings !== undefined, undefined, { timeout: 60_000 });
    await page.focus("#app");
    await page.keyboard.press("s");
    const line = page.locator("#app #status-line");
    await expect(line).toContainText(`open ${FILE.name}`);
    for (const phase of ["read", "parse", "compute", "first frame"]) await expect(line).toContainText(`${phase} `);
    await page.keyboard.press("s");
    expect(requests, "the whole journey makes no network request").toEqual([]);
  });
});
