// `describe`: the installed package answers questions about itself, so the answer matches the version in `node_modules`. It prints the model the build wrote
// into the package (`describe.json`, from the types and the stylesheet: scripts/lib/describe-model.ts) as markdown or, with `--json`, as data, and the
// "make it fit your product" procedure (`fit.md`, hand-written; src/cli/fit.md).
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Refusal } from "./detect.ts";

export interface Item { name: string; type: string; default: string | null; description: string }
export type Model = Record<"options" | "events" | "commands" | "tokens" | "errors", Item[]>;
/** What `describe` reads: the model and the text of `fit`. */
export interface Bundle { model: Model; fit: string }

/** The topics, each with the line the list gives it. */
export const TOPICS = {
  options: "what `mount` takes",
  events: "what the chart reports to `on`",
  commands: "what a page's own buttons call on `commands`",
  tokens: "the CSS custom properties that style the chart",
  errors: "the codes a failed start carries, and the fix for each",
  fit: "the steps that make the chart fit your product: its look, then its behavior",
} as const;
type Topic = keyof typeof TOPICS;

/** The two files beside the installed CLI (`dist-lib/describe.json` and `dist-lib/fit.md` from `dist-lib/cli/`). */
export function readBundle(): Bundle {
  const read = (file: string): string => {
    try {
      return readFileSync(fileURLToPath(new URL(`../${file}`, import.meta.url)), "utf8");
    } catch {
      throw new Refusal(`${file} is not in this install; reinstall the package`);
    }
  };
  return { model: JSON.parse(read("describe.json")) as Model, fit: read("fit.md") };
}

const list = (name: string, pairs: [string, string][]): string => pairs.map(([topic, line]) => `- \`${name} describe ${topic}\`: ${line}`).join("\n");

function line(i: Item): string {
  const head = i.type.startsWith("(") ? `\`${i.name}${i.type}\`` : `\`${i.name}\` (\`${i.type}\`)`;
  return `- ${head}: ${i.description}${i.default === null ? "" : ` Default: \`${i.default}\`.`}`;
}

/** The text of `describe [topic]`, markdown or JSON; an unknown topic is the error sentence with the topics listed. */
export function describeTopic(bundle: Bundle, topic: string | undefined, json: boolean, name: string): { out: string } | { error: string } {
  const topics = Object.entries(TOPICS);
  if (topic === undefined) return { out: json ? `${JSON.stringify({ topics: TOPICS }, null, 2)}\n` : `# ${name} describe\n\n${list(name, topics)}\n` };
  if (!Object.hasOwn(TOPICS, topic)) return { error: `unknown topic ${topic}; the topics are:\n${list(name, topics)}` };
  if (topic === "fit") {
    const markdown = bundle.fit.replaceAll("{name}", name);
    return { out: json ? `${JSON.stringify({ topic, markdown }, null, 2)}\n` : markdown };
  }
  const items = bundle.model[topic as Exclude<Topic, "fit">];
  return { out: json ? `${JSON.stringify({ topic, items }, null, 2)}\n` : `# ${name} describe ${topic}\n\n${TOPICS[topic as Topic]}\n\n${items.map(line).join("\n")}\n` };
}
