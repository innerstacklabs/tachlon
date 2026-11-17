// Builds our side's bench page (`ours-page.html`) with the hosted app's own settings: relative asset URLs and the engine preload,
// and the package the hosted app installs (`gantt-core`, built by `pnpm build:lib`). Run in `apps/hosted`, which has vite:
//
//   pnpm exec vite build --config ../../demos/bench/sides/ours-vite.config.ts --outDir <dir>
//
// `<dir>` is the bundle `sides.json` names for ours. The page is `<dir>/ours-page.html`.

import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import hosted from "../../../apps/hosted/vite.config.ts";

const here = dirname(fileURLToPath(import.meta.url));

// The page sits outside apps/hosted, so it has no install of its own: the package is looked up as if the hosted app's page asked.
const asked = fileURLToPath(new URL("../../../apps/hosted/src/main.ts", import.meta.url));
const ganttCore = {
  name: "gantt-core-from-hosted",
  resolveId(this: { resolve: (id: string, from: string, options: object) => unknown }, id: string) {
    return id.split("/")[0] === "gantt-core" ? this.resolve(id, asked, { skipSelf: true }) : null;
  },
};

export default {
  ...hosted,
  plugins: [...hosted.plugins, ganttCore],
  root: here,
  build: { ...hosted.build, rollupOptions: { input: `${here}/ours-page.html` }, emptyOutDir: true },
};
