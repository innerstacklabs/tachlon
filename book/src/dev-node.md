# Run the engine in Node

The engine that schedules the chart also runs in Node with no page: no canvas, no `window` and no
`document`. `openHeadless` opens a schedule there and answers questions about it. Your program imports it
from `tachlon/headless`; the examples are from `packages/gantt/tests-node/headless.test.ts`, which imports
it from the package's source, `packages/gantt/src/headless.ts`.

## Open a file

`openHeadless` takes the bytes of a P6 XER or MS Project XML file and returns the schedule. It returns no
promise, and it throws an `Error` with the reader's message when the file cannot be read. From the test
`whatIfLonger moves the project finish by the known days, and whatIf slips a week`, where `FITOUT` is the
bytes of `packages/gantt/tests/fixtures/office-fitout.xml`, read with `readFileSync`:

```ts
  const chart = openHeadless(FITOUT);
  // Three more days on the critical survey push the finish from 2027-06-24 to Tuesday 2027-06-29 (a weekend between).
  const longer = chart.whatIfLonger(1, 3);
  expect(longer).toMatchObject({ ok: true, finishWas: "2027-06-24 17:00", finishNow: "2027-06-29 17:00" });
```

For a P6 file that holds several projects, pass a second argument: `"all"`, or one project's `proj_id`.

## Open your own data

`openHeadless` also takes the plain `{ tasks, links }` that `mount` takes
([Give it your data and keep your own ids](dev-data.md)), and `{ document }`, where the document is what
`chart.toDocument()` returned. From the same test file, the data and the body of the test `plain tasks and
links open with no file, and criticalPath names the tasks worked by hand`:

```ts
const PLAIN = {
  tasks: [
    { id: "short", name: "Short", start: "2027-01-04", duration: 2 },
    { id: "long", name: "Long", start: "2027-01-04", duration: 3 },
    { id: "after", name: "After", start: "2027-01-04", duration: 1 },
  ],
  links: [{ from: "short", to: "after" }, { from: "long", to: "after" }],
};
const CRITICAL = [{ task: 1, id: "long", name: "Long" }, { task: 2, id: "after", name: "After" }];
```

```ts
  const chart = openHeadless(PLAIN);
  expect(chart.criticalPath()).toMatchObject({ ok: true, critical: { count: 2, tasks: CRITICAL } });
  expect(chart.leftOut).toEqual([]);
```

`leftOut` has one line for each thing in your data that was not carried over. Plain data the reader refuses throws a
`PlainDataError`, whose `issues` list the problems.

## What it answers

- `whatIf`, `whatIfLonger`, `explain`, `health` and `criticalPath` answer as the chart's commands of the
  same names do. `npx tachlon describe commands` prints those, for the version you installed.
- `results()` lists every task's early and late dates, its total and free float and whether it is
  critical. Match an entry to your task by its `key`: its `task` is valid only until the next open.
- Every call reads the schedule and changes nothing. A schedule opened this way takes no edits.
- Call `free()` when you are done: JavaScript does not release the engine's copy of the schedule, and any
  call after `free()` throws.

Next: [Give an AI assistant the engine tools](dev-assistant.md).
