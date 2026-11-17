// Prints the CI tier for a workflow run as a `tier=<tier>` line, ready to append to $GITHUB_OUTPUT:
//   node scripts/ci-tier.ts "$EVENT" "$REF" >> "$GITHUB_OUTPUT"
// `EVENT` is `github.event_name` and `REF` is `github.ref`. The rule is in scripts/lib/ci-tier.ts.
// Needs Node 22.18 or later (type stripping). Exits 2 and prints nothing to stdout when either is missing.
import { tierFor } from "./lib/ci-tier.ts";

const [event, ref] = process.argv.slice(2);
if (!event || !ref) {
  console.error("ci-tier: usage: node scripts/ci-tier.ts <event> <ref>");
  process.exit(2);
}
const tier = tierFor(event, ref);
console.error(`ci-tier: ${event} on ${ref} is the ${tier} tier`);
console.log(`tier=${tier}`);
