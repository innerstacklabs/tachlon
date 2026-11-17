// What the browser says about the GPU and itself, for the result's `environment`
// (plan gs#1, "Demo and measurements (M3)"). Part of the harness in `../measure.ts`.

/**
 * The WebGL2 renderer. The page's WebGL2 fallback context is created by wgpu
 * with only `antialias: false` set (wgpu-hal gles/web.rs), so the default power
 * preference is the matching one here. `known` is false without the unmasked
 * renderer string, since `RENDERER` alone is "WebKit WebGL".
 */
export function webglRenderer(): { text: string; known: boolean } {
  const gl = document.createElement("canvas").getContext("webgl2");
  if (!gl) return { text: "unavailable", known: false };
  const info = gl.getExtension("WEBGL_debug_renderer_info");
  const text = String(gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  return { text, known: info !== null };
}

interface GpuAdapterLike {
  isFallbackAdapter?: boolean;
  info?: { vendor?: string; architecture?: string; device?: string; description?: string; isFallbackAdapter?: boolean };
}

/**
 * The adapter the renderer gets. `crates/timeline/src/render.rs` requests
 * `PowerPreference::HighPerformance`, which wgpu passes on as
 * `powerPreference: "high-performance"`; asking for it here picks the same
 * adapter on a machine with two GPUs. `known` is false when the browser gives
 * no adapter or no name for it.
 */
export async function webgpuAdapter(): Promise<{ text: string; fallback: boolean; known: boolean }> {
  const gpu = (navigator as unknown as { gpu?: { requestAdapter(options: { powerPreference: string }): Promise<GpuAdapterLike | null> } }).gpu;
  const none = { text: "unavailable", fallback: false, known: false };
  if (!gpu) return none;
  try {
    const a = await gpu.requestAdapter({ powerPreference: "high-performance" });
    if (!a) return none;
    const i = a.info ?? {};
    const name = [i.vendor, i.architecture, i.device, i.description].filter(Boolean).join(" ");
    return { text: name || "unnamed adapter", fallback: Boolean(a.isFallbackAdapter ?? i.isFallbackAdapter), known: name !== "" };
  } catch {
    return none;
  }
}

export async function browserVersion(): Promise<string> {
  const data = (
    navigator as unknown as {
      userAgentData?: {
        brands?: { brand: string; version: string }[];
        getHighEntropyValues(hints: string[]): Promise<{ fullVersionList?: { brand: string; version: string }[] }>;
      };
    }
  ).userAgentData;
  if (!data) return navigator.userAgent;
  let list = data.brands ?? [];
  try {
    list = (await data.getHighEntropyValues(["fullVersionList"])).fullVersionList ?? list;
  } catch {
    // the major versions will do
  }
  const named = list.filter((b) => !/not.?a.?brand/i.test(b.brand)).map((b) => `${b.brand} ${b.version}`);
  return named.length > 0 ? named.join(", ") : navigator.userAgent;
}
