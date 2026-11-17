import type { Page } from "@playwright/test";

type Counts = { adapter: number; device: number };
type Gpu = { requestAdapter(...args: unknown[]): Promise<{ requestDevice(...args: unknown[]): Promise<unknown> } | null> };

/** Count the page's `requestAdapter` and `requestDevice` calls from its first script on; `gpuRequests` reads them. */
export const countGpuRequests = async (page: Page): Promise<void> => {
  await page.addInitScript(() => {
    const counts = ((window as unknown as { __gpu: Counts }).__gpu = { adapter: 0, device: 0 });
    const gpu = (navigator as unknown as { gpu?: Gpu }).gpu;
    const request = gpu?.requestAdapter.bind(gpu);
    if (gpu && request) {
      gpu.requestAdapter = async (...args) => {
        counts.adapter += 1;
        const adapter = await request(...args);
        const device = adapter?.requestDevice.bind(adapter);
        if (adapter && device) adapter.requestDevice = (...rest) => ((counts.device += 1), device(...rest));
        return adapter;
      };
    }
  });
};

export const gpuRequests = (page: Page): Promise<Counts> => page.evaluate(() => (window as unknown as { __gpu: Counts }).__gpu);
