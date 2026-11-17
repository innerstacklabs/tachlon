// Fails when the package a stranger installs does not work: the tarball installed into an empty folder, a small page type-checked against its
// declarations, and (unless --no-browser) the chart loaded on a page served from that folder until it is ready. Exit 1 on a problem, 2 on no pack.
//   node scripts/consumer-check.ts <package folder> [--no-browser]
import { consumerProblems } from "./lib/consumer-check.ts";

const [dir, flag] = process.argv.slice(2);
if (!dir || (flag !== undefined && flag !== "--no-browser")) {
  console.error("usage: node scripts/consumer-check.ts <package folder> [--no-browser]");
  process.exit(2);
}
try {
  const problems = await consumerProblems(dir, { browser: flag === undefined });
  for (const p of problems) console.error(`consumer-check: FAIL ${dir}: ${p}`);
  if (problems.length) {
    console.error(`consumer-check: FAIL ${dir}: ${problems.length} problem(s)`);
    process.exit(1);
  }
  console.log(`consumer-check: PASS ${dir}`);
} catch (e) {
  console.error(`consumer-check: ${(e as Error).message}`);
  process.exit(2);
}
