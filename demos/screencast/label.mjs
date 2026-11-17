// The source label of a recording in the README block: which branch it was recorded on and the
// commit it is based on. A recording on `main` keeps the label it always had.

// run.sh refuses any other branch name, so a name never breaks the backticks in the README.
export const BRANCH_PATTERN = /^[A-Za-z0-9._\/-]+$/;

// What `run.sh --local` passes as the branch when HEAD is detached (a `git worktree add --detach` tree).
// The parentheses are not allowed in a branch name, so no real branch can be mistaken for it.
export const DETACHED_BRANCH = "(detached)";

/**
 * @param {{ branch?: string, commit: string }} metadata screencast.json fields. A file written
 *   before `branch` existed has none; it was always recorded on main.
 * @returns {string} for example "from `main` at `f586337`", "from branch `feat/x` based on `f586337`"
 *   or "from detached HEAD based on `f586337`"
 */
export function sourceLabel({ branch = "main", commit }) {
  if (branch === DETACHED_BRANCH) return `from detached HEAD based on \`${commit}\``;
  if (!BRANCH_PATTERN.test(branch)) {
    throw new Error(`docs/media/screencast.json has a branch name that is not allowed: ${JSON.stringify(branch)}`);
  }
  if (branch === "main") return `from \`main\` at \`${commit}\``;
  return `from branch \`${branch}\` based on \`${commit}\``;
}
