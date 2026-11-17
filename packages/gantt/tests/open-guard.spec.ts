// Opening a file is one guarded step (gs#1152): `Gantt.open` and `Gantt.load` share one token, and a swap is refused while a drag or a cell edit is on.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

const bytes = (name: string): number[] => [...readFileSync(new URL(`./fixtures/${name}`, import.meta.url))];
const A = ["links.xml", bytes("links.xml")] as const;
const B = ["one-project.xer", bytes("one-project.xer")] as const;
const BUSY = "Finish the cell edit or drag first";
type Held = { release: () => void; done: Promise<unknown> };
declare global {
  interface Window {
    held?: Held;
  }
}

/** Start `open` or `load` on a file; with `hold` its bytes arrive only at `settle`. Resolves with the call's result when it is not held. */
const start = (page: Page, via: "open" | "load", [name, data]: readonly [string, number[]], hold = true): Promise<unknown> =>
  page.evaluate(async ([how, n, d, held]) => {
    const file = new File([new Uint8Array(d)], n);
    let release = (): void => undefined;
    const gate = new Promise<void>((go) => (release = go));
    if (held) file.arrayBuffer = async () => (await gate, new Uint8Array(d).buffer);
    const api = window.gantt!.api!;
    const done = how === "open" ? api.open(file) : api.load({ file });
    if (!held) return done;
    window.held = { release, done };
  }, [via, name, data, hold] as const);

/** Release the held read and wait for its call to end; the result of a `load`. */
const settle = (page: Page): Promise<unknown> => page.evaluate(async () => (window.held!.release(), await window.held!.done));
const opened = (page: Page): Promise<string | undefined> => page.evaluate(() => window.gantt!.opened?.name);

test("a load still reading when a newer open lands stops, and the open's file stays", async ({ g }) => {
  await g.open({ n: 20 });
  await start(g.page, "load", A);
  await start(g.page, "open", B, false);
  expect(await opened(g.page)).toBe("one-project.xer");
  expect(await settle(g.page), "the older load says it stopped").toMatchObject({ ok: false });
  expect(await opened(g.page), "the newer file stays").toBe("one-project.xer");
});

test("an open still reading when a newer load lands stops, and the load's file stays", async ({ g }) => {
  await g.open({ n: 20 });
  await start(g.page, "open", A);
  await start(g.page, "load", B, false);
  await settle(g.page);
  expect(await opened(g.page), "the newer load stays").toBe("one-project.xer");
  expect(await g.page.evaluate(() => window.gantt!.status().epoch)).toBe(1);
});

test("a cell edit refuses an open before its read starts", async ({ g }) => {
  await g.open({ n: 20 });
  await g.page.locator("#table-rows .row .name").first().dblclick();
  await g.page.keyboard.type("half-typed");
  await start(g.page, "open", A);
  await expect(g.page.locator("#open-error-text")).toContainText(BUSY);
  await settle(g.page);
  expect(await opened(g.page), "nothing was opened").toBeUndefined();
  await expect(g.page.locator("#table-rows input.cell-edit"), "the editor and its text stay").toHaveValue("half-typed");
});

test("a cell edit begun during the read refuses the swap", async ({ g }) => {
  await g.open({ n: 20 });
  await start(g.page, "open", A);
  await g.page.locator("#table-rows .row .name").first().dblclick();
  await g.page.keyboard.type("typed during the read");
  await settle(g.page);
  await expect(g.page.locator("#open-error-text")).toContainText(BUSY);
  expect(await opened(g.page), "the read ended under the edit: refused").toBeUndefined();
  await expect(g.page.locator("#table-rows input.cell-edit")).toHaveValue("typed during the read");
});

test("a bar drag begun during the read refuses the swap", async ({ g }) => {
  await g.open({ n: 20 });
  const box = await g.box();
  await start(g.page, "open", A);
  let pressed = false;
  for (let row = 1; row < 8 && !pressed; row++) {
    const y = box.y + AXIS + row * ROW + ROW / 2;
    const x = await g.page.evaluate((r) => {
      const start = window.gantt!.app.row_start(r);
      return start === undefined ? null : window.gantt!.app.x_of(start);
    }, row);
    if (x === null || x < 0 || x > box.width - 60) continue;
    await g.page.mouse.move(box.x + x + 8, y);
    await g.nextFrame(2);
    if (!(await g.page.evaluate(() => (window.gantt!.status().hover ?? { summary: true }).summary !== true))) continue;
    await g.page.mouse.down();
    for (let k = 1; k <= 6; k++) await g.page.mouse.move(box.x + x + 8 + 12 * k, y);
    await g.nextFrame(3);
    pressed = await g.page.evaluate(() => window.gantt!.status().drag?.active === true);
    if (!pressed) await g.page.mouse.up();
  }
  expect(pressed, "a bar drag is on").toBe(true);
  await settle(g.page);
  await expect(g.page.locator("#open-error-text")).toContainText(BUSY);
  expect(await opened(g.page), "the swap under a drag was refused").toBeUndefined();
  await g.page.mouse.up();
});

test("an edit made during a load's read is not replaced", async ({ g }) => {
  await g.open({ n: 20 });
  await start(g.page, "load", A);
  await g.page.evaluate(() => window.gantt!.api!.commands.insertRow());
  await g.nextFrame(3);
  expect(await settle(g.page)).toMatchObject({ ok: false, error: expect.stringContaining("edited while the file was read") });
  expect(await opened(g.page), "the edit stays").toBeUndefined();
});

test("an edit made during an open's read is not replaced, and a load under a cell edit is refused at once", async ({ g }) => {
  await g.open({ n: 20 });
  await start(g.page, "open", A);
  await g.page.evaluate(() => window.gantt!.api!.commands.insertRow());
  await g.nextFrame(3);
  await settle(g.page);
  await expect(g.page.locator("#open-error-text")).toContainText("edited while the file was read");
  expect(await opened(g.page), "the edit stays").toBeUndefined();
  await g.page.locator("#table-rows .row .name").first().dblclick();
  expect(await start(g.page, "load", B, false)).toMatchObject({ ok: false, error: expect.stringContaining(BUSY) });
});
