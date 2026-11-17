// What a run ran on, and why it might not be a result.
//
// The GPU and the display rate are read on a blank page of their own, before the side's page opens,
// so no side's code or load is in them: WebGL's unmasked vendor and renderer, and the median
// `requestAnimationFrame` gap over one idle second. Whether a side drew with WebGPU is the side's to
// say (`sideBackend`): a blank page is not a secure context, so it cannot see `navigator.gpu`.

import { percentile, startRecorder, stopRecorder } from "./frames.ts";

export interface Gpu {
  vendor: string | null;
  renderer: string | null;
}

export interface Environment {
  chrome: string;
  gpu: Gpu;
  viewport: { width: number; height: number };
  dpr: number;
  refreshHz: number;
  measuredHz: number | null;
  headless: boolean;
  os: string;
  node: string;
  /** What the side says it drew with, if it says. */
  sideBackend: string | null;
  /** What the side says about its build: true when it is not a production build (development, debug, unminified trial); null when it does not say. */
  debugBuild: boolean | null;
  /** The build the side names, as a table labels it; null when it does not say. */
  sideBuild: string | null;
  /** Whole rows the view shows at once, when the side says. */
  rowsOnScreen: number | null;
}

/** Read the GPU, the device pixel ratio and the display rate on a fresh blank page of `context`. */
export async function probeBlank(context: any): Promise<{ gpu: Gpu; dpr: number; measuredHz: number | null }> {
  const page = await context.newPage();
  try {
    const gpu: Gpu = await page.evaluate(() => {
      const gl = document.createElement("canvas").getContext("webgl2") ?? document.createElement("canvas").getContext("webgl");
      const info = gl?.getExtension("WEBGL_debug_renderer_info");
      return {
        vendor: gl && info ? String(gl.getParameter(info.UNMASKED_VENDOR_WEBGL)) : null,
        renderer: gl && info ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL)) : null,
      };
    });
    await page.evaluate(startRecorder);
    await new Promise((r) => setTimeout(r, 1000));
    const median = percentile(await page.evaluate(stopRecorder), 50);
    const dpr: number = await page.evaluate(() => window.devicePixelRatio);
    return { gpu, dpr, measuredHz: Number.isFinite(median) && median > 0 ? Math.round(10000 / median) / 10 : null };
  } finally {
    await page.close();
  }
}

/** Why this run is not a result; empty when it is one. Phase checks that failed are added by the driver. */
export function warningsFor(env: Pick<Environment, "headless" | "gpu" | "refreshHz" | "measuredHz" | "debugBuild">): string[] {
  const w: string[] = [];
  if (env.headless) w.push("headless: no window, software rendering; a smoke run, not a result");
  if (env.gpu.renderer === null) w.push("unknown acceleration: the page reports no GPU renderer");
  else if (/swiftshader|llvmpipe|softpipe|software/i.test(env.gpu.renderer)) w.push(`software renderer: ${env.gpu.renderer}`);
  if (env.measuredHz !== null && Math.abs(env.measuredHz - env.refreshHz) > env.refreshHz * 0.1) {
    w.push(`the page ran at ${env.measuredHz} Hz, not the --refresh-hz ${env.refreshHz} the long gaps are counted at`);
  }
  if (env.debugBuild === true) w.push("debug build: the side says this page is not its production build");
  return w;
}
