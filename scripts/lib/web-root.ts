// The chart's folder under the repo root, named once for the scripts. `.github/workflows/ci.yml` names it once more, in its
// `env` (WEB_ROOT). `scripts/build-web.sh` reads it with `node scripts/lib/web-root.ts`. Needs Node 22.18 or later.
import { fileURLToPath } from "node:url";

export const WEB_ROOT = "packages/gantt";

if (process.argv[1] === fileURLToPath(import.meta.url)) console.log(WEB_ROOT);
