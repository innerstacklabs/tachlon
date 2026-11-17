import { defineConfig } from "vite";

// Vite bundles what it finds in node_modules before it serves it, which would move the chart's script away from its engine file (app_bg.wasm).
export default defineConfig({ optimizeDeps: { exclude: ["tachlon"] } });
