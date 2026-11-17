import type { Page } from "@playwright/test";
import { expect, pageErrors, test, type Gantt } from "./support/gantt";

// Embedding (gs#100 PR 4, decision 0005): the chart takes the size of the box its host gives it, writes no style
// outside its root, and handles keys only while focus is inside it. The demo page is the host here: the tests move
// the root into a box of their own, as another page would place it.

const SIZE = 2000;

interface Box {
  width: number;
  height: number;
}

/** Move the chart root into a new `host` box of the given size, `left`/`top` from the window's corner. */
async function embed(g: Gantt, size: Box, at = { left: 0, top: 0 }): Promise<void> {
  await g.page.evaluate(
    ([w, h, left, top]) => {
      const host = document.createElement("div");
      host.id = "host";
      host.style.cssText = `position: absolute; left: ${left}px; top: ${top}px; width: ${w}px; height: ${h}px`;
      const root = document.getElementById("app")!;
      document.body.append(host);
      host.append(root);
      root.focus({ preventScroll: true });
    },
    [size.width, size.height, at.left, at.top] as const,
  );
}

async function resizeHost(page: Page, size: Box): Promise<void> {
  await page.evaluate(([w, h]) => {
    const host = document.getElementById("host")!;
    host.style.width = `${w}px`;
    host.style.height = `${h}px`;
  }, [size.width, size.height] as const);
}

/** The canvas's CSS box, and the size the renderer was last given (physical pixels, divided back to CSS pixels). */
async function canvasSizes(g: Gantt): Promise<{ css: Box; surface: Box }> {
  const css = await g.page.evaluate(() => {
    const r = document.getElementById("timeline")!.getBoundingClientRect();
    return { width: r.width, height: r.height };
  });
  const dpr = await g.page.evaluate(() => devicePixelRatio);
  const [w, h] = (await g.status()).surface;
  return { css, surface: { width: w / dpr, height: h / dpr } };
}

const near = (a: number, b: number): boolean => Math.abs(a - b) <= 1;

async function followsHost(g: Gantt, size: Box): Promise<void> {
  const root = await g.page.evaluate(() => {
    const r = document.getElementById("app")!.getBoundingClientRect();
    return { width: r.width, height: r.height };
  });
  expect(root, "the root is exactly the host's box").toEqual(size);
  await expect
    .poll(async () => {
      const { css, surface } = await canvasSizes(g);
      return near(css.width, surface.width) && near(css.height, surface.height);
    }, "the renderer follows the canvas")
    .toBe(true);
  const { css } = await canvasSizes(g);
  expect(css.width, "the canvas fits in the host").toBeLessThanOrEqual(size.width);
  expect(css.height, "the canvas fits in the host").toBeLessThanOrEqual(size.height);
  expect(css.height, "the canvas is the host's height less the toolbar").toBeGreaterThan(size.height / 2);
}

test("the chart follows the box its host gives it, and not the window", async ({ g }) => {
  await g.open({ n: SIZE });
  const first = { width: 1000, height: 400 };
  await embed(g, first);
  await followsHost(g, first);

  const second = { width: 1100, height: 500 };
  await resizeHost(g.page, second);
  await followsHost(g, second);
  const grown = await canvasSizes(g);

  // The window alone moves nothing.
  await g.page.setViewportSize({ width: 1500, height: 900 });
  await g.nextFrame(5);
  await followsHost(g, second);
  expect(await canvasSizes(g), "the window did not change the chart").toEqual(grown);
  await g.page.setViewportSize({ width: 1280, height: 800 });
  await g.nextFrame(3);
  await followsHost(g, second);

  // A smaller box shrinks it again.
  await resizeHost(g.page, first);
  await followsHost(g, first);
});

test("a host box narrower than the task table raises no page error", async ({ g }) => {
  await g.open({ n: SIZE });
  await embed(g, { width: 400, height: 300 });
  const before = (await g.status()).frames;
  await g.nextFrame(10);
  expect((await g.status()).frames, "the page keeps drawing").toBeGreaterThan(before);
  await resizeHost(g.page, { width: 300, height: 120 });
  await g.nextFrame(5);
  await resizeHost(g.page, { width: 1000, height: 400 });
  await followsHost(g, { width: 1000, height: 400 });
  expect(pageErrors(g.logs), "no page error").toEqual([]);
});

test("the hover card stays inside the chart root, at its right and bottom edges", async ({ g }) => {
  await g.open({ n: SIZE });
  const size = { width: 800, height: 400 };
  // The host sits inside the window with room to spare on its right and below, so a card clamped to the window would hang outside the root.
  await embed(g, size, { left: 20, top: 60 });
  await followsHost(g, size);

  const canvas = await g.box();
  const s = await g.status();
  const lastRow = Math.floor((canvas.height - s.axisHeight - 1) / s.rowHeight);
  // Find a bar near the bottom-right corner of the canvas: scan the lowest rows from the right edge leftwards.
  let found: { x: number; y: number } | null = null;
  for (let row = lastRow; row >= lastRow - 3 && found === null; row -= 1) {
    const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;
    for (let x = canvas.x + canvas.width - 3; x > canvas.x + 4 && found === null; x -= 5) {
      await g.page.mouse.move(x, y);
      await g.nextFrame(1);
      if ((await g.status()).hover !== null) found = { x, y };
    }
  }
  expect(found, "a bar under the pointer near the bottom-right corner").not.toBeNull();
  const card = g.page.locator("#hover-card");
  await expect(card).toBeVisible();
  await g.nextFrame(3);

  const rects = await g.page.evaluate(() => {
    const r = document.getElementById("app")!.getBoundingClientRect();
    const c = document.getElementById("hover-card")!.getBoundingClientRect();
    return { root: { left: r.left, top: r.top, right: r.right, bottom: r.bottom }, card: { left: c.left, top: c.top, right: c.right, bottom: c.bottom } };
  });
  expect(rects.card.left, "card inside the root: left").toBeGreaterThanOrEqual(rects.root.left);
  expect(rects.card.top, "card inside the root: top").toBeGreaterThanOrEqual(rects.root.top);
  expect(rects.card.right, "card inside the root: right").toBeLessThanOrEqual(rects.root.right + 0.5);
  expect(rects.card.bottom, "card inside the root: bottom").toBeLessThanOrEqual(rects.root.bottom + 0.5);
  expect(await g.page.evaluate(() => document.getElementById("hover-card")!.parentElement!.id), "the card is a child of the root").toBe("app");
});

test("keys typed in a host input beside the chart do nothing to it", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.evaluate(() => {
    const input = document.createElement("input");
    input.id = "host-input";
    input.style.cssText = "position: absolute; left: 0; bottom: 0; z-index: 50";
    document.body.append(input);
    (window as unknown as { __keys: boolean[] }).__keys = [];
    // Bubbling on the window runs after the chart's own listener, so it sees whether the chart called preventDefault.
    window.addEventListener("keydown", (e) => (window as unknown as { __keys: boolean[] }).__keys.push(e.defaultPrevented));
  });
  let chooser = false;
  g.page.on("filechooser", () => (chooser = true));

  const before = await g.status();
  await g.page.locator("#host-input").focus();
  for (const key of ["1", "s", "c", "Shift+C", "Shift+L", "Control+f", "Control+o", "Escape"]) {
    await g.page.keyboard.press(key);
  }
  await g.nextFrame(4);

  const prevented = await g.page.evaluate(() => (window as unknown as { __keys: boolean[] }).__keys);
  expect(prevented.length, "the keys reached the page (a modifier is a key too)").toBe(12);
  expect(prevented.every((p) => !p), "the chart called preventDefault on none of them").toBe(true);
  const after = await g.status();
  expect(after.rowCount, "1 collapsed nothing").toBe(before.rowCount);
  expect(after.criticalOnly, "Shift+C changed no filter").toBe(before.criticalOnly);
  expect(after.longestPath, "Shift+L changed no filter").toBe(before.longestPath);
  expect(after.scrollY, "c scrolled nothing").toBe(before.scrollY);
  expect(after.selected, "c selected nothing").toEqual(before.selected);
  await expect(g.page.locator("#app #status-line"), "s did not show the status line").toBeHidden();
  expect(await g.page.evaluate(() => document.activeElement?.id), "focus stayed in the host input").toBe("host-input");
  expect(await g.page.locator("#host-input").inputValue(), "the host input took the typed text").toBe("1scCL");
  expect(chooser, "Ctrl+O opened no file chooser").toBe(false);
});

test("a key pressed in the open project picker is the dialog's, not the chart's", async ({ g }) => {
  await g.open({ n: SIZE });
  const before = await g.status();
  await g.page.evaluate(() => document.querySelector<HTMLDialogElement>("#picker")!.showModal());
  await g.page.locator("#picker-cancel").focus();
  for (const key of ["1", "s", "c", "Shift+C"]) await g.page.keyboard.press(key);
  await g.nextFrame(3);
  const after = await g.status();
  expect(after.rowCount, "1 collapsed nothing").toBe(before.rowCount);
  expect(after.criticalOnly).toBe(before.criticalOnly);
  await expect(g.page.locator("#app #status-line")).toBeHidden();
  await g.page.keyboard.press("Escape");
  await expect(g.page.locator("#picker")).toBeHidden();
});

test("the project picker is sized by the page, not by the chart's box", async ({ g }) => {
  await g.open({ n: SIZE });
  await embed(g, { width: 700, height: 300 }, { left: 20, top: 20 });
  await g.page.evaluate(() => document.querySelector<HTMLDialogElement>("#picker")!.showModal());
  const box = await g.page.locator("#picker").boundingBox();
  expect(box!.width, "560px at a 1280px window").toBe(560);
  await g.page.setViewportSize({ width: 400, height: 700 });
  await expect.poll(async () => (await g.page.locator("#picker").boundingBox())!.width).toBe(368);
  await g.page.keyboard.press("Escape");
});

test("a page's own elements outside the chart root keep their default style", async ({ g }) => {
  await g.open({ n: SIZE });
  const props = ["fontFamily", "fontSize", "color", "backgroundColor", "position", "margin", "padding", "display"] as const;
  // The probe: the same five elements, appended to a page, then their computed styles.
  const probe = (): Record<string, string[]> => {
    const out: Record<string, string[]> = {};
    for (const tag of ["button", "input", "pre", "aside", "div"]) {
      const el = document.createElement(tag);
      el.textContent = "x";
      document.body.append(el);
      const cs = getComputedStyle(el);
      out[tag] = ["fontFamily", "fontSize", "color", "backgroundColor", "position", "margin", "padding", "display"].map((p) => (cs as unknown as Record<string, string>)[p] ?? "");
      el.remove();
    }
    return out;
  };
  // The demo page sets its own body font as a style attribute; the host page is not the component's, so clear it for the comparison.
  await g.page.evaluate(() => {
    document.body.style.font = "";
  });
  const here = await g.page.evaluate(probe);
  const blank = await g.page.context().newPage();
  try {
    await blank.setContent("<!doctype html><html><body></body></html>");
    const there = await blank.evaluate(probe);
    for (const tag of Object.keys(there)) {
      props.forEach((p, i) => expect(here[tag]![i], `${tag} ${p} outside the root`).toBe(there[tag]![i]));
    }
  } finally {
    await blank.close();
  }
});

test("task names stay left-aligned inside a host that centres its text", async ({ g }) => {
  await g.open({ n: SIZE });
  await embed(g, { width: 1000, height: 400 });
  await g.page.evaluate(() => (document.getElementById("host")!.style.textAlign = "center"));
  const name = g.page.locator("#table-rows .row > .name").first();
  await expect(name).toBeVisible();
  expect(await name.evaluate((el) => getComputedStyle(el).textAlign), "the host's text-align does not reach the task names").toBe("start");
});
