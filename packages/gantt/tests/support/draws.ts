import { expect, type Page } from "@playwright/test";

/** True when the page's WebGPU adapter is SwiftShader, the CI runner's software adapter. */
export const onSwiftShader = (page: Page): Promise<boolean> =>
  page.evaluate(async () => {
    const info = (await (navigator as unknown as { gpu: { requestAdapter(): Promise<{ info?: { architecture?: string; description?: string } } | null> } }).gpu.requestAdapter())?.info;
    return /swiftshader/i.test(`${info?.architecture ?? ""} ${info?.description ?? ""}`);
  });

/** The canvas in the root `#id` shows a drawn chart: a screenshot of the canvas alone (every other element hidden, so a banner over it cannot count) holds more than a few colours. */
export async function draws(page: Page, id: string): Promise<boolean> {
  return coloursIn(page, await canvasPng(page, id));
}

const canvasPng = (page: Page, id: string): Promise<Buffer> =>
  page.locator(`#${id} canvas`).screenshot({ style: `* { visibility: hidden !important } #${id} canvas { visibility: visible !important }` });

async function coloursIn(page: Page, shot: Buffer): Promise<boolean> {
  const png = shot.toString("base64");
  return page.evaluate(async (png) => {
    const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${png}`)).blob());
    const scratch = Object.assign(document.createElement("canvas"), { width: bitmap.width, height: bitmap.height });
    const ctx = scratch.getContext("2d")!;
    ctx.drawImage(bitmap, 0, 0);
    const data = ctx.getImageData(0, 0, bitmap.width, bitmap.height).data;
    const colours = new Set<number>();
    for (let i = 0; i < data.length && colours.size < 8; i += 4 * 7) colours.add((data[i]! << 16) | (data[i + 1]! << 8) | data[i + 2]!);
    return colours.size >= 8;
  }, png);
}

/**
 * Chart `id` (the host's `__charts` entry) is drawing now: its bare canvas shows a chart, and after a zoom the canvas shows a different one.
 * A chart's test hook cannot say this: a switch or a lost device frees the hook's app, and `window.gantt` is only the last chart's.
 */
export async function expectDrawing(page: Page, id: string): Promise<void> {
  await expect.poll(() => draws(page, id), { message: `${id} draws`, timeout: 5_000 }).toBe(true);
  const before = await canvasPng(page, id);
  await page.evaluate((id) => (window as unknown as { __charts: Record<string, { commands: { zoomIn(): unknown } }> }).__charts[id]!.commands.zoomIn(), id);
  await expect.poll(async () => (await canvasPng(page, id)).equals(before), { message: `${id} draws again after a zoom`, timeout: 5_000 }).toBe(false);
}
