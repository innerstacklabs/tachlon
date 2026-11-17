// One turn of an AI assistant (gantt#1875): the host's `send` asks its model, each tool call the model makes runs through `callTool` over the chart's commands, and the results go back to the model until it answers
// without one. Pure: no page, no wasm, and no model, key or address here; `send` is the host's own call to its model.
import { callTool, type ToolResult } from "../ai-tools.ts"; // `.ts`, so the Node tier can load this file

/** One tool call a model asks for: `id` ties the call to its result, `arguments` is the object the model sent. */
export interface ToolCall {
  id: string;
  name: string;
  arguments: unknown;
}

/** What a model answers: its words and the tool calls it wants run before it goes on. */
export interface ModelReply {
  content: string;
  toolCalls?: readonly ToolCall[];
}

/** One message of the conversation: what a person said, what the model answered, or the result of one of its calls. */
export type Message =
  | { role: "user"; content: string }
  | ({ role: "assistant" } & ModelReply)
  | { role: "tool"; callId: string; name: string; result: ToolResult };

/** The host's call to its model: the whole conversation so far in, the model's next reply out. */
export type Send = (messages: readonly Message[]) => Promise<ModelReply>;

/** The most tool calls one turn runs. */
export const MAX_TOOL_CALLS = 8;

/** How a turn ended: `reply` when the model answered with no tool call, `limit` when it asked for more than `MAX_TOOL_CALLS` calls (the extra calls did not run). */
export interface TurnResult {
  messages: readonly Message[];
  stopped: "reply" | "limit";
}

/** Run one turn over `history` (ending with what the person said): `messages` is `history` plus every reply and tool result of the turn. A call that fails is a result like any other, as `callTool` makes it; a `send` that rejects rejects the turn. */
export async function runAssistantTurn(send: Send, history: readonly Message[], commands: Parameters<typeof callTool>[0]): Promise<TurnResult> {
  const messages = [...history];
  let ran = 0;
  for (;;) {
    const reply = await send([...messages]);
    const calls = reply.toolCalls ?? [];
    if (calls.length === 0) return { messages: [...messages, { role: "assistant", ...reply }], stopped: "reply" };
    if (ran + calls.length > MAX_TOOL_CALLS) return { messages, stopped: "limit" };
    messages.push({ role: "assistant", ...reply });
    for (const call of calls) messages.push({ role: "tool", callId: call.id, name: call.name, result: callTool(commands, call.name, call.arguments) });
    ran += calls.length;
  }
}
