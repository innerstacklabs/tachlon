import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// The wasm-bindgen output (src/wasm/, built by scripts/build-web.sh) loads
// its .wasm through `new URL(..., import.meta.url)`, which Vite bundles as
// an asset; no plugin is needed. The developer page puts the app's chrome
// into the chart: `@gantt/chrome`, which reaches the chart as `gantt-core`, here the source (the hosted app loads the built package; gantt#1090).
const at = (path: string): string => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  resolve: { alias: { "gantt-core": at("src/index.ts"), "@gantt/chrome": at("../../apps/chrome/src/index.ts") } },
  build: { target: "es2022" },
  // The dev server serves the chrome's files from outside this project's root.
  server: { fs: { allow: [at("../..")] } },
  preview: { host: "127.0.0.1", port: Number(process.env.GANTT_PORT ?? 4173), strictPort: true },
});
