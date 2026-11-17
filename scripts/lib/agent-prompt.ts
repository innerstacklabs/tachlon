// The one prompt the package README's first screen tells a person to paste to an AI assistant (gantt#1560):
// the fenced block that follows its lead-in line.

const LEAD = "Using an AI assistant? Paste this:";

/** The prompt in `readme`, or a thrown sentence when the lead-in line or the block after it is missing. */
export function agentPrompt(readme: string): string {
  const at = readme.indexOf(LEAD);
  const block = at < 0 ? null : /^\s*```\w*\n([\s\S]*?)\n```/.exec(readme.slice(at + LEAD.length));
  if (block === null) throw new Error(`the README has no "${LEAD}" line followed by a fenced block`);
  return block[1];
}
