// Fails when a package's packed tarball is larger than the `bytes` in its size-budget.json. Exit 1 over budget, 2 on no budget file or no pack.
//   node scripts/package-size.ts <package folder>
import { budgetBytes, tarballBytes } from "./lib/package-size.ts";

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node scripts/package-size.ts <package folder>");
  process.exit(2);
}
try {
  const size = tarballBytes(dir);
  const budget = budgetBytes(dir);
  console.log(`package-size: ${dir} tarball ${size} B, budget ${budget} B`);
  if (size > budget) {
    console.error(`package-size: FAIL ${dir}: ${size - budget} B over budget. Shrink the tarball; the budget only shrinks.`);
    process.exit(1);
  }
} catch (e) {
  console.error(`package-size: ${(e as Error).message}`);
  process.exit(2);
}
