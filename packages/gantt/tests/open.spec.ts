// Open a file from the page (gs#87 PR 2): the Open button, Ctrl+O, a drop, a second file that replaces the
// first, no network request, and the errors. The fixtures are files in tests/fixtures, written by xer::synth.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// WebGL2 from the start: this file checks the file chooser, the drop and the page's marks, not the GPU. A WebGPU device lost just
// after the page was ready reloaded it onto WebGL2 under the test (the chooser never came, a stale editor held the keys), and
// five runs of this file failed on main (gantt#1163). A page that never holds a device has nothing to wait out.
test.use({ webgl: true });

const SIZE = 300;
const fixture = (name: string): Buffer => readFileSync(new URL(`./fixtures/${name}`, import.meta.url));
const ONE_PROJECT = { name: "one-project.xer", buffer: fixture("one-project.xer"), tasks: 14 };
const TWO_PROJECTS = { name: "two-projects.xer", buffer: fixture("two-projects.xer"), tasks: 20 };
const MSPDI = {
  name: "summary.xml",
  buffer: readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url)),
  tasks: 5,
};
type Fixture = { name: string; buffer: Buffer; tasks: number };

/** Every request the page makes from now on. */
function trackRequests(page: Page): string[] {
  const requests: string[] = [];
  page.on("request", (request) => requests.push(`${request.method()} ${request.url()}`));
  return requests;
}

async function pickFile(page: Page, file: Fixture): Promise<void> {
  await page.setInputFiles("#open-input", { name: file.name, mimeType: "application/octet-stream", buffer: file.buffer });
}

/** Wait until the epoch is `epoch` and the new schedule has been drawn a few times. */
async function drawn(g: Gantt, epoch: number): Promise<void> {
  await g.page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch, { timeout: 60_000 });
  await g.nextFrame(3);
  // The table redraws on the frame after the epoch moves.
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
}

/** Drop `files` on `target` the way a browser does: dragenter, dragover, then drop. */
async function drop(page: Page, files: (Fixture & { size?: number })[], target = "#app"): Promise<void> {
  await page.evaluate(
    ([selector, list]) => {
      const transfer = new DataTransfer();
      for (const entry of list) {
        const file = new File([new Uint8Array(entry.bytes)], entry.name);
        // A file the test cannot really make is given a size by overriding the property.
        if (entry.size !== undefined) Object.defineProperty(file, "size", { value: entry.size });
        transfer.items.add(file);
      }
      const element = document.querySelector(selector)!;
      for (const type of ["dragenter", "dragover", "drop"]) {
        element.dispatchEvent(new DragEvent(type, { dataTransfer: transfer, bubbles: true, cancelable: true }));
      }
    },
    [target, files.map((file) => ({ name: file.name, bytes: Array.from(file.buffer), size: file.size }))] as const,
  );
}

/** The canvas is not one flat colour: a PNG of one colour is a few hundred bytes, a drawn chart is far more. */
async function canvasDrawn(page: Page): Promise<number> {
  return (await page.locator("#timeline").screenshot()).length;
}

/**
 * Count every way the page can read a file's bytes: the Blob read methods and FileReader. The counter is
 * `window.__reads`; the wrappers call through, so a read still works. Installed in the page, not in the app.
 */
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
    for (const name of ["readAsArrayBuffer", "readAsText", "readAsBinaryString", "readAsDataURL"] as const) {
      const original = FileReader.prototype[name] as (...args: unknown[]) => unknown;
      Object.defineProperty(FileReader.prototype, name, {
        configurable: true,
        writable: true,
        value: function (this: FileReader, ...args: unknown[]) {
          w.__reads.push(`FileReader.${name} ${((args[0] as File | undefined)?.name) ?? ""}`);
          return original.apply(this, args);
        },
      });
    }
  });
}

async function fileReads(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __reads: string[] }).__reads.slice());
}

async function firstRowName(page: Page): Promise<string> {
  return (await page.locator("#table-rows .row .name").first().textContent()) ?? "";
}

test("the Open input opens an XER in place: rows, bars, toolbar line, unsupported list, no network request", { tag: "@smoke" }, async ({ g }) => {
  await g.open({ n: SIZE });
  const before = await g.status();
  expect(before.n).toBe(SIZE);
  expect(before.epoch).toBe(0);
  expect(await g.page.evaluate(() => window.gantt!.opened)).toBeUndefined();
  const requests = trackRequests(g.page);

  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);

  const s = await g.status();
  expect(s.n).toBe(ONE_PROJECT.tasks);
  expect(s.rowCount).toBe(ONE_PROJECT.tasks);
  expect(s.bars).toBeGreaterThan(0);
  expect(s.revision).toBe(0);
  expect(s.epoch).toBe(1);
  expect(s.backend).toBe(before.backend);
  // The file has activity IDs, so the table gains its ID column inside its half of the box (gs#866) and the canvas keeps its width.
  expect(s.surface[1]).toBe(before.surface[1]);
  expect(s.surface[0]).toBe(before.surface[0]);
  // The phases of the open are checked in their own test below; here the file's facts.
  expect(await g.page.evaluate(() => ({ ...window.gantt!.opened!, timings: undefined }))).toEqual({
    name: "one-project.xer",
    bytes: ONE_PROJECT.buffer.length,
    format: "xer",
    tasks: ONE_PROJECT.tasks,
    timings: undefined,
  });
  expect(await canvasDrawn(g.page), "the canvas is drawn, not blank").toBeGreaterThan(4_000);
  expect(await firstRowName(g.page)).toContain("Café Project 1");

  const info = g.page.locator("#open-info");
  await expect(info).toContainText("one-project.xer");
  await expect(info).toHaveAttribute("title", /P6 XER/);
  await expect(info).toHaveAttribute("title", new RegExp(`${ONE_PROJECT.tasks} tasks`));
  // The toolbar shows one line for the file: no second line says where it was read (gs#646).
  await expect(info).not.toContainText("Parsed in this browser");
  await expect(g.page.locator("#open-note")).toHaveCount(0);
  // The line fits the toolbar's fixed height and nothing is pushed out of it.
  const fit = await g.page.evaluate(() => {
    const toolbar = document.getElementById("toolbar")!;
    const bar = toolbar.getBoundingClientRect();
    const infoBox = document.getElementById("open-info")!.getBoundingClientRect();
    const file = document.getElementById("open-file")!;
    const find = document.getElementById("find-input")!.getBoundingClientRect();
    return {
      overflow: toolbar.scrollWidth - toolbar.clientWidth,
      infoHeight: infoBox.height,
      lineHeight: parseFloat(getComputedStyle(file).lineHeight),
      infoInside: infoBox.bottom <= bar.bottom && infoBox.top >= bar.top,
      findInside: find.right <= bar.right,
      fileWidth: file.getBoundingClientRect().width,
      fileScroll: file.scrollWidth,
    };
  });
  expect(fit.overflow, "the toolbar does not overflow").toBeLessThanOrEqual(0);
  expect(fit.infoHeight, "the file line is one line high").toBeLessThanOrEqual(fit.lineHeight);
  expect(fit.infoInside && fit.findInside, "the file line and the find box are inside the toolbar").toBe(true);
  // The strip is full at 1280px once the data date is in it, so the file group may give up a fraction of a pixel.
  expect(fit.fileScroll, "the file line is shown whole").toBeLessThanOrEqual(fit.fileWidth + 1);
  await expect(g.page.locator("#open-error")).toBeHidden();

  // The importer's notes for this file are listed, replacing the demo's (which had none).
  await expect(g.page.locator("#unsupported")).toBeHidden();
  await g.page.locator("#import-notes").click();
  await expect(g.page.locator("#unsupported")).toBeVisible();
  // The notes themselves are read in packages/gantt/tests-node/parse.test.ts.

  expect(requests, "opening a file makes no network request").toEqual([]);
});

test("the Open button and Ctrl+O show the file chooser, and Ctrl+O acts only while focus is inside the chart", async ({ g }) => {
  await g.open({ n: SIZE });
  const requests = trackRequests(g.page);

  const byButton = g.page.waitForEvent("filechooser");
  await g.page.locator("#open-button").click();
  const chooser = await byButton;
  expect(chooser.isMultiple()).toBe(false);
  await chooser.setFiles({ name: MSPDI.name, mimeType: "application/xml", buffer: MSPDI.buffer });
  await drawn(g, 1);
  expect((await g.status()).n).toBe(MSPDI.tasks);
  expect(await g.page.evaluate(() => window.gantt!.opened?.format)).toBe("mspdi");
  await expect(g.page.locator("#open-info")).toHaveAttribute("title", /MS Project XML/);

  // Focus is back on the chart after an open; Ctrl+O shows the chooser.
  const byKey = g.page.waitForEvent("filechooser");
  await g.page.keyboard.press("Control+o");
  const second = await byKey;
  await second.setFiles({ name: ONE_PROJECT.name, mimeType: "application/octet-stream", buffer: ONE_PROJECT.buffer });
  await drawn(g, 2);
  expect((await g.status()).n).toBe(ONE_PROJECT.tasks);
  // The data date each file states is checked in crates/app/tests/reopen.rs.

  // Outside the chart the key does nothing, from a real key press or an event sent to the document.
  await g.page.evaluate(() => (document.activeElement as HTMLElement).blur());
  const stray = g.page.waitForEvent("filechooser", { timeout: 1_000 }).then(
    () => true,
    () => false,
  );
  await g.page.keyboard.press("Control+o");
  await g.page.evaluate(() => {
    for (const target of [document.body, document.documentElement, document, window] as EventTarget[]) {
      target.dispatchEvent(new KeyboardEvent("keydown", { key: "o", ctrlKey: true, bubbles: true, cancelable: true }));
    }
  });
  expect(await stray, "no file chooser opened outside the chart").toBe(false);
  expect(requests).toEqual([]);
});

test("a dropped file opens, the dashed outline shows while it is over the chart, and extra files are ignored", async ({ g }) => {
  await g.open({ n: SIZE });
  const requests = trackRequests(g.page);
  const root = g.page.locator("#app");

  // Over the chart: the outline. Away again: gone.
  await g.page.evaluate(() => {
    const transfer = new DataTransfer();
    transfer.items.add(new File(["x"], "x.xer"));
    document.getElementById("app")!.dispatchEvent(new DragEvent("dragover", { dataTransfer: transfer, bubbles: true, cancelable: true }));
  });
  await expect(root).toHaveAttribute("data-dropping", "true");
  expect(await root.evaluate((el) => getComputedStyle(el).outlineStyle)).toBe("dashed");
  expect(await root.evaluate((el) => getComputedStyle(el, "::after").content), "the drag says what a release does").toBe('"Drop to open"');
  await g.page.evaluate(() => {
    document.getElementById("app")!.dispatchEvent(new DragEvent("dragleave", { bubbles: true, cancelable: true }));
  });
  await expect(root).not.toHaveAttribute("data-dropping", /.*/);

  // A drag that carries no file (selected text, say) is not taken.
  const refused = await g.page.evaluate(() => {
    const transfer = new DataTransfer();
    transfer.setData("text/plain", "hello");
    const event = new DragEvent("dragover", { dataTransfer: transfer, bubbles: true, cancelable: true });
    document.getElementById("app")!.dispatchEvent(event);
    return event.defaultPrevented;
  });
  expect(refused).toBe(false);
  await expect(root).not.toHaveAttribute("data-dropping", /.*/);

  await drop(g.page, [ONE_PROJECT, MSPDI, TWO_PROJECTS]);
  await drawn(g, 1);
  const s = await g.status();
  expect(s.n).toBe(ONE_PROJECT.tasks);
  expect(s.bars).toBeGreaterThan(0);
  expect(await g.page.evaluate(() => window.gantt!.opened?.name)).toBe("one-project.xer");
  await expect(root).not.toHaveAttribute("data-dropping", /.*/);
  await expect(g.page.locator("#open-info")).toContainText("2 other files ignored");
  expect(await canvasDrawn(g.page)).toBeGreaterThan(4_000);

  // A drop on the unsupported panel, which floats over the chart once the notice is opened, opens too: the browser would otherwise navigate to the file.
  await g.page.locator("#import-notes").click();
  await expect(g.page.locator("#unsupported")).toBeVisible();
  await drop(g.page, [MSPDI], "#unsupported");
  await drawn(g, 2);
  expect((await g.status()).n).toBe(MSPDI.tasks);
  expect(requests).toEqual([]);
});

test("a file dropped beside the chart opens nothing and the page stays, and a drag with no file is left alone", async ({ g }) => {
  await g.open({ n: SIZE });
  const url = g.page.url();
  // A real drag, below a chart cut short: a constructed DataTransfer ignores dropEffect, so it cannot tell a refusal from a pass.
  await g.page.evaluate(() => {
    document.getElementById("app")!.style.height = "400px";
    const log: string[] = ((window as unknown as { log: string[] }).log = []);
    window.addEventListener("dragover", (e) => log.push(`${e.dataTransfer!.types.includes("Files") ? "file" : "text"} ${e.defaultPrevented ? "refused" : "left"}`));
  });
  const cdp = await g.page.context().newCDPSession(g.page);
  const file = fileURLToPath(new URL("./fixtures/one-project.xer", import.meta.url));
  for (const data of [{ items: [], files: [file] }, { items: [{ mimeType: "text/plain", data: "hello" }] }]) {
    for (const type of ["dragEnter", "dragOver", "drop"]) await cdp.send("Input.dispatchDragEvent", { type, x: 600, y: 600, data: { ...data, dragOperationsMask: 1 } } as never);
  }
  await g.nextFrame(3);
  const log = await g.page.evaluate(() => (window as unknown as { log: string[] }).log);
  expect(new Set(log), "a dragged file is refused and a dragged text is left alone").toEqual(new Set(["file refused", "text left"]));
  expect(g.page.url(), "the page stays").toBe(url);
  expect((await g.status()).epoch, "nothing was opened").toBe(0);
  await expect(g.page.locator("#app")).not.toHaveAttribute("data-dropping", /.*/);
});

test("the note on dropped extra files goes with the file it was for, and survives the project list", async ({ g }) => {
  await g.open({ n: SIZE });
  const info = g.page.locator("#open-info");
  await drop(g.page, [ONE_PROJECT, MSPDI]);
  await drawn(g, 1);
  await expect(info).toContainText("1 other file ignored");
  // The same file opened again by the button: nothing was ignored this time.
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 2);
  await expect(info).not.toContainText("ignored");
  // A file with several projects dropped with another file: the count is still there after a project is chosen.
  await drop(g.page, [TWO_PROJECTS, MSPDI]);
  await expect(g.page.locator("dialog#picker")).toHaveAttribute("open", "");
  await g.page.locator("#picker-list button[data-project='all']").click();
  await drawn(g, 3);
  await expect(info).toContainText("1 other file ignored");
});

// The session's own reset (revision, selection, filter, scroll, both histories) is checked in crates/app/tests/reopen.rs;
// this test checks what the page keeps beside the session: the find box and the table's scroll.
test("a second file replaces the first: a clean page, no leftovers", async ({ g }) => {
  await g.page.setViewportSize({ width: 1280, height: 190 }); // a short window, so rows overflow
  await g.open({ n: SIZE });
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);

  // Leave marks on the first schedule: a search and a scrolled view. An open cell editor refuses the open (open-guard.spec.ts).
  await g.page.locator("#find-input").fill("A1-1");
  await expect(g.page.locator("#find-count")).toContainText("of");
  await g.nextFrame(2);
  // The wheel clamps to the canvas's height, so it moves nothing until the canvas has taken the short window: wheel until it has.
  await g.page.waitForFunction(() => {
    const { app } = window.gantt!;
    const canvas = document.getElementById("timeline")!.getBoundingClientRect();
    app.wheel(canvas.width / 2, canvas.height / 2, 0, 1_000, false);
    return window.gantt!.status().scrollY > 0;
  });
  await g.nextFrame(3);
  expect(await g.page.evaluate(() => document.getElementById("table-body")!.scrollTop), "the task list is scrolled before the replacement").toBeGreaterThan(0);

  // The second file has more rows than the window shows, so a scroll position that was kept would still be valid.
  await pickFile(g.page, TWO_PROJECTS);
  await g.page.locator('#picker button[data-project="all"]').click();
  await drawn(g, 2);
  const s = await g.status();
  expect(s.epoch).toBe(2);
  expect(s.n).toBe(TWO_PROJECTS.tasks);
  await expect(g.page.locator("#find-input")).toHaveValue("");
  await expect(g.page.locator("#find-count")).toHaveText("");
  expect(await firstRowName(g.page)).toBe(await g.page.evaluate(() => window.gantt!.app.task_label(0)));
  expect(await g.page.evaluate(() => window.gantt!.opened?.name)).toBe("two-projects.xer");
  await expect(g.page.locator("#open-info")).toContainText("two-projects.xer");
  expect(await g.page.evaluate(() => document.getElementById("table-body")!.scrollTop), "the task list is back at the top").toBe(0);
  // The shell is not left believing an editor is open: a key that an open editor would block still works.
  const chooser = g.page.waitForEvent("filechooser", { timeout: 5_000 }).then(
    () => true,
    () => false,
  );
  await g.page.keyboard.press("Control+o");
  expect(await chooser, "Ctrl+O works: no stale cell editor holds the chart's keys").toBe(true);

  // The same file chosen again opens again.
  await pickFile(g.page, MSPDI);
  await drawn(g, 3);
  expect((await g.status()).epoch).toBe(3);
});

test("a file that cannot be opened is a message on the error line and the previous schedule stays", async ({ g }) => {
  await g.open({ n: SIZE });
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);
  const kept = await g.status();
  const keptName = await firstRowName(g.page);
  const error = g.page.locator("#open-error");
  const requests = trackRequests(g.page);
  await countFileReads(g.page);

  const intact = async (): Promise<void> => {
    const s = await g.status();
    expect(s.epoch, "the failed open changed nothing").toBe(kept.epoch);
    expect(s.n).toBe(kept.n);
    expect(await g.page.evaluate(() => window.gantt!.opened?.name)).toBe("one-project.xer");
    expect(await firstRowName(g.page)).toBe(keptName);
    await g.nextFrame(2);
    expect((await g.status()).frames, "the page keeps drawing").toBeGreaterThan(s.frames);
  };
  const fails = async (file: Fixture & { size?: number }, message: RegExp): Promise<void> => {
    await drop(g.page, [file]);
    await expect(error).toBeVisible();
    await expect(g.page.locator("#open-error-text")).toHaveText(message);
    await expect(g.page.locator("#open-info")).not.toContainText("Opening");
    await intact();
  };

  // A truncated XER: the header and a cut-off table. A file that is neither format is refused in packages/gantt/tests-node/parse.test.ts.
  await fails({ name: "cut.xer", buffer: ONE_PROJECT.buffer.subarray(0, 700), tasks: 0 }, /could not open cut\.xer: ./);
  await fails({ name: "empty.xer", buffer: Buffer.alloc(0), tasks: 0 }, /empty\.xer is empty/);
  // The wrappers see a real read: the files above were all read.
  const readsSoFar = await fileReads(g.page);
  expect(readsSoFar.filter((read) => read.includes("cut.xer")), "the counter sees the page read a file").not.toEqual([]);
  // Over the cap: the size is read from the File and its bytes are never read.
  await fails(
    { name: "huge.xer", buffer: Buffer.from("ERMHDR"), tasks: 0, size: 300 * 1024 * 1024 },
    /huge\.xer is 300\.0 MiB; files over 256 MiB are not opened/,
  );
  expect(
    (await fileReads(g.page)).slice(readsSoFar.length),
    "no read method was called for the oversized file, nor for anything else after it",
  ).toEqual([]);

  // The next open clears the line; the close button clears it too.
  await pickFile(g.page, MSPDI);
  await drawn(g, 2);
  await expect(error).toBeHidden();
  await drop(g.page, [{ name: "again.xml", buffer: Buffer.from("nope"), tasks: 0 }]);
  await expect(error).toBeVisible();
  await g.page.locator("#open-error-close").click();
  await expect(error).toBeHidden();
  expect(await g.page.evaluate(() => document.activeElement?.id), "focus returns to the chart").toBe("app");
  expect(requests, "a failed open makes no network request either").toEqual([]);
});

test("the chart works with the toolbar removed: a drop still opens, Ctrl+O does nothing", async ({ g }) => {
  await g.page.addInitScript(() => {
    const strip = (): void => {
      document.getElementById("toolbar")?.remove();
    };
    new MutationObserver(strip).observe(document, { childList: true, subtree: true });
  });
  await g.open({ n: SIZE });
  expect(await g.page.locator("#open-input").count()).toBe(0);

  const stray = g.page.waitForEvent("filechooser", { timeout: 1_000 }).then(
    () => true,
    () => false,
  );
  await g.page.keyboard.press("Control+o");
  expect(await stray).toBe(false);

  await drop(g.page, [ONE_PROJECT]);
  await drawn(g, 1);
  expect((await g.status()).n).toBe(ONE_PROJECT.tasks);
});

const PHASES = ["read", "paint", "copy", "parse", "build", "compute", "swap", "firstFrame", "total"] as const;
/** Rust's first to last clock read of `load_bytes`: a check on the three Rust phases, not a phase and not in the total. */
const FIELDS = [...PHASES, "rustSpan"] as const;

/** The timings of the open that is current, once its first frame has been submitted. */
async function timingsOf(g: Gantt): Promise<Record<(typeof FIELDS)[number], number>> {
  await g.page.waitForFunction(() => window.gantt!.opened?.timings !== undefined, undefined, { timeout: 60_000 });
  return g.page.evaluate(() => window.gantt!.opened!.timings as unknown as Record<(typeof FIELDS)[number], number>);
}

test("an open records its phases: every one counted once, they add up to the total, the status line shows them", async ({ g }) => {
  await g.open({ n: SIZE });
  expect(await g.page.evaluate(() => window.gantt!.opened)).toBeUndefined();
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);
  const t = await timingsOf(g);

  expect(Object.keys(t).sort()).toEqual([...FIELDS].sort());
  for (const phase of FIELDS) {
    expect(Number.isFinite(t[phase]) && t[phase] >= 0, `${phase} is finite and not negative (${t[phase]})`).toBe(true);
  }
  // The three Rust phases are part of the call, which is part of the open.
  expect(t.total).toBeGreaterThanOrEqual(t.parse + t.build + t.compute);
  // That the three Rust phases add up to `rustSpan` (gs#248) is checked in Rust, `load_bytes_with_span`'s own tests.
  expect(t.copy, "copy is not clamped, and a real one is not negative").toBeGreaterThanOrEqual(0);
  // The windows are back to back: nothing is left out and nothing is counted twice. (`copy` is the call minus its
  // Rust phases, not clamped; the clock is coarse, so allow a tenth of a millisecond.)
  const sum = PHASES.filter((phase) => phase !== "total").reduce((total, phase) => total + t[phase], 0);
  expect(Math.abs(t.total - sum), `total ${t.total} vs the phases' sum ${sum}`).toBeLessThan(0.1);
  // A parse of a 14-activity file is far below the paint wait; the wait is a frame and a task, so it is not zero.
  expect(t.paint).toBeGreaterThan(0);
  expect(t.firstFrame).toBeGreaterThan(0);

  // The status line (the `s` key) carries one `open` segment with the split.
  await g.page.keyboard.press("s");
  const line = g.page.locator("#app #status-line");
  await expect(line).toBeVisible();
  await expect(line).toContainText("open one-project.xer");
  for (const label of ["read", "paint", "copy", "parse", "build", "compute", "swap", "first frame"]) {
    await expect(line).toContainText(`${label} `);
  }
  const text = (await line.textContent()) ?? "";
  expect(text.match(/open one-project\.xer/g)?.length, "one open segment").toBe(1);
  await g.page.keyboard.press("s");
});

test("a failed open leaves the previous open's phases and status line as they were; the next open replaces them", async ({ g }) => {
  await g.open({ n: SIZE });
  await pickFile(g.page, ONE_PROJECT);
  await drawn(g, 1);
  const first = await timingsOf(g);
  await g.page.keyboard.press("s");
  const line = g.page.locator("#app #status-line");
  await expect(line).toContainText("open one-project.xer");
  // The line holds live counters too; the `open` segment is the part that must not change.
  const segment = async (): Promise<string> => ((await line.textContent()) ?? "").match(/open one-project\.xer[^|]*/)?.[0] ?? "";
  const shown = await segment();
  expect(shown).toContain("first frame");

  await drop(g.page, [{ name: "cut.xer", buffer: ONE_PROJECT.buffer.subarray(0, 700), tasks: 0 }]);
  await expect(g.page.locator("#open-error")).toBeVisible();
  await g.nextFrame(3);
  expect(await g.page.evaluate(() => window.gantt!.opened!.timings)).toEqual(first);
  expect(await segment(), "the line still shows the same open").toBe(shown);
  expect((await g.status()).epoch).toBe(1);

  // A newer open drops the old line at once and shows its own when its first frame is in.
  await pickFile(g.page, MSPDI);
  await drawn(g, 2);
  const second = await timingsOf(g);
  expect(second).not.toEqual(first);
  await expect(line).toContainText("open summary.xml");
  await expect(line).not.toContainText("open one-project.xer");
});

test("the second pass of a multi-project file is the open that counts: probeMs is apart from its phases", async ({ g }) => {
  await g.open({ n: SIZE });
  await pickFile(g.page, TWO_PROJECTS);
  await expect(g.page.locator("dialog#picker")).toHaveAttribute("open", "");
  // The first pass stopped at the project list: no open yet, so no phases.
  expect(await g.page.evaluate(() => window.gantt!.opened)).toBeUndefined();
  await g.page.locator("#picker-list button[data-project='all']").click();
  await drawn(g, 1);
  const t = await timingsOf(g);
  const opened = await g.page.evaluate(() => window.gantt!.opened!);
  expect(typeof opened.probeMs).toBe("number");
  for (const phase of FIELDS) expect(Number.isFinite(t[phase]) && t[phase] >= 0, phase).toBe(true);
  const sum = PHASES.filter((phase) => phase !== "total").reduce((total, phase) => total + t[phase], 0);
  expect(Math.abs(t.total - sum)).toBeLessThan(0.1);
});

/** An MSPDI file of `count` independent tasks that each carry a finish no later than constraint with no date, so each adds one unsupported line. */
function constraintProject(count: number): Fixture {
  let xml =
    `<?xml version="1.0" encoding="UTF-8"?>\n<Project xmlns="http://schemas.microsoft.com/project">` +
    `<StartDate>2026-01-05T08:00:00</StartDate><Tasks>`;
  for (let uid = 1; uid <= count; uid++) {
    xml +=
      `<Task><UID>${uid}</UID><Name>T${uid}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>` +
      `<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat><ConstraintType>7</ConstraintType></Task>`;
  }
  xml += `</Tasks></Project>`;
  return { name: `constraints-${count}.xml`, buffer: Buffer.from(xml), tasks: count };
}

/** The lines of the unsupported panel as the page shows them. */
async function panelLines(page: Page): Promise<string[]> {
  return page.evaluate(() => document.querySelector("#unsupported-list")!.textContent!.split("\n"));
}

test("the unsupported panel lists every line of a file with a few dozen, past the old limit of 20", async ({ g }) => {
  await g.open({ n: SIZE });
  // 25 such tasks make 25 lines, and the importer adds one for the missing calendars: 26 in all.
  await pickFile(g.page, constraintProject(25));
  await drawn(g, 1);
  const all = await g.page.evaluate(() => window.gantt!.unsupported!);
  expect(await panelLines(g.page), "the panel lists them all").toEqual(all);
  await expect(g.page.locator("#unsupported-title")).toHaveText("26 import notes: what was not read from this file");
});

test("the unsupported panel stops at 500 lines with one last line that counts the rest, and scrolls to it", async ({ g }) => {
  await g.open({ n: SIZE });
  await pickFile(g.page, constraintProject(520));
  await drawn(g, 1);
  await g.page.locator("#import-notes").click();
  const all = await g.page.evaluate(() => window.gantt!.unsupported!);
  const lines = await panelLines(g.page);
  expect(lines, "500 lines, then the count of the rest").toHaveLength(501);
  expect(lines.slice(0, 500)).toEqual(all.slice(0, 500));
  expect(lines[500]).toBe("and 21 more");
  await expect(g.page.locator("#unsupported-title")).toHaveText("521 import notes: what was not read from this file, first 500 shown");

  // The list scrolls inside the panel, and the title row with the close button stays in view.
  const scroll = await g.page.evaluate(() => {
    const panel = document.querySelector<HTMLElement>("#unsupported")!;
    const list = document.querySelector<HTMLElement>("#unsupported-list")!;
    const head = document.querySelector<HTMLElement>("#unsupported-head")!;
    const scrollable = list.scrollHeight > list.clientHeight;
    list.scrollTop = list.scrollHeight;
    const after = list.getBoundingClientRect();
    const panelBox = panel.getBoundingClientRect();
    return {
      scrollable,
      reachedEnd: list.scrollTop + list.clientHeight >= list.scrollHeight - 1,
      listInside: after.bottom <= panelBox.bottom + 1,
      headInside: head.getBoundingClientRect().top >= panelBox.top,
      panelHeight: panelBox.height,
      viewHeight: window.innerHeight,
    };
  });
  expect(scroll.scrollable, "the list is taller than its box").toBe(true);
  expect(scroll.reachedEnd, "the last line can be scrolled to").toBe(true);
  expect(scroll.listInside && scroll.headInside, "the list and the title row stay inside the panel").toBe(true);
  expect(scroll.panelHeight, "the panel stays at most 40% of the chart height").toBeLessThan(scroll.viewHeight * 0.5);
  await expect(g.page.locator("#unsupported-close")).toBeVisible();

  // The next open shows its own list from the top.
  await pickFile(g.page, constraintProject(3));
  await drawn(g, 2);
  expect(await panelLines(g.page)).toHaveLength(4);
  expect(await g.page.evaluate(() => document.querySelector("#unsupported-list")!.scrollTop)).toBe(0);
});
