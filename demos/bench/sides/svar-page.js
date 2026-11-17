// The SVAR Gantt side's page script: our code, bundled beside SVAR by `svar-setup.ts` (into page.js,
// with React's production build, as an app's build makes it).
//
// SVAR is set up as its own benchmark sets it up (https://github.com/svar-widgets/gantt-performance,
// `svar-react/src/components/GanttChart.jsx` and `App.jsx`, the code behind
// https://svar.dev/blog/react-gantt-benchmark/): a `Gantt` with `cellWidth` 40 and `cellHeight` 35
// inside the `Willow` theme, `tasks` and `links` as props, everything else SVAR's default (its
// virtual rendering, a day scale, the time axis spanning the tasks). The data is handed in as Dates,
// as its readme's example, that benchmark and the large-data demo its readme links ("demo with 10k
// tasks", `demos/cases/GanttPerformance.jsx` in https://github.com/svar-widgets/react-gantt) do; that
// demo sets nothing more for size.
//
// `?cell=<shape>-<size>-<seed>` names the schedule: `cells/<cell>/svar.json` of this directory,
// written by `svar-adapt.ts`. The page takes the dataset's hash out before SVAR sees the data
// (`ganttDatasetSha256`) and notes the file it fetched (`ganttDataUrl`), so the side's
// `datasetSha256` can hash that file again. SVAR's API (its `init` callback's) is `window.gantt`.
//
// The marks the driver splits load at (`demos/bench/side.ts`), placed as ours and Bryntum's are:
// `library-ready` once the library has run (the bundle's modules evaluated); `data-start` once the
// component is made (the empty Gantt mounted) and before the schedule's fetch. The data then goes in
// as SVAR's benchmark gives it: a new `Gantt` (another `key`) with the tasks and links as props, so
// SVAR builds its store from them in that Gantt's first render (`Gantt.jsx`: `dataStore.init` while
// nothing is mounted yet), not by updating the empty one (a render, a commit, an effect that inits the
// store, another render). `init` is one function for the page's life: SVAR re-inits its store when
// it changes. Data to ready holds the fetch, the parse, the Dates made and SVAR's drawing.
//
// SVAR's theme loads its icon font from SVAR's CDN unless its `fonts` is off, and its stylesheet its
// text fonts: `svar-setup.ts` puts both in this directory (`fonts/`, `svar.css`) and `index.html`
// links them, so the page loads nothing from outside the box.

import { createElement as h, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { Gantt, Willow } from "@svar-ui/react-gantt";

performance.mark("library-ready");
const params = new URLSearchParams(location.search);
const dir = `cells/${params.get("cell")}/`;
window.ganttDataDir = dir;
window.ganttProductionBuild = process.env.NODE_ENV === "production";

const init = (api) => {
  window.gantt = api;
};
const EMPTY = { key: "empty", tasks: [], links: [] };

function Page() {
  const [data, setData] = useState(EMPTY);
  useEffect(() => {
    performance.mark("data-start");
    const url = new URL(dir + "svar.json", location.href).href;
    window.ganttDataUrl = url;
    fetch(url)
      .then((res) => res.json())
      .then(({ datasetSha256, tasks, links }) => {
        window.ganttDatasetSha256 = datasetSha256;
        for (const t of tasks) {
          t.start = new Date(t.start);
          if (t.end !== undefined) t.end = new Date(t.end);
        }
        setData({ key: "loaded", tasks, links });
      });
  }, []);
  const gantt = h(Gantt, { key: data.key, cellWidth: 40, cellHeight: 35, tasks: data.tasks, links: data.links, init });
  return h(Willow, { fonts: false }, gantt);
}

createRoot(document.getElementById("root")).render(h(Page));
