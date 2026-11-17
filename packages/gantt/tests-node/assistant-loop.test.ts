// One turn of the assistant loop (gantt#1875): the model is a stub that answers only from the messages it is handed on that call, so a loop that drops or misorders a tool result gets a wrong answer.
import { MAX_TOOL_CALLS, runAssistantTurn, type Message, type ModelReply } from "../src/assistant/loop.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const calls: unknown[][] = [];
const commands = {
  whatIf: (tasks: readonly number[]) => (calls.push(["whatIf", tasks]), { ok: true, finishWas: "a", finishNow: "b", moved: [] } as const),
  whatIfLonger: () => ({ ok: false, reason: "busy", message: "Busy." }) as const,
  explain: (task: number) => (calls.push(["explain", task]), { ok: false, reason: task === 6 ? "busy" : "unknownTask", message: "Because." } as const),
  health: () => (calls.push(["health"]), { ok: false, reason: "busy", message: "Busy." } as const),
  criticalPath: () => ({ ok: false, reason: "busy", message: "Busy." }) as const,
};
const said = (m: Message | undefined): string => (m && "content" in m ? m.content : "");
/** The results after the last model message, in the order the prompt holds them, as `callId=what it says` (a tool result's `reason`, or its project finish). */
const seen = (prompt: readonly Message[]): string =>
  prompt.slice(prompt.map((m) => m.role).lastIndexOf("assistant") + 1).map((m) => (m.role === "tool" ? `${m.callId}=${"reason" in m.result ? m.result.reason : (m.result as { finishNow: string }).finishNow}` : "")).join(" ");
const person = (text: string): Message[] => [{ role: "user", content: text }];
/** A model that asks for `ask` while the prompt ends with the person's words, and answers from the tool results it was shown once they end it. */
const model = (ask: ModelReply["toolCalls"]) => async (prompt: readonly Message[]): Promise<ModelReply> => (prompt.at(-1)?.role === "tool" ? { content: seen(prompt) } : { content: "", toolCalls: ask });

test("a_reply_with_no_tool_call_ends_the_turn", async () => {
  const turn = await runAssistantTurn(model(undefined), person("hi"), commands);
  expect(turn).toEqual({ stopped: "reply", messages: [...person("hi"), { role: "assistant", content: "" }] });
});

test("each_tool_call_runs_through_callTool_and_its_results_reach_the_next_prompt_in_order", async () => {
  calls.length = 0;
  const turn = await runAssistantTurn(model([{ id: "c1", name: "explain", arguments: { task: 6 } }, { id: "c2", name: "whatIf", arguments: { tasks: [3] } }]), person("why"), commands);
  expect(said(turn.messages.at(-1))).toBe("c1=busy c2=b");
  expect(calls).toEqual([["explain", 6], ["whatIf", [3]]]);
});

test("a_failed_call_is_a_result_the_model_is_shown", async () => {
  const turn = await runAssistantTurn(model([{ id: "c1", name: "nope", arguments: {} }, { id: "c2", name: "explain", arguments: "x" }]), person("go"), commands);
  expect(said(turn.messages.at(-1))).toBe("c1=unknownTool c2=badArguments");
});

test("a_model_that_always_asks_for_a_tool_is_stopped_at_the_cap", async () => {
  calls.length = 0;
  let sends = 0;
  const turn = await runAssistantTurn(async () => (sends++, { content: "", toolCalls: [{ id: "c", name: "health", arguments: {} }] }), person("loop"), commands);
  expect(turn.stopped).toBe("limit");
  expect([calls.length, sends, turn.messages.length]).toEqual([MAX_TOOL_CALLS, MAX_TOOL_CALLS + 1, 1 + 2 * MAX_TOOL_CALLS]);
});
