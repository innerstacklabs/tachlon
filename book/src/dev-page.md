# Put a chart on a page

This part of the book is written for a developer and their coding assistant.

Tachlon draws in one element of your page. This chapter puts three tasks on a page. It assumes the package
is installed: [Install it in your app](install.md) covers that.

## Plain JavaScript

The element is a `div` with the class `gantt` and a box with a width and a height. This one is from
`examples/plain/index.html`:

```html
<div id="chart" class="gantt" style="width: 800px; height: 300px"></div>
```

`mount` takes that element and the tasks to draw, and returns a promise of the chart. This is the start of
`examples/plain/main.js`:

```js
import { mount } from "tachlon";
import "tachlon/gantt.css";

const tasks = [
  { id: 1, name: "Design", start: "2026-11-02", duration: 3 },
  { id: 2, name: "Build", start: "2026-11-05", duration: 5 },
  { id: 3, name: "Ship", start: "2026-11-12", duration: 1 },
];
const links = [{ from: 1, to: 2 }, { from: 2, to: 3 }];

// forceWebgl: this page needs no GPU. Drop it to let the chart use WebGPU when the browser has it.
const chart = await mount(document.getElementById("chart"), { tasks, links, forceWebgl: true });
```

`chart.destroy()` stops the chart and empties its element. CI runs this page: `node examples/check.ts plain
react`, a step of the `web` job in `.github/workflows/ci.yml`, installs the packed package into a copy of
the folder and drives the page in Chromium with `examples/plain/spec.ts`.

## React, Vue and Angular

The same package has a component named `Gantt` for each: `tachlon/react`, `tachlon/vue` (Vue 3) and
`tachlon/angular` (a standalone component with the selector `gantt-chart`). Each takes the options of
`mount` as props or inputs, draws the chart when it mounts and calls `destroy()` when it is removed. The
package README has the tag to write and what each one needs:

- [Use it in React](https://github.com/innerstacklabs/tachlon/blob/main/packages/gantt/README.md#use-it-in-react)
- [Use it in Vue](https://github.com/innerstacklabs/tachlon/blob/main/packages/gantt/README.md#use-it-in-vue)
- [Use it in Angular](https://github.com/innerstacklabs/tachlon/blob/main/packages/gantt/README.md#use-it-in-angular)

## What `mount` takes

This book lists no options, because a list here would go out of date. Ask the package you installed:
`npx tachlon describe options` prints every option of `mount` for the version in `node_modules`, and
`npx tachlon describe errors` prints the codes a failed start carries, with the fix for each.

Next: [Give it your data and keep your own ids](dev-data.md).
