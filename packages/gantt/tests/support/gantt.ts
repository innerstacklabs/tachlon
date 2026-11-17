import { expect, test as base, type Page } from "@playwright/test";
import type { Status } from "../../src/api";
import { backend, n, url } from "./knobs";

export async function acrossReload<T>(page: Page, run: () => Promise<T>, attempts = 3): Promise<T> {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return await run();
    } catch (error) {
      if (attempt >= attempts || !/context was destroyed|navigat/i.test(String(error))) throw error;
      await page.waitForLoadState("load");
    }
  }
}

/** The page's own font (gs#529): wait until `document.fonts` has settled, then fail if the bundled "Gantt Sans" file is not the loaded face. A spec that measures text must never measure a system font. */
export async function waitFonts(page: Page): Promise<void> {
  // A page that has just reloaded (the WebGPU to WebGL2 fallback) has no font faces yet, and reading them then reported
  // "no faces". Wait for the face to load first; if it never does, the check below fails with its own message.
  await acrossReload(page, () =>
    page
      .waitForFunction(
        () => [...document.fonts].some((face) => face.family.replace(/"/g, "") === "Gantt Sans" && face.status === "loaded"),
        undefined,
        { timeout: 15_000 },
      )
      .catch((error: unknown) => {
        if (/context was destroyed|navigat/i.test(String(error))) throw error;
      }),
  );
  await acrossReload(page, () =>
    page.evaluate(async () => {
      await document.fonts.ready;
      const faces = [...document.fonts].filter((face) => face.family.replace(/"/g, "") === "Gantt Sans");
      if (!faces.some((face) => face.status === "loaded")) {
        throw new Error(`the bundled font is not loaded: ${[...document.fonts].map((f) => `${f.family}:${f.status}`).join(", ") || "no faces"}`);
      }
    }),
  );
}

export async function waitReady(
  page: Page,
  { frames, timeout }: { frames: number; timeout: number },
): Promise<void> {
  await acrossReload(page, () =>
    page.waitForFunction((k) => window.gantt?.ready && window.gantt.status().frames >= k, frames, { timeout }),
  );
  await waitFonts(page);
}

/**
 * Destroy the page's WebGPU device once the chart has drawn `frame` frames, to force the GPU device loss that makes the
 * app reload itself onto WebGL2 (gs#1159). Call it before `page.goto`. Only the first document is hit: the fallback page
 * asks for no device, so it never loses one. `holdMs` holds that fallback document back, as a slow runner does; the
 * old document stays up and stops drawing while it waits.
 */
export async function loseDeviceAtFrame(page: Page, frame: number, holdMs = 0): Promise<void> {
  await page.addInitScript((target: number) => {
    if (!navigator.gpu) return;
    let device: GPUDevice | null = null;
    const request = GPUAdapter.prototype.requestDevice;
    GPUAdapter.prototype.requestDevice = async function (this: GPUAdapter, ...args: Parameters<typeof request>): Promise<GPUDevice> {
      device = await request.apply(this, args);
      return device;
    };
    const timer = setInterval(() => {
      const status = (window as unknown as { gantt?: { status(): { frames: number } } }).gantt?.status();
      if (device && status && status.frames >= target) {
        clearInterval(timer);
        device.destroy();
      }
    }, 20);
  }, frame);
  if (holdMs > 0) {
    await page.route(
      (target) => target.searchParams.has("fallback"),
      async (route) => {
        await new Promise((done) => setTimeout(done, holdMs));
        await route.continue();
      },
    );
  }
}

export function pageErrors(logs: string[]): string[] {
  return logs.filter((line) => /^(error|pageerror)/.test(line));
}

export function trackLogs(page: Page): string[] {
  const logs: string[] = [];
  page.on("console", (message) => logs.push(`${message.type()}: ${message.text()}`));
  page.on("pageerror", (error) => logs.push(`pageerror: ${error.message}`));
  return logs;
}

/** Open the toolbar's Filters menu (gs#808) if it is shut: the filter controls sit in its panel and cannot be pressed while it is closed. It opens by key, so a pointer press held on the chart stays held. */
export async function openFilters(page: Page): Promise<void> {
  if (await page.locator("#filters-panel").isHidden()) {
    await page.focus("#filters-button");
    await page.keyboard.press("Enter");
  }
}

/** Close the Filters menu if it is open (Escape inside it, so the chart's own Escape does not run). */
export async function closeFilters(page: Page): Promise<void> {
  if (await page.locator("#filters-panel").isVisible()) await page.keyboard.press("Escape");
}

export interface Gantt {
  page: Page;
  logs: string[];
  open(opts?: { n?: number; params?: Record<string, string>; frames?: number; timeout?: number }): Promise<void>;
  status(): Promise<Status>;
  nextFrame(k?: number): Promise<void>;
  box(): Promise<{ x: number; y: number; width: number; height: number }>;
}

export const test = base.extend<{ g: Gantt; webgl: boolean }>({
  // A file that checks keys, files or timing, not the GPU, sets `test.use({ webgl: true })`: its pages start on WebGL2, so a
  // WebGPU device lost on a loaded runner cannot reload the page under the test (gs#1159, gantt#1163).
  webgl: [false, { option: true }],
  g: async ({ page, webgl }, use, testInfo) => {
    const logs = trackLogs(page);
    // Keep Playwright's file-chooser interception on for the page's whole life (gs#198). `waitForEvent("filechooser")`
    // turns it on when it registers and off when it resolves or times out, with a CDP `Page.setInterceptFileChooserDialog`
    // each way and no acknowledgement to wait on. A chooser requested right after a previous one resolved (Ctrl+O after the
    // Open button) was sometimes opened without a `filechooser` event, and the test waited out its timeout. With a listener
    // always present the interception is switched on once, here, before any test step runs.
    page.on("filechooser", () => {});
    let opened = false;
    const g: Gantt = {
      page,
      logs,
      async open(opts = {}) {
        opened = true;
        const query = new URLSearchParams({ n: String(opts.n ?? n) });
        if (backend === "webgl" || webgl) query.set("backend", "webgl");
        for (const [key, value] of Object.entries(opts.params ?? {})) query.set(key, value);
        await page.goto(`${url}?${query}`);
        await waitReady(page, { frames: opts.frames ?? 30, timeout: opts.timeout ?? 180_000 });
      },
      status: () => page.evaluate(() => window.gantt!.status()),
      nextFrame: (k = 2) =>
        page.evaluate(
          (frameCount) =>
            new Promise<void>((done) => {
              const target = window.gantt!.status().frames + frameCount;
              const wait = (): void => {
                if (window.gantt!.status().frames >= target) done();
                else requestAnimationFrame(wait);
              };
              requestAnimationFrame(wait);
            }),
          k,
        ),
      async box() {
        const bounds = await page.locator("#timeline").boundingBox();
        if (!bounds) throw new Error("#timeline has no bounding box");
        return bounds;
      },
    };

    try {
      await use(g);
    } finally {
      if (opened) {
        await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
        const errors = pageErrors(logs);
        expect.soft(
          errors.length === 0,
          `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`,
        ).toBe(true);
      }
    }
  },
});

export { expect };
