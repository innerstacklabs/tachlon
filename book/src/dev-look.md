# Make it read-only and theme it

## Read-only

Start the chart read-only with the `readOnly` option of `mount`, or switch it while the chart runs:
`chart.setReadOnly(true)` and `chart.setReadOnly(false)`. `chart.readOnly()` says which it is. In the
React, Vue and Angular components, `readOnly` is a prop or an input.

While it is on, no edit path changes the schedule: the keys, the cell editor, the task dialog, a bar drag,
paste and undo are all refused, and the chart says why. A click still selects a row.
`packages/gantt/tests/read-only.spec.ts` checks each path.

## Colors and shape

The chart's colors and bar shape are CSS custom properties, the tokens, in `tachlon/gantt.css`. Set one on
the chart's root element, the `div` with the class `gantt`, in your own CSS or from a script. Your value
wins whatever the order of the stylesheets, because the package sets its tokens at zero specificity.

These lines are from the test "root style overrides reach the scene and the DOM through the observers, and
removing them restores both" in `packages/gantt/tests/theme.spec.ts`. They turn the critical bars green
and the table's head cream while the chart runs:

```ts
      const root = document.querySelector<HTMLElement>(".gantt")!;
      root.style.setProperty("--gantt-crit", "#00aa00");
      root.style.setProperty("--gantt-crit-fill", "#00aa00");
      root.style.setProperty("--gantt-crit-edge", "#00aa00");
      root.style.setProperty("--gantt-axis", "#ffeecc");
```

The chart sees the change by itself: there is nothing to call. Removing the properties brings the
package's own colors back.

## Light and dark

With nothing set, the chart follows the system's light or dark setting. `data-theme="dark"` or
`data-theme="light"` on the root element forces one, whatever the system says.

## The token names

This book lists no tokens. `npx tachlon describe tokens` prints every one for the version you installed.
Run `npx tachlon describe fit` before you theme the chart: it prints the steps, the look first.

Next: [Load and save P6 and MS Project files](dev-files.md).
