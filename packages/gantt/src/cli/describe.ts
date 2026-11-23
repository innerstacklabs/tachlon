// `describe`: the installed package answers questions about itself, so the answer matches the version in `node_modules`. It prints the model the build wrote
// into the package (`describe.json`, from the types and the stylesheet: scripts/lib/describe-model.ts) as markdown or, with `--json`, as data, and the
// "make it fit your product" procedure (`fit.md`, hand-written; src/cli/fit.md) and the prompt that turns the user guide into the product's own (`guide/PROMPT.md`).
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Refusal } from "./detect.ts";

export interface Item { name: string; type: string; default: string | null; description: string }
export type Model = Record<"options" | "events" | "commands" | "methods" | "tokens" | "errors" | "refusals", Item[]>;
/** What `describe` reads: the model, the text of `fit` and the prompt of `guide`. */
export interface Bundle { model: Model; fit: string; guide: string }

/** The topics, each with the line the list gives it. */
export const TOPICS = {
  options: "what `mount` takes",
  events: "what the chart reports to `on`",
  commands: "what a page's own buttons call on `commands`",
  methods: "what the chart object answers to, each method with its whole doc comment",
  tokens: "the CSS custom properties that style the chart",
  errors: "the codes a failed start carries, and the fix for each",
  refusals: "why a call changed nothing: each `reason`, and the fix where there is one",
  fit: "the steps that make the chart fit your product: its look, then its behavior",
  guide: "the prompt that turns the package's user guide into your product's own documentation",
} as const;
type Topic = keyof typeof TOPICS;

/** The files beside the installed CLI, in `base` (`dist-lib/`, the folder above `dist-lib/cli/`): `describe.json`, `fit.md` and `guide/PROMPT.md`. */
export function readBundle(base = new URL("../", import.meta.url)): Bundle {
  const read = (file: string): string => {
    try {
      return readFileSync(fileURLToPath(new URL(file, base)), "utf8");
    } catch {
      throw new Refusal(`${file} is not in this install; reinstall the package`);
    }
  };
  return { model: JSON.parse(read("describe.json")) as Model, fit: read("fit.md"), guide: read("guide/PROMPT.md") };
}

const list = (name: string, pairs: [string, string][]): string => pairs.map(([topic, line]) => `- \`${name} describe ${topic}\`: ${line}`).join("\n");

function line(i: Item): string {
  const head = i.type.startsWith("(") ? `\`${i.name}${i.type}\`` : `\`${i.name}\` (\`${i.type}\`)`;
  return `- ${head}: ${i.description}${i.default === null ? "" : ` Default: \`${i.default}\`.`}`;
}

const reasons = (i: Item): string => `- ${i.name.split(", ").map((r) => `\`${r}\``).join(", ")}: ${i.description}`;

/** The text of `describe [topic]`, markdown or JSON; an unknown topic is the error sentence with the topics listed. */
export function describeTopic(bundle: Bundle, topic: string | undefined, json: boolean, name: string): { out: string } | { error: string } {
  const topics = Object.entries(TOPICS);
  if (topic === undefined) return { out: json ? `${JSON.stringify({ topics: TOPICS }, null, 2)}\n` : `# ${name} describe\n\n${list(name, topics)}\n` };
  if (!Object.hasOwn(TOPICS, topic)) return { error: `unknown topic ${topic}; the topics are:\n${list(name, topics)}` };
  if (topic === "fit" || topic === "guide") {
    const markdown = bundle[topic].replaceAll("{name}", name);
    return { out: json ? `${JSON.stringify({ topic, markdown }, null, 2)}\n` : markdown };
  }
  const items = bundle.model[topic as Exclude<Topic, "fit" | "guide">];
  const more = topic === "methods" || topic === "commands" ? `\nWhy a call changes nothing: \`${name} describe refusals\`.\n` : "";
  return { out: json ? `${JSON.stringify({ topic, items }, null, 2)}\n` : `# ${name} describe ${topic}\n\n${TOPICS[topic as Topic]}\n\n${items.map(topic === "refusals" ? reasons : line).join("\n")}\n${more}` };
}
