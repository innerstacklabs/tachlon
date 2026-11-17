// The hosted build the screencast records and the address it opens (gs#541, gs#601). run.sh builds it and copies it to the
// Mac, record.mjs opens the address, and clip.mjs records on it, so the README video and a book clip show the same app.
// test-hosted-build.mjs fails if one of them names another build or address. run.sh reads the build directory with
//   node demos/screencast/hosted-build.mjs dist
// and the address's query with `query`.
import { fileURLToPath } from "node:url";

/** The hosted app's build directory, from the repo root (`pnpm build` in apps/hosted writes it). */
export const HOSTED_DIST = "apps/hosted/dist";
/** The address's query: the built-in 50,000-task sample the scenes name their tasks in. */
export const HOSTED_QUERY = "?sample=demo-50k";
/** The file the hosted build has and the plain web build does not: the sample list `build-samples.mjs` writes. */
export const HOSTED_MARKER = "samples/samples.json";

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const values = { dist: HOSTED_DIST, query: HOSTED_QUERY };
  const value = values[process.argv[2]];
  if (process.argv.length !== 3 || value === undefined) {
    console.error("usage: node hosted-build.mjs dist|query");
    process.exitCode = 2;
  } else {
    console.log(value);
  }
}
