// The project picker (gs#87 PR 3): an XER file with several projects asks which to open. Choose one, choose all,
// or cancel; a cancelled picker leaves the schedule on the page exactly as it was.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

const SIZE = 300;
const fixture = (name: string): Buffer => readFileSync(new URL(`./fixtures/${name}`, import.meta.url));
type Fixture = { name: string; buffer: Buffer };
const ONE_PROJECT: Fixture = { name: "one-project.xer", buffer: fixture("one-project.xer") };
const TWO_PROJECTS: Fixture = { name: "two-projects.xer", buffer: fixture("two-projects.xer") };
const MSPDI: Fixture = {
  name: "summary.xml",
  buffer: readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url)),
};

// two-projects.xer: projects 1000 ("Café Project 1") and 1001 ("Project 2"), 8 activities each. Each project
// opens as a root summary, a work package and its activities; "all" opens both (journey-j1.spec.ts).
const PROJECT_TASKS = 10;

async function pickFile(page: Page, file: Fixture): Promise<void> {
  await page.setInputFiles("#open-input", { name: file.name, mimeType: "application/octet-stream", buffer: file.buffer });
}

/** Wait until the epoch is `epoch` and the new schedule has been drawn a few times. */
async function drawn(g: Gantt, epoch: number): Promise<void> {
  await g.page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
}

function trackRequests(page: Page): string[] {
  const requests: string[] = [];
  page.on("request", (request) => requests.push(`${request.method()} ${request.url()}`));
  return requests;
}

/** Count the reads of a file's bytes in `window.__reads`; the wrappers call through. */
async function countFileReads(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { __reads: string[] };
    w.__reads = [];
    for (const name of ["arrayBuffer", "text", "stream", "slice"] as const) {
      const original = Blob.prototype[name] as (...args: unknown[]) => unknown;
      Object.defineProperty(Blob.prototype, name, {
        configurable: true,
        writable: true,
        value: function (this: Blob, ...args: unknown[]) {
          w.__reads.push(`Blob.${name} ${(this as File).name ?? ""}`);
          return original.apply(this, args);
        },
      });
    }
  });
}

const fileReads = (page: Page): Promise<string[]> =>
  page.evaluate(() => (window as unknown as { __reads: string[] }).__reads.slice());

const firstRowName = async (page: Page): Promise<string> =>
  (await page.locator("#table-rows .row .name").first().textContent()) ?? "";

const pickerOpen = (page: Page): Promise<boolean> => page.evaluate(() => document.querySelector<HTMLDialogElement>("#picker")!.open);

const activeId = (page: Page): Promise<string | undefined> => page.evaluate(() => document.activeElement?.id);

test("a file with several projects asks first: one row per project in file order, an all row, and nothing changes yet", { tag: "@smoke" }, async ({ g }) => {
  await g.open({ n: SIZE });
  const requests = trackRequests(g.page);
  await countFileReads(g.page);

  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("#picker")).toBeVisible();
  expect(await pickerOpen(g.page)).toBe(true);
  await expect(g.page.locator("#picker-title")).toHaveText("two-projects.xer holds 2 projects");
  const rows = g.page.locator("#picker-list button");
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(0)).toHaveText("1000 · Café Project 1 · 8 activities");
  await expect(rows.nth(1)).toHaveText("1001 · Project 2 · 8 activities");
  await expect(rows.nth(2)).toHaveText("Open all 2 projects (16 activities)");
  expect(await rows.evaluateAll((buttons) => buttons.map((b) => (b as HTMLElement).dataset.project))).toEqual(["1000", "1001", "all"]);
  await expect(g.page.locator("#picker-cancel")).toHaveText("Cancel");

  // The dialog is modal: the focus is inside it, and the page behind it has not changed.
  expect(await g.page.evaluate(() => document.querySelector("#picker")!.contains(document.activeElement))).toBe(true);
  const s = await g.status();
  expect(s.epoch).toBe(0);
  expect(s.n).toBe(SIZE);
  expect(await g.page.evaluate(() => window.gantt!.opened)).toBeUndefined();
  await expect(g.page.locator("#open-info")).not.toContainText("two-projects");
  await expect(g.page.locator("#open-info")).not.toContainText("Opening");
  await expect(g.page.locator("#open-error")).toBeHidden();

  // Only the File is kept while the dialog is open: the file was read once, for the first pass.
  expect(await fileReads(g.page)).toEqual(["Blob.arrayBuffer two-projects.xer"]);
  expect(requests, "the picker makes no network request").toEqual([]);
});

test("choosing a project opens that project, reads the file again and records probeMs", async ({ g }) => {
  await g.open({ n: SIZE });
  const requests = trackRequests(g.page);
  await countFileReads(g.page);
  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("#picker")).toBeVisible();

  await g.page.locator('#picker button[data-project="1001"]').click();
  await drawn(g, 1);
  expect(await pickerOpen(g.page)).toBe(false);
  const s = await g.status();
  expect(s.n).toBe(PROJECT_TASKS);
  expect(s.bars).toBeGreaterThan(0);
  expect(s.epoch).toBe(1);
  expect(await firstRowName(g.page)).toContain("Project 2");
  expect(await firstRowName(g.page)).not.toContain("Café");
  const opened = await g.page.evaluate(() => window.gantt!.opened!);
  expect(opened).toMatchObject({ name: "two-projects.xer", format: "xer", tasks: PROJECT_TASKS, bytes: TWO_PROJECTS.buffer.length });
  expect(Number.isFinite(opened.probeMs) && opened.probeMs! >= 0, "probeMs is the first pass's time").toBe(true);
  await expect(g.page.locator("#open-info")).toContainText("two-projects.xer");
  await expect(g.page.locator("#open-error")).toBeHidden();
  expect(await activeId(g.page), "focus returns to the chart").toBe("app");
  expect(await fileReads(g.page), "the file is read again after the choice").toEqual([
    "Blob.arrayBuffer two-projects.xer",
    "Blob.arrayBuffer two-projects.xer",
  ]);
  expect(requests).toEqual([]);

  // The other project, from a fresh picker (the same file chosen twice must ask twice).
  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("#picker")).toBeVisible();
  await g.page.locator('#picker button[data-project="1000"]').click();
  await drawn(g, 2);
  expect((await g.status()).n).toBe(PROJECT_TASKS);
  // The Windows-1252 byte in a project name is decoded.
  expect(await firstRowName(g.page)).toContain("Café Project 1");
});

test("a file with one project, or an MS Project file, opens without asking and has no probeMs", async ({ g }) => {
  await g.open({ n: SIZE });
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);
  expect(await pickerOpen(g.page)).toBe(false);
  expect(await g.page.evaluate(() => "probeMs" in window.gantt!.opened!)).toBe(false);
  await pickFile(g.page, MSPDI);
  await drawn(g, 2);
  expect(await pickerOpen(g.page)).toBe(false);
  expect(await g.page.evaluate(() => "probeMs" in window.gantt!.opened!)).toBe(false);
});

test("Esc and Cancel leave the schedule on the page exactly as it was", async ({ g }) => {
  // Each open is over the edit, so it asks (gs#703); the test answers yes and then cancels at the project list.
  g.page.on("dialog", (dialog) => void dialog.accept());
  await g.open({ n: SIZE });
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);
  // Leave a mark on the open schedule: a selection and an edit with its undo.
  await g.page.evaluate(() => {
    const { app } = window.gantt!;
    app.rename_task(3, "kept name");
  });
  await g.nextFrame(2);
  const before = await g.status();
  const beforeName = await firstRowName(g.page);
  const beforeOpened = await g.page.evaluate(() => window.gantt!.opened!);
  const beforeInfo = await g.page.locator("#open-info").textContent();
  const beforeLabel = await g.page.evaluate(() => window.gantt!.app.task_label(3));
  expect(beforeLabel).toBe("kept name");

  const unchanged = async (): Promise<void> => {
    const s = await g.status();
    expect(s.epoch).toBe(before.epoch);
    expect(s.n).toBe(before.n);
    expect(s.revision, "the edit and its undo history are still there").toBe(before.revision);
    expect(await g.page.evaluate(() => window.gantt!.app.task_label(3))).toBe("kept name");
    expect(await firstRowName(g.page)).toBe(beforeName);
    expect(await g.page.evaluate(() => window.gantt!.opened)).toEqual(beforeOpened);
    expect(await g.page.locator("#open-info").textContent()).toBe(beforeInfo);
    await expect(g.page.locator("#open-error")).toBeHidden();
    expect(await activeId(g.page), "focus returns to the chart").toBe("app");
    await g.nextFrame(2);
    expect((await g.status()).frames, "the page keeps drawing").toBeGreaterThan(s.frames);
  };

  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("#picker")).toBeVisible();
  await g.page.keyboard.press("Escape");
  await expect(g.page.locator("#picker")).toBeHidden();
  await unchanged();

  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("#picker")).toBeVisible();
  await g.page.locator("#picker-cancel").click();
  await expect(g.page.locator("#picker")).toBeHidden();
  await unchanged();

  // A cancelled file does not stay half-chosen: choosing again asks again, and then a choice works.
  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("#picker")).toBeVisible();
  await g.page.locator('#picker button[data-project="1000"]').click();
  await drawn(g, 2);
  expect((await g.status()).n).toBe(PROJECT_TASKS);
});

test("the dialog's keys stay in the dialog: the chart's keys do nothing while it is open", async ({ g }) => {
  await g.open({ n: SIZE });
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);
  // Control: with focus on the chart, these keys do change what it shows.
  const full = (await g.status()).rowCount;
  await g.page.keyboard.press("1");
  await g.nextFrame(2);
  expect((await g.status()).rowCount, "the collapse key works on the chart").toBeLessThan(full);
  await g.page.keyboard.press("0");
  await g.nextFrame(2);
  expect((await g.status()).rowCount).toBe(full);
  const before = await g.status();
  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("#picker")).toBeVisible();

  const chooser = g.page.waitForEvent("filechooser", { timeout: 1_000 }).then(
    () => true,
    () => false,
  );
  for (const key of ["1", "2", "0", "Control+o", "Control+z", "Control+f", "Shift+C", "Shift+L", "ArrowDown", "Tab"]) {
    await g.page.keyboard.press(key);
  }
  expect(await chooser, "Ctrl+O shows no file chooser behind the dialog").toBe(false);
  const s = await g.status();
  expect(s.rowCount, "no collapse key reached the chart").toBe(before.rowCount);
  expect(s.filter).toBe(before.filter);
  expect(s.revision).toBe(before.revision);
  expect(s.epoch).toBe(before.epoch);
  expect(await pickerOpen(g.page), "none of those keys closed or chose anything").toBe(true);
  expect(await g.page.evaluate(() => document.querySelector("#picker")!.contains(document.activeElement))).toBe(true);
  await expect(g.page.locator("#find-input")).not.toBeFocused();
});

test("a newer file supersedes a waiting project choice", async ({ g }) => {
  await g.open({ n: SIZE });
  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("#picker")).toBeVisible();
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);
  expect(await pickerOpen(g.page)).toBe(false);
  expect((await g.status()).n).toBe(14);
  expect(await g.page.evaluate(() => window.gantt!.opened!.name)).toBe("one-project.xer");
  expect(await g.page.evaluate(() => "probeMs" in window.gantt!.opened!)).toBe(false);
});
