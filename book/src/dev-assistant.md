# Give an AI assistant the engine tools

An assistant that calls tools can ask the engine about a schedule: what moves if a task slips, why a task
starts when it does, which tasks set the finish date. The package describes those questions as tool
definitions and runs the call the assistant makes. Your code talks to the model: `toolDefinitions` and
`callTool`, both exported by `tachlon`, need no page and make no request.

## The tool definitions

`toolDefinitions(form)` returns the tools in the form your model's API takes: `"mcp"`, `"anthropic"` or
`"openai"`. The three forms hold the same tools. These lines name them, from the test
`the_three_forms_hold_the_same_tools_named_for_the_commands_and_each_shape_is_its_own` in
`packages/gantt/tests-node/ai-tools.test.ts`:

```ts
  const [mcp, anthropic, openai] = (["mcp", "anthropic", "openai"] as const).map(toolDefinitions);
  expect(mcp!.map((t) => (t as { name: string }).name)).toEqual(["whatIf", "whatIfLonger", "explain", "health", "criticalPath", "propose"]);
```

The first five ask a question and change nothing. `propose` holds one edit for a person to accept or
reject: the schedule stays as it is, and the chart draws a ghost where each task would go. No tool accepts
a proposal. Each call of `toolDefinitions` returns a new copy, so a schema you edit changes no other call.

## Run a tool call

`callTool(chart.commands, name, args)` runs the tool the assistant picked and returns the result to send
back to it, the same result as the command of that name gives. `args` is the object the assistant sent.
From the test `a_call_runs_the_command_of_that_name_and_answers_its_result` in the same file, where
`commands` stands in for `chart.commands`:

```ts
  expect(callTool(commands, "whatIf", { tasks: [3, 5] })).toBe(answer);
  expect(callTool(commands, "whatIfLonger", { task: 4, days: 2 })).toBe(answer);
```

A call that cannot run is a result and never a throw: `{ ok: false, reason, message }`, where `reason` is
`"unknownTool"`, `"badArguments"` or `"destroyed"`. A tool that takes arguments answers `"badArguments"`
when they are text and not an object, so parse JSON text first. `npx tachlon describe commands` prints
what each command takes and answers, for the version you installed.

## In Node, with no chart

`tachlon/headless` exports the same two functions, and its `callTool` takes the schedule that
`openHeadless` returned ([Run the engine in Node](dev-node.md)). That schedule takes no edits, so `propose`
answers `"unknownTool"`. From the test `the assistant's tools run over a file, propose is refused, and
free releases the schedule` in `packages/gantt/tests-node/headless.test.ts`:

```ts
  const chart = openHeadless(FITOUT);
  expect(toolDefinitions("mcp").length).toBe(6);
  expect(callTool(chart, "whatIfLonger", { task: 1, days: 3 })).toEqual(chart.whatIfLonger(1, 3));
  expect(callTool(chart, "propose", { kind: "move", task: 3, start: "2027-02-01" })).toMatchObject({ ok: false, reason: "unknownTool" });
```

## A chat box beside the chart

The chart can draw the chat panel and run the tool calls itself. You give `mount` the `assistant` option
with `send`, your own call to your model, and the page sends the conversation nowhere else.
`npx tachlon describe options` prints the option, and the package README shows a `send`:
[Tools for an AI assistant](https://github.com/innerstacklabs/tachlon/blob/main/packages/gantt/README.md#tools-for-an-ai-assistant).
