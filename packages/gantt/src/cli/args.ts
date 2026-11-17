// The command line's arguments: one command, and flags only (no prompts: the caller is usually an assistant).

export interface Args {
  command: string | undefined;
  dryRun: boolean;
  json: boolean;
  dir: string | undefined;
  /** `init` installs `<package>@<version>` unless this replaces that spec (a tarball, a version range). */
  spec: string | undefined;
  /** `doctor --data <file>`: plain task data to check with `validate()`. */
  data: string | undefined;
  /** `doctor --url <page>`: a served page to read the policy from, and the engine address it names. */
  url: string | undefined;
  /** False with `--no-install`. */
  install: boolean;
  /** `init --plain`: write the framework-free module, for a project the command cannot tell the bundler of. */
  plain: boolean;
  /** `init --skill`: also write the assistant skill (`skill.ts`). */
  skill: boolean;
  help: boolean;
  version: boolean;
}

/** The flags that take a value: the field each fills and what it needs, for the sentence when the value is missing. */
const VALUED: Record<string, ["dir" | "spec" | "data" | "url", string]> = { "--dir": ["dir", "a folder"], "--spec": ["spec", "a package spec"], "--data": ["data", "a data file"], "--url": ["url", "a page address"] };

/** The flags that stand alone: `--skill=false` is refused, not read as `--skill`. */
const BOOLEAN = new Set(["--dry-run", "--json", "--no-install", "--plain", "--skill", "--help", "-h", "--version", "-v"]);

/** The arguments `argv` holds, or the sentence that says why they are not valid. */
export function parseArgs(argv: string[]): Args | { error: string } {
  const args: Args = { command: undefined, dryRun: false, json: false, dir: undefined, spec: undefined, data: undefined, url: undefined, install: true, plain: false, skill: false, help: false, version: false };
  for (let i = 0; i < argv.length; i++) {
    const [flag, inline] = argv[i].split(/=(.*)/s, 2);
    if (inline !== undefined && BOOLEAN.has(flag)) return { error: `${flag} takes no value` };
    if (flag === "--dry-run") args.dryRun = true;
    else if (flag === "--json") args.json = true;
    else if (flag === "--no-install") args.install = false;
    else if (flag === "--plain") args.plain = true;
    else if (flag === "--skill") args.skill = true;
    else if (flag === "--help" || flag === "-h") args.help = true;
    else if (flag === "--version" || flag === "-v") args.version = true;
    else if (Object.hasOwn(VALUED, flag)) {
      const [key, needs] = VALUED[flag];
      const value = inline ?? argv[++i];
      if (!value || value.startsWith("-")) return { error: `${flag} needs ${needs}` };
      args[key] = value;
    } else if (flag.startsWith("-")) return { error: `unknown flag ${flag}` };
    else if (args.command === undefined) args.command = flag;
    else return { error: `unexpected argument ${flag}` };
  }
  return args;
}
