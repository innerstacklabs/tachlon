import type { Page } from "@playwright/test";
import { acrossReload, expect, loseDeviceAtFrame, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { dpr, loseDeviceFrame, loseDeviceHoldMs, n, url } from "./support/knobs";
import { AXIS, countColour, decodePng } from "./support/pixels";

// The theme journey that verify.mjs (removed in 3f191ea6) used to run (gs#97 P2): the token table at load, root style overrides
// flowing through the observers in main.ts, set_theme refusing a bad update, and the colour-scheme change. One 50k page
// is shared by the whole file (ruling devteam-1-90): the tests run in order on it and each leaves it as it found it.

import { ROW } from "./support/pixels";

const THEME_TOKENS = [
  "bg",
  "surface",
  "band",
  "rule",
  "text",
  "muted",
  "bar",
  "bar-edge",
  "crit-fill",
  "crit",
  "crit-edge",
  "bar-soft",
  "crit-soft",
  "bar-ink",
  "crit-text",
  "sum",
  "sel",
  "sel-edge",
  "hov",
  "link",
  "today",
  "grid",
  "grid-major",
  "axis",
  "nonwork",
  "stripe",
  "bar-text",
  "bar-sel-progress",
  "held",
  "link-refused",
  "link-terminal",
  "baseline",
];

// The production CSS minifier writes #ffffff as #fff and #ffffff99 as #fff9: compare the colours, not the spelling.
const expandCss = (css: Record<string, string>): Record<string, string> =>
  Object.fromEntries(
    Object.entries(css).map(([key, value]) => [
      key,
      /^#[0-9a-f]{3,4}$/i.test(value) ? `#${[...value.slice(1)].map((c) => c + c).join("")}`.toLowerCase() : value.toLowerCase(),
    ]),
  );

interface ThemeState {
  tokens: string[];
  app: Record<string, string>;
  hook: Record<string, string>;
  css: Record<string, string>;
}

const themeState = (page: Page): Promise<ThemeState> =>
  page.evaluate(() => {
    const tokens = JSON.parse((window.gantt!.app.constructor as unknown as { theme_tokens(): string }).theme_tokens()) as string[];
    const style = getComputedStyle(document.querySelector(".gantt")!);
    return {
      tokens,
      app: JSON.parse(window.gantt!.app.theme()) as Record<string, string>,
      hook: window.gantt!.theme(),
      css: Object.fromEntries(tokens.map((token) => [token, style.getPropertyValue(`--gantt-${token}`).trim()])),
    };
  });

const framePair = (page: Page): Promise<void> =>
  page.evaluate(
    () =>
      new Promise<void>((done) => {
        requestAnimationFrame(() => requestAnimationFrame(() => done()));
      }),
  );

const nextFrame = (page: Page, k = 2): Promise<void> =>
  page.evaluate(
    (frameCount) =>
      new Promise<void>((done) => {
        const target = window.gantt!.status().frames + frameCount;
        const wait = (): void => (window.gantt!.status().frames >= target ? done() : void requestAnimationFrame(wait));
        requestAnimationFrame(wait);
      }),
    k,
  );

/** Zoom to the year level with ctrl+wheel steps at the canvas, then scroll and pan until the first critical row is near the top. */
async function showCriticalRow(page: Page): Promise<void> {
  const box = await page.locator("#timeline").boundingBox();
  if (!box) throw new Error("#timeline has no bounding box");
  const cx = box.x + box.width / 2;
  const cy = box.y + AXIS + 10 * ROW;
  for (let i = 0; i < 200 && (await page.evaluate(() => window.gantt!.status().level)) !== "year"; i++) {
    await page.mouse.move(cx, cy);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, 120);
    await page.keyboard.up("Control");
    await nextFrame(page);
  }
  expect(await page.evaluate(() => window.gantt!.status().level), "the page reached the year level").toBe("year");
  // The earliest rows need not hold a critical task: ask the engine for the first critical row, then scroll and pan
  // (plain wheel) until that row is near the top and its start is a fifth of the way across.
  const criticalRow = await page.evaluate(() => window.gantt!.app.test_first_critical_row() ?? null);
  expect(criticalRow, "the schedule has a critical row").not.toBeNull();
  const es = await page.evaluate((r) => window.gantt!.app.row_start(r), criticalRow!);
  expect(es, "the first critical row has a start").not.toBeUndefined();
  const target = Math.max(0, (criticalRow! - 5) * ROW);
  // The loop runs inside the page, on the engine's own wheel call. A round trip and two frames per step (about 515 steps) took
  // 51 s on the hub and, on a loaded runner, ran the beforeAll hook out of time (gantt#1163).
  await page.evaluate(
    ({ es, target, width, x, y }) => {
      const { app, status } = window.gantt!;
      let before = Infinity;
      for (let i = 0; i < 1000; i++) {
        const dy = target - status().scrollY;
        const dx = app.x_of(es) - width / 5;
        const off = Math.abs(dx) + Math.abs(dy);
        if (off < 1 || off >= before) break; // there, or clamped
        before = off;
        const step = (d: number): number => Math.sign(d) * Math.min(Math.abs(d), 2000);
        app.wheel(x, y, step(dx), step(dy), false);
      }
    },
    { es: es!, target, width: box.width, x: box.width / 2, y: AXIS + 10 * ROW },
  );
  await nextFrame(page);
  const view = await page.evaluate(() => window.gantt!.status());
  expect(view.criticalBars, `${view.criticalBars} critical bars in view on rows ${view.rows.join("-")}`).toBeGreaterThan(0);
}

test.describe.serial("theme journey", () => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  let page: Page;
  let logs: string[];
  let initialTheme: ThemeState;

  test.beforeAll(async ({ browser }) => {
    page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dpr });
    logs = trackLogs(page);
    if (loseDeviceFrame > 0) await loseDeviceAtFrame(page, loseDeviceFrame, loseDeviceHoldMs);
    const query = new URLSearchParams({ n: String(n) });
    // WebGL2 from the start (gs#1159): a WebGPU device lost mid-setup reloads the whole 50k page onto WebGL2, and on a loaded
    // runner that second load, plus the setup done again on it, ran the beforeAll hook out of time. The tokens and the
    // colours read here are the same on either backend, and the page never holds a device it could lose.
    query.set("backend", "webgl");
    await page.goto(`${url}?${query}`);
    await waitReady(page, { frames: 30, timeout: 180_000 });
    initialTheme = await acrossReload(page, () => themeState(page));
    await showCriticalRow(page);
  });

  test.afterAll(async ({}, testInfo) => {
    await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
    const errors = pageErrors(logs);
    expect.soft(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`).toBe(true);
    await page.close();
  });

  test("the token table has the 32 stable entries and the light anchors at load", async () => {
    const lightTheme = initialTheme.app;
    expect
      .soft(
        initialTheme.tokens.length === 32 && JSON.stringify(initialTheme.tokens) === JSON.stringify(THEME_TOKENS),
        "theme: App.theme_tokens() has the 32 stable entries",
      )
      .toBe(true);
    expect
      .soft(
        JSON.stringify(initialTheme.hook) === JSON.stringify(lightTheme) &&
          JSON.stringify(expandCss(initialTheme.css)) === JSON.stringify(lightTheme),
        "theme: gantt.theme() and all root CSS tokens equal the light table at load",
      )
      .toBe(true);
    expect.soft(lightTheme.bg === "#f2f5f9" && lightTheme.crit === "#c42b2b", "theme: light table has the product palette's bg and crit anchors (tl#1193)").toBe(true);
  });

  test("root style overrides reach the scene and the DOM through the observers, and removing them restores both", async ({}, testInfo) => {
    // Style mutations must flow through the observers in main.ts, without calling the syncTheme test hook.
    await page.evaluate(() => {
      const root = document.querySelector<HTMLElement>(".gantt")!;
      root.style.setProperty("--gantt-crit", "#00aa00");
      root.style.setProperty("--gantt-crit-fill", "#00aa00");
      root.style.setProperty("--gantt-crit-edge", "#00aa00");
      root.style.setProperty("--gantt-axis", "#ffeecc");
    });
    await framePair(page);
    const overriddenTheme = await page.evaluate(() => ({
      theme: window.gantt!.theme(),
      axis: getComputedStyle(document.querySelector(".gantt #table-head")!).backgroundColor,
    }));
    const overriddenShot = await page.locator("#timeline").screenshot();
    await testInfo.attach("overridden-timeline", { body: overriddenShot, contentType: "image/png" });
    const overriddenPng = decodePng(overriddenShot);
    const overriddenCritical = countColour(overriddenPng, [0x00, 0xaa, 0x00], 6);
    // The light `crit-fill` is #e04f47 (tl#1193); `crit`, `crit-fill` and `crit-edge` are all overridden, so it may not show.
    const oldCritical = countColour(overriddenPng, [0xe0, 0x4f, 0x47], 6);
    expect.soft(overriddenTheme.axis === "rgb(255, 238, 204)", "theme: a root axis override updates the scoped DOM style").toBe(true);
    expect
      .soft(overriddenTheme.theme.crit === "#00aa00", "theme: a root crit override reaches the scene theme through MutationObserver")
      .toBe(true);
    expect
      .soft(
        overriddenCritical > 0 && oldCritical === 0,
        `theme: the canvas uses only the overridden critical colour (${overriddenCritical} green, ${oldCritical} old-red pixels)`,
      )
      .toBe(true);

    await page.evaluate(() => {
      const root = document.querySelector<HTMLElement>(".gantt")!;
      root.style.removeProperty("--gantt-crit");
      root.style.removeProperty("--gantt-crit-fill");
      root.style.removeProperty("--gantt-crit-edge");
      root.style.removeProperty("--gantt-axis");
    });
    await framePair(page);
    const restoredOverrides = await page.evaluate(() => ({
      theme: window.gantt!.theme(),
      axis: getComputedStyle(document.querySelector(".gantt #table-head")!).backgroundColor,
    }));
    expect
      .soft(
        JSON.stringify(restoredOverrides.theme) === JSON.stringify(initialTheme.app) && restoredOverrides.axis === "rgb(232, 237, 244)",
        "theme: removing root overrides restores the light scene and DOM colours",
      )
      .toBe(true);
  });

  test("set_theme throws on an invalid colour or an unknown token and leaves the theme as it was", async () => {
    const rejectedThemes = await page.evaluate(() => {
      const before = window.gantt!.theme();
      let badColourThrew = false;
      let unknownKeyThrew = false;
      try {
        window.gantt!.app.set_theme('{"crit":"banana"}');
      } catch {
        badColourThrew = true;
      }
      const afterBadColour = window.gantt!.theme();
      try {
        window.gantt!.app.set_theme('{"nope":"#000000"}');
      } catch {
        unknownKeyThrew = true;
      }
      return { before, afterBadColour, afterUnknownKey: window.gantt!.theme(), badColourThrew, unknownKeyThrew };
    });
    expect.soft(rejectedThemes.badColourThrew, "theme: set_theme throws on an invalid colour").toBe(true);
    expect.soft(rejectedThemes.unknownKeyThrew, "theme: set_theme throws on an unknown token").toBe(true);
    expect
      .soft(
        JSON.stringify(rejectedThemes.before) === JSON.stringify(rejectedThemes.afterBadColour) &&
          JSON.stringify(rejectedThemes.before) === JSON.stringify(rejectedThemes.afterUnknownKey),
        "theme: rejected updates are atomic",
      )
      .toBe(true);
  });

  test("a dark colour scheme applies the dark tokens to the scene and a light one restores the light tokens", async ({}, testInfo) => {
    try {
      await page.emulateMedia({ colorScheme: "dark" });
      await framePair(page);
      const darkState = await themeState(page);
      const darkTheme = darkState.app;
      expect
        .soft(
          JSON.stringify(darkState.hook) === JSON.stringify(darkTheme) &&
            JSON.stringify(expandCss(darkState.css)) === JSON.stringify(darkTheme),
          "theme: the matchMedia path applies all dark tokens to the scene",
        )
        .toBe(true);
      expect.soft(darkTheme.bg === "#0e1630" && darkTheme.crit === "#f0544a", "theme: dark table has the product palette's bg and crit anchors (tl#1193)").toBe(true);
      const darkShot = await page.locator("#timeline").screenshot();
      await testInfo.attach("dark-timeline", { body: darkShot, contentType: "image/png" });
      const darkPng = decodePng(darkShot);
      // The empty-row fills, read from the dark theme itself so a palette change cannot leave stale copies here (tl#1193).
      const channels = (hex: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];
      const darkClear = (
        [darkTheme.bg, darkTheme.stripe, darkTheme.hov].map((hex) => [hex, channels(hex)]) as [string, [number, number, number]][]
      ).find(([, rgb]) => countColour(darkPng, rgb, 2) > 0);
      expect.soft(darkClear !== undefined, `theme: the dark canvas has an empty-row background pixel (${darkClear?.[0] ?? "none"})`).toBe(true);
    } finally {
      await page.emulateMedia({ colorScheme: "light" });
    }
    await framePair(page);
    const restoredLight = await page.evaluate(() => window.gantt!.theme());
    expect
      .soft(
        JSON.stringify(restoredLight) === JSON.stringify(initialTheme.app),
        "theme: returning to a light colour scheme restores all light tokens",
      )
      .toBe(true);
  });

  for (const scheme of ["light", "dark"] as const) {
    test(`the message line and the result link take the chart's tokens in ${scheme}`, async () => {
      try {
        await page.emulateMedia({ colorScheme: scheme });
        await framePair(page);
        // One synchronous read: the loop rewrites the message line on its next frame, so no frame may fall between the set and the read.
        const read = await page.evaluate(() => {
          const probe = document.createElement("span");
          document.querySelector(".gantt")!.appendChild(probe);
          const resolve = (token: string): string => {
            probe.style.color = `var(--gantt-${token})`;
            return getComputedStyle(probe).color;
          };
          const line = document.getElementById("overlay")!;
          const link = document.querySelector<HTMLElement>("[data-part=result-link]")!;
          const before = [line.textContent, link.hidden] as const;
          line.textContent = "loading the chart";
          link.hidden = false;
          const [ls, ks] = [getComputedStyle(line), getComputedStyle(link)];
          const out = {
            line: { bg: ls.backgroundColor, color: ls.color, border: ls.borderTopColor },
            link: { bg: ks.backgroundColor, color: ks.color },
            tokens: Object.fromEntries(["surface", "text", "rule", "sel-edge", "bar-text"].map((t) => [t, resolve(t)])),
          };
          [line.textContent, link.hidden] = before;
          probe.remove();
          return out;
        });
        const t = read.tokens;
        expect(read.line.bg, `message line background ${read.line.bg} equals --gantt-surface ${t.surface}`).toBe(t.surface);
        expect(read.line.color, `message line text ${read.line.color} equals --gantt-text ${t.text}`).toBe(t.text);
        expect(read.line.border, `message line border ${read.line.border} equals --gantt-rule ${t.rule}`).toBe(t.rule);
        expect(read.link.bg, `result link background ${read.link.bg} equals --gantt-sel-edge ${t["sel-edge"]}`).toBe(t["sel-edge"]);
        expect(read.link.color, `result link text ${read.link.color} equals --gantt-bar-text ${t["bar-text"]}`).toBe(t["bar-text"]);
      } finally {
        await page.emulateMedia({ colorScheme: "light" });
        await framePair(page);
      }
    });

    test(`the canvas notes take the chart's tokens and the dialog scrims take --gantt-scrim in ${scheme}`, async () => {
      try {
        await page.emulateMedia({ colorScheme: scheme });
        await framePair(page);
        // Probe elements wear each part's data-part, so the read never waits for the app to show a note or open a dialog.
        const read = await page.evaluate(() => {
          const root = document.querySelector(".gantt")!;
          const probe = document.createElement("span");
          root.appendChild(probe);
          const resolve = (prop: "color" | "backgroundColor", token: string): string => {
            probe.style[prop] = `var(--gantt-${token})`;
            return getComputedStyle(probe)[prop];
          };
          const notes = ["moved-note", "moved-show", "baseline-summary", "path-line", "drag-note", "save-note", "offscreen-note"].map((part) => {
            const el = document.createElement("div");
            el.dataset.part = part;
            el.textContent = "x";
            root.appendChild(el);
            const cs = getComputedStyle(el);
            const out = { part, bg: cs.backgroundColor, color: cs.color, border: cs.borderTopColor, width: cs.borderTopWidth };
            el.remove();
            return out;
          });
          const scrims = ["picker", "keys-help", "export-dialog"].map((part) => {
            const el = document.createElement("dialog");
            el.dataset.part = part;
            root.appendChild(el);
            el.showModal();
            const out = { part, bg: getComputedStyle(el, "::backdrop").backgroundColor };
            el.close();
            el.remove();
            return out;
          });
          const out = { notes, scrims, surface: resolve("backgroundColor", "surface"), text: resolve("color", "text"), muted: resolve("color", "muted"), scrim: resolve("backgroundColor", "scrim") };
          probe.remove();
          return out;
        });
        expect(read.notes.length, "seven note parts were read").toBe(7);
        for (const note of read.notes) {
          expect(note.bg, `${note.part} background ${note.bg} equals --gantt-surface ${read.surface}`).toBe(read.surface);
          expect(note.color, `${note.part} text ${note.color} equals --gantt-text ${read.text}`).toBe(read.text);
          expect(note.border, `${note.part} border ${note.border} equals --gantt-muted ${read.muted}`).toBe(read.muted);
          expect(note.width, `${note.part} has a border`).toBe("1px");
        }
        expect(read.scrims.length, "three dialog parts were read").toBe(3);
        for (const scrim of read.scrims) expect(scrim.bg, `${scrim.part} backdrop ${scrim.bg} equals --gantt-scrim ${read.scrim}`).toBe(read.scrim);
      } finally {
        await page.emulateMedia({ colorScheme: "light" });
        await framePair(page);
      }
    });

    test(`the hover card takes the chart's surface and text tokens in ${scheme}`, async () => {
      try {
        await page.emulateMedia({ colorScheme: scheme });
        await framePair(page);
        // The critical row sits five rows down with its bar a fifth of the way across: sweep that row until the card shows.
        const box = await page.locator("#timeline").boundingBox();
        if (!box) throw new Error("#timeline has no bounding box");
        const y = box.y + AXIS + 5 * ROW + ROW / 2;
        let shown = false;
        for (let x = box.x + 4; x < box.x + box.width - 3 && !shown; x += 6) {
          await page.mouse.move(x, y);
          await nextFrame(page, 1);
          shown = await page.locator("#hover-card").isVisible();
        }
        expect(shown, "a bar under the pointer shows the card").toBe(true);
        const card = await page.evaluate(() => {
          const cs = getComputedStyle(document.getElementById("hover-card")!);
          // Resolve a token to a computed rgb() through a probe element.
          const probe = document.createElement("span");
          document.querySelector(".gantt")!.appendChild(probe);
          const resolve = (token: string): string => {
            probe.style.color = `var(--gantt-${token})`;
            return getComputedStyle(probe).color;
          };
          const colourOf = (selector: string): string => {
            const el = document.querySelector(`#hover-card ${selector}`);
            return el ? getComputedStyle(el).color : "missing";
          };
          const out = {
            bg: cs.backgroundColor,
            color: cs.color,
            surface: resolve("surface"),
            text: resolve("text"),
            muted: resolve("muted"),
            critText: resolve("crit-text"),
            outlinePath: colourOf(".outline-path"),
            label: colourOf(".label"),
            critical: colourOf(".value.critical"),
            borderWidth: cs.borderTopWidth,
            borderColour: cs.borderTopColor,
          };
          probe.remove();
          return out;
        });
        const lum = (rgb: string): number => {
          const [r, g, b] = (rgb.match(/\d+/g) ?? []).slice(0, 3).map((v) => {
            const c = Number(v) / 255;
            return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
          });
          return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
        };
        const [hi, lo] = [lum(card.color), lum(card.bg)].sort((a, b) => b - a);
        const ratio = (hi! + 0.05) / (lo! + 0.05);
        expect(card.bg, `card background ${card.bg} equals --gantt-surface ${card.surface}`).toBe(card.surface);
        expect(card.color, `card text ${card.color} equals --gantt-text ${card.text}`).toBe(card.text);
        expect(card.outlinePath, `outline path ${card.outlinePath} equals --gantt-muted ${card.muted}`).toBe(card.muted);
        expect(card.label, `label ${card.label} equals --gantt-muted ${card.muted}`).toBe(card.muted);
        expect(card.critical, `critical value ${card.critical} equals --gantt-crit-text ${card.critText}`).toBe(card.critText);
        expect(card.borderWidth, "the card has a border").not.toBe("0px");
        expect(card.borderColour, `border ${card.borderColour} equals --gantt-muted ${card.muted}`).toBe(card.muted);
        expect(ratio, `text on the card is ${ratio.toFixed(2)}:1`).toBeGreaterThanOrEqual(4.5);
      } finally {
        await page.mouse.move(0, 0);
        await page.emulateMedia({ colorScheme: "light" });
        await framePair(page);
      }
    });
  }
});
