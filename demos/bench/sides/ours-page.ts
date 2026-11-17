// Our side's page: a host page that hands the benchmark schedule to `mount(el, { tasks, links })`, the call a
// page that embeds the chart makes. `?cell=<shape>-<size>-<seed>` names the schedule: `cells/<cell>/ours.json` beside
// this page, written by `ours-adapt.ts` (the same tasks, dates and links the other sides load).
//
// The order is the DHTMLX page's: the engine loads first (`preloadEngine`, the library), then `library-ready` and
// `data-start` are marked, then the file is fetched, then `mount` parses it, schedules it and draws. `mount` needs the
// tasks, so the page marks `library-ready` itself, right after the engine can run, instead of leaving it to `mount`
// (which would mark it after the file is in; the driver reads the first mark of a name). So `libraryMs` is the
// navigation to the engine ready, with no file wait in it, and data to ready is the fetch, the parse, the schedule
// and the first frame, as on the other sides.
//
// The page takes `datasetSha256` out of the file and notes the file's URL, as the DHTMLX page does, so the side's
// `datasetSha256` hook can hash that file again.

import { type PlainLink, type PlainTask, mount, preloadEngine } from "gantt-core";
import "gantt-core/gantt.css";

interface OursData {
  datasetSha256: string;
  tasks: PlainTask[];
  links: PlainLink[];
}

const w = window as unknown as { ganttDataUrl: string; ganttDatasetSha256: string };
w.ganttDataUrl = new URL(`cells/${new URLSearchParams(location.search).get("cell")}/ours.json`, location.href).href;

await preloadEngine();
performance.mark("library-ready");
performance.mark("data-start");
const { datasetSha256, tasks, links } = (await fetch(w.ganttDataUrl).then((res) => res.json())) as OursData;
w.ganttDatasetSha256 = datasetSha256;
performance.mark("mount-start");
await mount(document.getElementById("gantt_here") as HTMLDivElement, { tasks, links });
