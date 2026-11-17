import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { type Plugin, defineConfig } from "vite";

// The app loads the chart as a customer does: the built package, `gantt-core`, linked from `packages/gantt/` in this project's own
// install (not a workspace: `packages/gantt/` keeps its own lockfile), so `pnpm build:lib` runs there first. No alias names it
// (architecture check 12). The package's .wasm is loaded through `new URL(..., import.meta.url)`, which Vite bundles as an asset.

// The page's static <title> keeps a neutral name in the source (decision 0013); the build writes PRODUCT_NAME, read
// from the one file that holds it, so the tab is right before any script runs.
const product = readFileSync(new URL("../../packages/gantt/src/product.ts", import.meta.url), "utf8");
const productName = /export const PRODUCT_NAME = "([^"]+)";/.exec(product)?.[1];
if (productName === undefined) throw new Error("vite.config.ts: no PRODUCT_NAME in packages/gantt/src/product.ts");
const productTitle: Plugin = {
  name: "product-title",
  transformIndexHtml: (html) => html.replace(/<title>[^<]*<\/title>/, `<title>${productName}</title>`),
};

// The engine (the chart's .wasm file) is the longest download. The built page names it in a preload, so the browser asks for it at
// navigation, before main.ts runs, and the chart's own request takes that one over (gs#1336). The build knows the hashed name; the
// dev server has no bundle, so it adds nothing.
const enginePreload: Plugin = {
  name: "engine-preload",
  transformIndexHtml: (_html, ctx) => {
    const wasm = Object.keys(ctx.bundle ?? {}).find((name) => name.endsWith(".wasm"));
    if (wasm === undefined) return [];
    // The inline script compiles that response as it arrives and keeps the promise for `loadEngine` (gs#1354): it does not wait for the module script.
    const early = `window.__engineModule=WebAssembly.compileStreaming(fetch(${JSON.stringify(`./${wasm}`)}));window.__engineModule.catch(function(){});`;
    return [
      { tag: "link", injectTo: "head-prepend", attrs: { rel: "preload", as: "fetch", type: "application/wasm", crossorigin: "", href: `./${wasm}` } },
      { tag: "script", injectTo: "head-prepend", children: `if(window.WebAssembly&&WebAssembly.compileStreaming){${early}}` },
    ];
  },
};

export default defineConfig({
  // Relative asset URLs, so the build works at `/` (the site root, the demos, the screencast, the tests) and under
  // any path prefix a host puts it behind. The sample files are fetched as `samples/...`, relative to the page.
  base: "./",
  plugins: [productTitle, enginePreload],
  build: { target: "es2022" },
  // The dev server serves the linked package and the chrome's files, which lie outside this project's root.
  server: { fs: { allow: [fileURLToPath(new URL("../..", import.meta.url))] } },
  preview: { host: "127.0.0.1", port: Number(process.env.GANTT_PORT ?? 4173), strictPort: true },
});
