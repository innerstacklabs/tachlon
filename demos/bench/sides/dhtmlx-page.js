// The DHTMLX side's page script: our code, copied beside the community edition by
// `dhtmlx-setup.ts` (as page.js). A classic script, run right after the library's own build
// (`codebase/dhtmlxgantt.js`, which sets the global `gantt`), as DHTMLX's readme loads it with plain
// `<script>` tags, and its first-mount path as the readme's "Quick start" has it: `gantt.init` on
// the container, then `gantt.parse` with the data.
//
// Set up per DHTMLX's guide "Performance: Ways to Improve"
// (https://docs.dhtmlx.com/gantt/guides/performance/): smart rendering and smart scales on (both its
// defaults, set here so the page says so) and autosize off (its default; "do not enable the autosize
// option for large charts"); `duration_step` 1 with the minute unit, as the guide asks for
// `duration_unit` "minute". The guide's static background and branch loading are PRO only, and it
// has no work-time setting to make (the community edition has no calendars; `dhtmlx-adapt.ts`
// hands it both dates of every task). Three columns (name, start, duration), as the other sides.
// Everything else is DHTMLX's default (the day scale, the time axis spanning the tasks). The chart
// is `window.gantt` for the side's hooks.
//
// `?cell=<shape>-<size>-<seed>` names the schedule: `cells/<cell>/dhtmlx.json` of this directory,
// written by `dhtmlx-adapt.ts`. Its `config` is set before `gantt.parse`; its `datasetSha256` is
// taken out (`ganttDatasetSha256`) and never handed to DHTMLX, and the file's URL is noted
// (`ganttDataUrl`) so the side's `datasetSha256` hashes that file again.
//
// The marks the driver splits load at (`demos/bench/side.ts`), placed as ours and Bryntum's are:
// `library-ready` once the library has run (this script runs right after it); `data-start` once the
// empty chart is made (`gantt.init`) and before the schedule's fetch. The community edition does not
// schedule: `gantt.parse` takes the dates it is given, rolls each project up from its children and
// draws, all before it returns; `ganttParsedAt` is then, which the side's `settled` reports.

(() => {
  performance.mark("library-ready");
  const gantt = window.gantt;
  const dir = `cells/${new URLSearchParams(location.search).get("cell")}/`;
  window.ganttDataDir = dir;
  window.ganttDataUrl = new URL(`${dir}dhtmlx.json`, location.href).href;

  gantt.config.smart_rendering = true;
  gantt.config.smart_scales = true;
  gantt.config.autosize = false;
  gantt.config.duration_step = 1;
  gantt.config.columns = [
    { name: "text", label: "Name", tree: true, width: 200 },
    { name: "start_date", label: "Start date", align: "center" },
    { name: "duration", label: "Duration", align: "center" },
  ];
  gantt.init("gantt_here");

  performance.mark("data-start");
  fetch(window.ganttDataUrl)
    .then((res) => res.json())
    .then(({ datasetSha256, config, data, links }) => {
      window.ganttDatasetSha256 = datasetSha256;
      for (const [key, value] of Object.entries(config)) gantt.config[key] = value;
      gantt.parse({ data, links });
      window.ganttParsedAt = performance.now();
    });
})();
