# Install it in your app

The quickest way is to ask your AI assistant: paste the prompt under
[Add it](https://github.com/innerstacklabs/tachlon#add-it) in the README. The assistant installs the
package, puts the chart on one existing page, and runs your build and `npx tachlon doctor` until both pass.

To do it by hand, put the Tachlon chart in a Vite React app: install the package, let `npx tachlon init` write the chart
component, and open a schedule file. The video goes through every step; the steps are below it.

<video class="clip" controls playsinline preload="metadata" poster="media/install-setup.webp">
  <source src="media/install-setup.mp4" type="video/mp4">
</video>

## The steps

1. Start from a Vite React app, or make one: `npm create vite@latest my-app -- --template react-ts`.
2. In the app folder, install the package and write the chart file:

   ```sh
   npm install tachlon
   npx tachlon init
   ```

   `init` writes `src/tachlon-chart.tsx`, a component with three sample tasks, and prints the line to add
   to a page. It changes no file you already have.
3. In `src/App.tsx`, add `import Chart from "./tachlon-chart";` and render `<Chart />` inside a
   full-width `div`.
4. Run `npm run dev` and open the page. The three tasks are drawn with dates, durations and zero days of
   float. <kbd>Ctrl</kbd> and the mouse wheel zoom the timeline.
5. To open a schedule, keep the file the user picks in state (`useState<File>()`, set from the `onChange`
   of an `<input type="file">`) and, once there is one, render the `Gantt` component from `tachlon/react`
   with that file as its `schedule` prop. It reads a P6 XER file or an MS Project XML file.
6. With no bundler, a plain HTML page loads the package from jsDelivr: a stylesheet link and an import
   map, pinned to the package version you chose. The package README, "No bundler", has the page.
7. If a chart does not draw, run `npx tachlon doctor`. It checks that the package is listed and
   installed once, the engine file, a stylesheet import and the page's Content-Security-Policy, and asks a
   Chrome, Edge or Chromium on your machine whether the chart gets WebGPU or WebGL2. It writes nothing in
   your project and says how to fix each problem.
