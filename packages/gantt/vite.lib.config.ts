import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { PRODUCT_NAME } from "./src/product.ts";

const at = (path: string): string => fileURLToPath(new URL(path, import.meta.url));

// The library build: the ES modules of the five entries and one stylesheet in dist-lib/, checked by scripts/pack-check.ts. Library mode inlines every asset,
// so the plugin marks the fonts ?no-inline: they land beside gantt.css as files, which names them with url() relative to itself
// (base "./" and assetsDir "" keep that path to ./name.woff2). The same goes for the engine: both places that name app_bg.wasm with
// `new URL(..., import.meta.url)` (the loader in mount-load.ts and wasm-bindgen's own default in wasm/app.js) get ?no-inline, so
// dist-lib/ holds the engine once as a file that the script loads by a URL relative to itself, not twice as base64 inside it.
export default defineConfig({
  base: "./",
  plugins: [
    {
      // The Angular entry is what `ngc` wrote (dist-ng/, `pnpm build:lib` runs it first); its `./index.js` is the chart's own module, so the entries share one copy of the chart.
      name: "angular-from-ngc",
      enforce: "pre",
      resolveId: (source, importer) => (source === "./index.js" && importer === at("dist-ng/angular.js") ? at("src/index.ts") : null),
    },
    {
      name: "fonts-as-files",
      enforce: "pre",
      transform: (code, id) => (id.endsWith(".css") ? code.replace(/\.woff2(["')])/g, ".woff2?no-inline$1") : null),
    },
    {
      name: "engine-as-file",
      enforce: "pre",
      transform: (code, id) => (/\.[jt]s$/.test(id) ? code.replace(/(app_bg\.wasm)(["'],\s*import\.meta\.url)/g, "$1?no-inline$2") : null),
    },
    {
      // The task data's JSON Schema ships as dist-lib/schema.json (package.json exports ./schema.json), so an assistant reads the task format from node_modules.
      name: "schema-as-file",
      generateBundle() {
        this.emitFile({ type: "asset", fileName: "schema.json", source: readFileSync(at("src/adapters/plain.schema.json"), "utf8") });
      },
    },
    {
      // The fonts' licence text and notice ship beside the woff2 files they cover (the SIL OFL asks for it with every copy).
      name: "font-licence-as-files",
      generateBundle() {
        for (const name of ["OFL.txt", "NOTICE.md"]) this.emitFile({ type: "asset", fileName: name, source: readFileSync(at(`src/fonts/${name}`), "utf8") });
      },
    },
  ],
  build: {
    target: "es2022",
    outDir: "dist-lib",
    emptyOutDir: true,
    assetsDir: "",
    // Five entries, the chart, its React, Vue and Angular wrappers (gantt#1096, gantt#1875) and the Node-only headless reads (gantt#1250): the wrapper imports the chart by relative path, so they share one copy of it, and React, Vue and Angular stay the app's own.
    lib: { entry: { [PRODUCT_NAME.toLowerCase()]: at("src/index.ts"), react: at("src/react.tsx"), vue: at("src/vue.ts"), angular: at("dist-ng/angular.js"), headless: at("src/headless.ts") }, formats: ["es"], fileName: (_format, entry) => `${entry}.js`, cssFileName: "gantt" },
    rollupOptions: { external: ["react", "react/jsx-runtime", "vue", "@angular/core", "node:fs"] },
  },
});
