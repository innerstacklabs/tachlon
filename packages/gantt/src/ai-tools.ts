// Tool definitions for an AI assistant (gantt#1250): the six tools (what moves, why a start, health, the critical path, and a held edit), described once and handed out in the three forms an assistant takes
// (MCP, Anthropic, OpenAI), and `callTool` runs one by its name over `Commands`. Pure: no page, no wasm. A bad call is a result, never a throw.
import type { Commands } from "./commands.js";
import type { CriticalPathResult } from "./critical-path.js";
import type { ExplainResult } from "./explain.js";
import type { HealthResult } from "./health.js";
import type { ProposedChange, ProposeResult } from "./propose.js";
import type { WhatIfResult } from "./what-if.js";

/** One tool in the shape its form takes: MCP (`inputSchema`), Anthropic (`input_schema`) or OpenAI (`type: "function"`, the rest under `function`, the schema as `parameters`). */
export type ToolDefinition =
  | { name: string; description: string; inputSchema: object }
  | { name: string; description: string; input_schema: object }
  | { type: "function"; function: { name: string; description: string; parameters: object } };

/** What `callTool` answers: the tool's own result, or why no tool ran. */
export type ToolResult = WhatIfResult | ExplainResult | HealthResult | CriticalPathResult | ProposeResult | { ok: false; reason: "unknownTool" | "badArguments" | "destroyed"; message: string };

const num = (n: unknown): n is number => typeof n === "number";
const taskNumber = { type: "integer", minimum: 0, description: "A task's number: the `task` field of a result, not its activity ID." };

const noArguments = { type: "object", properties: {}, required: [], additionalProperties: false };

/** Each tool: what an assistant reads, the schema of its arguments, and how it runs (null for arguments that do not fit, undefined once the chart is destroyed). */
const TOOLS = {
  whatIf: {
    description: "Ask what moves if each of up to 10 tasks slips a week (7 working days). Changes nothing. Answers the project finish before and after and every task that would move, or why not.",
    schema: { type: "object", properties: { tasks: { type: "array", items: taskNumber, maxItems: 10, description: "The tasks that slip." } }, required: ["tasks"], additionalProperties: false },
    run: (c: Pick<Commands, "whatIf">, a: Record<string, unknown>) => (Array.isArray(a.tasks) && a.tasks.every(num) ? c.whatIf(a.tasks) : null),
  },
  whatIfLonger: {
    description: "Ask what moves if one task takes some working days longer. Changes nothing. Answers the project finish before and after and every task that would move, or why not.",
    schema: { type: "object", properties: { task: taskNumber, days: { type: "integer", minimum: 0, description: "The working days to add." } }, required: ["task", "days"], additionalProperties: false },
    run: (c: Pick<Commands, "whatIfLonger">, a: Record<string, unknown>) => (num(a.task) && num(a.days) ? c.whatIfLonger(a.task, a.days) : null),
  },
  explain: {
    description: "Ask why one task starts when it does. Changes nothing. Answers its early start and the chain of tasks whose links set it, nearest first, each with its float, or why not.",
    schema: { type: "object", properties: { task: taskNumber }, required: ["task"], additionalProperties: false },
    run: (c: Pick<Commands, "explain">, a: Record<string, unknown>) => (num(a.task) ? c.explain(a.task) : null),
  },
  health: {
    description: "Ask how healthy the schedule is. Changes nothing. Answers, for each schedule check that walks tasks, its total and the first 50 tasks it holds, or why not.",
    schema: noArguments,
    run: (c: Pick<Commands, "health">) => c.health(),
  },
  criticalPath: {
    description: "Ask which tasks set the finish date. Changes nothing. Answers the unfinished tasks with no float and the tasks of the longest path, each list with its total and first 50 tasks, or why not.",
    schema: noArguments,
    run: (c: Pick<Commands, "criticalPath">) => c.criticalPath(),
  },
  propose: {
    description:
      'Propose one edit and hold it for a person to accept or reject: the schedule stays as it is and the chart draws a ghost where each task would go. kind "move" takes task and start (YYYY-MM-DD or YYYY-MM-DD HH:MM); "duration" takes task and days; "progress" takes task and one of actualStart, actualFinish, remaining or progress; "link" takes pred, succ and text (FS, SS+2d); "unlink" takes pred and succ. Answers the project finish before and after and every task that would move, or why not.',
    schema: {
      type: "object",
      properties: {
        kind: { enum: ["move", "duration", "progress", "link", "unlink"], description: "The edit to hold." },
        task: taskNumber,
        start: { type: "string", description: "A move's earliest start." },
        days: { type: "integer", minimum: 0, description: "A duration's working days; 0 makes a milestone." },
        actualStart: { type: "string", description: "A progress report's actual start (YYYY-MM-DD or YYYY-MM-DD HH:MM)." },
        actualFinish: { type: "string", description: "A progress report's actual finish, written as actualStart is." },
        remaining: { type: "number", minimum: 0, description: "A progress report's remaining working days." },
        progress: { type: "number", minimum: 0, maximum: 100, description: "A progress report's percent complete, a whole number." },
        pred: taskNumber,
        succ: taskNumber,
        text: { type: "string", description: "A link's type and lag, such as FS or SS+2d." },
      },
      required: ["kind"],
      additionalProperties: false,
    },
    run: (c: Partial<Pick<Commands, "propose">>, a: Record<string, unknown>) => (typeof a.kind === "string" ? c.propose?.(a as ProposedChange) : null),
  },
};

/** The tools in the form an assistant's API takes: each call gives a copy, so a host that edits one schema changes no other. */
export function toolDefinitions(form: "mcp" | "anthropic" | "openai"): readonly ToolDefinition[] {
  return Object.entries(TOOLS).map(([name, { description, schema: s }]) => {
    const schema = structuredClone(s);
    if (form === "mcp") return { name, description, inputSchema: schema };
    if (form === "anthropic") return { name, description, input_schema: schema };
    return { type: "function", function: { name, description, parameters: schema } };
  });
}

/** Run the tool called `name` with `args` (the object an assistant sends) over `commands`, which is `chart.commands` or a headless schedule. Commands with no `propose` answer `"unknownTool"` for it. */
export function callTool(commands: Pick<Commands, "whatIf" | "whatIfLonger" | "explain" | "health" | "criticalPath"> & Partial<Pick<Commands, "propose">>, name: string, args: unknown): ToolResult {
  const refused = (reason: "unknownTool" | "badArguments" | "destroyed", message: string): ToolResult => Object.freeze({ ok: false, reason, message });
  if (!Object.hasOwn(TOOLS, name)) return refused("unknownTool", `No tool is named ${JSON.stringify(name)}; the tools are ${Object.keys(TOOLS).join(", ")}.`);
  if (typeof (commands as Record<string, unknown>)[name] !== "function") return refused("unknownTool", `These commands have no ${name}.`);
  const tool = TOOLS[name as keyof typeof TOOLS];
  const given = typeof args === "object" && args !== null && !Array.isArray(args) ? (args as Record<string, unknown>) : {};
  const result = tool.run(commands, given);
  if (result === null) return refused("badArguments", name === "propose" ? "propose takes a text \"kind\"." : `${name} takes ${JSON.stringify(tool.schema.required)}, as numbers.`);
  return result ?? refused("destroyed", "The chart has been destroyed.");
}
