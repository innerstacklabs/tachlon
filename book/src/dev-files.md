# Load and save P6 and MS Project files

The chart reads Primavera P6 XER files and MS Project XML files, and writes both. What each reader maps
and what it leaves out is in
[Importing a schedule](https://github.com/innerstacklabs/tachlon/blob/main/docs/IMPORT.md); this book does
not repeat it.

## Load a file

`chart.load({ file })` takes a `File`, such as the one a file input gives your page, and replaces the
schedule with it. It resolves `{ ok: true }`, or `{ ok: false, error }` with the schedule as it was, and
it never throws. These lines are from the test `load_a_file_replaces_the_schedule` in
`packages/gantt/tests/data-api.spec.ts`. The test runs them in the page, where `window.gantt.api` is the
chart that `mount` returned:

```ts
  const result = await g.page.evaluate(async (xer) => {
    const file = new File([new Uint8Array(xer.bytes)], xer.name);
    return window.gantt!.api!.load({ file });
  }, { name: XER.name, bytes: [...XER.buffer] });
  expect(result).toEqual({ ok: true });
```

- `load` does not ask before it replaces unsaved edits. `chart.open(file)` asks first.
- `load` reads every project of a P6 file that holds several. `chart.open(file)` stops at the list of
  projects, which the `error` event carries as `projects`, and `chart.open(file, { project })` opens the
  one you name, or `"all"`.
- After a load, `chart.state().importNotes` holds the import notes: the lines the reader reported.
- To start a chart on a file, give `mount` the file as its `schedule` option.

`npx tachlon describe options` and `npx tachlon describe events` print what `mount` takes and what the
`error` event carries, for the version you installed.

## Save a file

`chart.exportXer()` returns the open schedule as a P6 XER file, and `chart.exportMspdi()` returns it as an
MS Project XML file. Each returns `bytes`, a `Uint8Array`, and `leftOut`, one line for each thing the
file cannot carry. Neither call downloads anything: the bytes are yours to offer as a download or to send
to your server. Each throws when the schedule holds a value the file's form cannot write. These lines are
from the test `export_xer_reads_back_and_a_second_round_trip_changes_nothing` in the same file, where
`api` is the chart:

```ts
      const { bytes } = api.exportXer();
      results.push(await api.load({ file: new File([bytes], "round.xer") }));
```

The test writes the schedule and loads the written file, twice, and both loads answer `{ ok: true }`.

`docs/IMPORT.md` says what each writer puts in the file and what `leftOut` names:
[Writing a P6 XER file](https://github.com/innerstacklabs/tachlon/blob/main/docs/IMPORT.md#writing-a-p6-xer-file)
and
[Writing an MS Project XML file](https://github.com/innerstacklabs/tachlon/blob/main/docs/IMPORT.md#writing-an-ms-project-xml-file).

A P6 or MS Project file cannot carry everything the chart holds, which is what `leftOut` reports. The
save that loses nothing is the document `chart.toDocument()` returns:
[Listen for edits and save them](dev-save.md).

Next: [Run the engine in Node](dev-node.md).
