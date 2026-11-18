import { defineConfig } from "vite";

// Vite bundles what it finds in node_modules before it serves it, which would move the chart's script away from its engine file (app_bg.wasm).
// The `/react` entry stays out of that step too, so its own import of React's JSX runtime needs the step: it is listed here.
export default defineConfig({ optimizeDeps: { exclude: ["tachlon", "tachlon/react"], include: ["react/jsx-runtime"] } });
