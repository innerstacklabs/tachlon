# Listen for edits and save them

## Give `save` one handler for each change

When your app already has an API client, pass `save` an object with one handler for each kind of change. The
chart calls the right one for each edit, in the order the person made them, and waits for it to resolve
before it calls the next. You write no queue, no retry and no put-back. This is `examples/save/src/main.tsx`
after its imports; `fields` names your API's members:

```tsx
const fields = { // the chart's names on the left, your API's on the right; any other member of a row comes back untouched
  tasks: { key: "id", name: "title", start: "plannedStart", duration: "days", parent: "parentId" },
  links: { key: "id", from: "source", to: "target" },
};

const save: Handlers = { // one call into your API per change, in the order the person made them
  createTask: (row, c) => api.createTask(row, c),
  updateTask: (key, set, c) => api.updateTask(key, set, c),
  deleteTask: (key, c) => api.deleteTask(key, c),
  createLink: (row, c) => api.createLink(row, c),
  updateLink: (key, set, c) => api.updateLink(key, set, c),
  deleteLink: (key, c) => api.deleteLink(key, c),
  updateSchedule: (set, c) => api.updateSchedule(set, c),
};

export function Schedule({ tasks, links, reload }: { tasks: Task[]; links: Link[]; reload: () => void }) {
  return <Gantt tasks={tasks} links={links} fields={fields} dateForm="day"
    save={save} onError={(e) => e.source === "save" && reload()} style={{ height: 480 }} />;
}
```

- The last argument, `c`, carries `once` (the idempotency key to send with the request) and `signal` (aborted after 10 seconds).
- Throw a `SaveRefused` when the server says no. The chart puts that edit and the ones after it back and fires `error` with the source `"save"`, so read your data again there, as `onError` does. Any other throw stops the queue.
- A handler you leave out turns its edits off. Every task and link you pass needs a `key`.
- [The package README](https://github.com/innerstacklabs/tachlon/blob/main/packages/gantt/README.md#connect-to-your-backend) has the rest, and a `save` function for a host that writes a whole commit at once.

## Or write your own save loop on `edit`

`chart.on("edit", handler)` calls your handler after an edit, an undo or a redo changed the schedule. It
fires once per change, never once per frame, and it returns the function that removes the handler.

## Save the whole schedule

The simplest save writes the whole schedule after each edit. `chart.toDocument()` returns the schedule as
a document, and `chart.load({ document })` reads it back. The hosted app saves this way. These lines are
from `installAutosave` in `apps/hosted/src/autosave.ts`, which `apps/hosted/tests/autosave.spec.ts` runs:

```ts
  gantt.on("edit", () => {
    clearTimeout(timer);
    timer = setTimeout(() => void (!held && gantt.state().unsaved && write(gantt.toDocument())), QUIET_MS);
  });
```

`gantt` is the chart. `write` stores the document: the hosted app keeps it in the browser, and your app
would send it to your server. The timer waits `QUIET_MS` after the last edit, so a run of typing is one
write.

## Save each change

To write only what changed, read the event. Its `commits` are the undo steps made since the last `edit`
event, oldest first. Each commit has a `seq` that counts from 1 and is never reused, a `source` that says
where the step came from, and its `changes`. A change names the task or link by your `key`
([Give it your data and keep your own ids](dev-data.md)) and gives its `set` (the members that changed, with their new values) and its `was` (the same members as they were).

- An undo or a redo is a commit too, with `set` and `was` swapped, so the same code saves it.
- A bar drag is one commit, and it arrives when the bar is released.
- When the event's `snapshot` is true, commits were lost. Save `chart.snapshot()`, which returns the
  schedule as plain `tasks` and `links`, then read on from the event's `seq`.
- A task added in the chart has `key: null` in its change. Save it, then give it its key with `assignKey`.
- Mounted with `dateForm: "day"`, every date in a change is `YYYY-MM-DD`, and `update`, `hostEdit` and `setData`
  take dates in that form too, so you can write back what you stored. They refuse a date with a time of day
  as `badValue`, and change nothing.

`npx tachlon describe events` prints what every event carries, for the version you installed. Run
`npx tachlon describe fit` before you wire saves: it prints the steps.

Next: [Make it read-only and theme it](dev-look.md).
