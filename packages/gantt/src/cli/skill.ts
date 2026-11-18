// The assistant skill `init --skill` writes: a short file that says when to use the package, where its instructions are, and what an assistant never does.
// `name` is the package's own name, read at run time (the product's name is written in one place only).

/** The skill's path, relative to the project folder. */
export const skillPath = (name: string): string => `.claude/skills/${name}/SKILL.md`;

/** The skill's text. */
export function skillFile(name: string): string {
  return `---
name: ${name}
description: Add or change a Gantt chart with the ${name} package. Use when a task asks for a Gantt chart in this app.
---

Read \`node_modules/${name}/AGENTS.md\` before you touch the chart, and follow it.

Before you write code against an option, event, command or token, run \`npx ${name} describe\` and ask the topic it names: the answer comes from the installed version. Run \`npx ${name} describe fit\` before you theme the chart or wire saves.

Never:

- replace or delete a page or its content
- hand-edit a lockfile
- change a Content-Security-Policy unless \`${name} doctor\` reports \`CSP_WASM_EVAL\` or \`CSP_CONNECT_SRC\`
- push, open a pull request or change a remote unless asked
- install anything globally
- leave a dev server running
`;
}
