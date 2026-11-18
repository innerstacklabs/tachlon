# Give it your data and keep your own ids

`mount` reads plain data: an array of `tasks` and an array of `links`. A task has two names for itself,
and they do different jobs.

- `id` is a handle inside one payload. A task's `parent`, and a link's `from` and `to`, name a task by its
  `id`. Every task needs one, and no two tasks in a payload share one.
- `key` is your own identity for the task, as text: the id of its row in your database. The chart never
  reads what is in it. It hands it back to you.

These two tasks are from `loadKeyed` in `packages/gantt/tests/events.spec.ts`:

```js
  const tasks = [{ id: "a", key: "k-dig", name: "Dig", start: "2026-03-02", duration: 3 }, { id: "b", key: "k-pour", name: "Pour", start: "2026-03-02", duration: 2 }];
```

## What comes back

An event that names a task carries both `key` and `task`. In the same file, the test "a keyed task fires
select with its key" selects the second task and hears `{ task: 1, key: "k-pour" }`.

`task` is the chart's own number for the task. It is valid until the next load or open, and it is not your
`id`: the second task above has the `id` `"b"` and the number 1. Never store it and never look a task up
by it. Read `key`.

## Rules for keys

- A key is text that is not empty. Leave `key` out for a task that has none: the chart does not copy `id`
  into it, and the task's events carry `key: null`.
- No two tasks share a key, and no two links do. `mount` rejects data that breaks this, and its message
  names both places. `validate(data)` reports the same problems without mounting.
- A task that a person adds in the chart has no key. Save it, then give it the key your database made:
  `chart.commands.assignKey({ task }, key)`, with the `task` number the event carried. This is not an
  edit, so it makes no undo step.

## The other fields

`tachlon/schema.json` in the package is the JSON Schema of `tasks` and `links`: give it to a tool or an
assistant that writes task data. `npx tachlon describe options` prints what else `mount` takes, for the
version you installed.

Next: [Listen for edits and save them](dev-save.md).
