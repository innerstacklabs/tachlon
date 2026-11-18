# Listen for edits and save them

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

`npx tachlon describe events` prints what every event carries, for the version you installed. Run
`npx tachlon describe fit` before you wire saves: it prints the steps.

Next: [Make it read-only and theme it](dev-look.md).
