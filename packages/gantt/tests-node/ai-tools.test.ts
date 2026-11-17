// The tool definitions of an AI assistant (gantt#1250): `toolDefinitions` gives the same tools in three forms and `callTool` runs one by name. No page: the commands are a recording stand-in.
import { callTool, toolDefinitions, type ToolDefinition } from "../src/ai-tools.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const calls: unknown[][] = [];
const answer = { ok: true, finishWas: "2026-03-02 17:00", finishNow: "2026-03-09 17:00", moved: [] } as const;
const refusal = { ok: false, reason: "busy", message: "Busy." } as const; // a stand-in answer that every one of the six results can be
const commands = {
  propose: (change: unknown) => (calls.push(["propose", change]), answer),
  whatIf: (tasks: readonly number[]) => (calls.push(["whatIf", tasks]), answer),
  whatIfLonger: (task: number, days: number) => (calls.push(["whatIfLonger", task, days]), answer),
  explain: (task: number) => (calls.push(["explain", task]), refusal),
  health: () => (calls.push(["health"]), refusal),
  criticalPath: () => (calls.push(["criticalPath"]), refusal),
};
/** One tool as every form says it: its name, its description and its argument schema. */
const plain = (d: ToolDefinition): unknown => {
  if ("function" in d) return { name: d.function.name, description: d.function.description, schema: d.function.parameters };
  return { name: d.name, description: d.description, schema: "inputSchema" in d ? d.inputSchema : d.input_schema };
};

test("the_three_forms_hold_the_same_tools_named_for_the_commands_and_each_shape_is_its_own", () => {
  const [mcp, anthropic, openai] = (["mcp", "anthropic", "openai"] as const).map(toolDefinitions);
  expect(mcp!.map((t) => (t as { name: string }).name)).toEqual(["whatIf", "whatIfLonger", "explain", "health", "criticalPath", "propose"]);
  expect(anthropic!.map(plain)).toEqual(mcp!.map(plain));
  expect(openai!.map(plain)).toEqual(mcp!.map(plain));
  expect(openai![0]).toMatchObject({ type: "function", function: { parameters: { required: ["tasks"] } } });
  expect(anthropic![1]).toMatchObject({ input_schema: { required: ["task", "days"] } });
  expect(mcp![1]).toMatchObject({ inputSchema: { properties: { days: { minimum: 0 } } } });
  expect(anthropic![2]).toMatchObject({ input_schema: { required: ["task"] } });
  expect(openai![3]).toMatchObject({ function: { parameters: { properties: {}, additionalProperties: false } } });
  expect(openai![5]).toMatchObject({ function: { parameters: { required: ["kind"], properties: { kind: { enum: ["move", "duration", "progress", "link", "unlink"] } } } } });
});

test("a_proposal_of_each_kind_is_held_by_the_command_and_a_person_accepts_it_not_a_tool", () => {
  calls.length = 0;
  const asks = [{ kind: "move", task: 3, start: "2026-03-09" }, { kind: "duration", task: 4, days: 5 }, { kind: "link", pred: 1, succ: 2, text: "SS+2d" }, { kind: "unlink", pred: 1, succ: 2 }];
  for (const ask of asks) expect(callTool(commands, "propose", ask)).toBe(answer);
  expect(calls).toEqual(asks.map((ask) => ["propose", ask]));
  expect(toolDefinitions("mcp").map((t) => (t as { name: string }).name).filter((n) => /accept|reject/i.test(n))).toEqual([]);
});

test("commands_with_no_propose_answer_unknownTool_for_it_and_run_the_other_five", () => {
  const { propose: _held, ...readOnly } = commands;
  expect(callTool(readOnly, "propose", { kind: "move", task: 3, start: "2026-03-09" })).toMatchObject({ ok: false, reason: "unknownTool" });
  expect(callTool(readOnly, "whatIf", { tasks: [3] })).toBe(answer);
});

test("a_schema_a_host_edits_changes_no_other_call", () => {
  (toolDefinitions("mcp")[0] as { inputSchema: { required: string[] } }).inputSchema.required.length = 0;
  expect(toolDefinitions("mcp")[0]).toMatchObject({ inputSchema: { required: ["tasks"] } });
});

test("a_call_runs_the_command_of_that_name_and_answers_its_result", () => {
  calls.length = 0;
  expect(callTool(commands, "whatIf", { tasks: [3, 5] })).toBe(answer);
  expect(callTool(commands, "whatIfLonger", { task: 4, days: 2 })).toBe(answer);
  expect(callTool(commands, "explain", { task: 6 })).toBe(refusal);
  expect(callTool(commands, "health", undefined)).toBe(refusal);
  expect(callTool(commands, "criticalPath", { ignored: 1 })).toBe(refusal);
  expect(calls).toEqual([["whatIf", [3, 5]], ["whatIfLonger", 4, 2], ["explain", 6], ["health"], ["criticalPath"]]);
});

test("what_cannot_run_is_a_refusal_and_never_a_throw", () => {
  calls.length = 0;
  const reason = (name: string, args: unknown, c = commands): unknown => (callTool(c, name, args) as { reason?: string }).reason;
  for (const bad of [undefined, null, "3", [3], {}, { tasks: "3" }, { tasks: [3, "5"] }]) expect(reason("whatIf", bad)).toBe("badArguments");
  for (const bad of [{ task: 4 }, { task: "4", days: 2 }, { task: 4, days: null }]) expect(reason("whatIfLonger", bad)).toBe("badArguments");
  for (const bad of [{}, { task: "6" }, { task: null }]) expect(reason("explain", bad)).toBe("badArguments");
  for (const bad of [undefined, {}, { task: 3 }, { kind: 3 }]) expect(reason("propose", bad)).toBe("badArguments");
  expect(reason("propose", { kind: "move", task: 3, start: "x" }, { ...commands, propose: () => undefined as never })).toBe("destroyed");
  for (const name of ["nope", "toString", "constructor", "", undefined as unknown as string]) expect(reason(name, { tasks: [1] })).toBe("unknownTool");
  expect(reason("whatIf", { tasks: [1] }, { ...commands, whatIf: () => undefined as never })).toBe("destroyed");
  for (const name of ["explain", "health", "criticalPath"]) expect(reason(name, { task: 1 }, { ...commands, [name]: () => undefined })).toBe("destroyed");
  expect(calls).toEqual([]);
});
