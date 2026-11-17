// Our side's hooks, read from `window.gantt` (the hook `packages/gantt/src/test-hook.ts` publishes) and the
// chart's `#timeline` canvas, on the bench page (`ours-page.ts`). Each runs in the page, so each is an
// arrow function: Playwright sends its source text, and a method's text is not an expression.

import type { Side } from "../side.ts";

const ours: Side = {
  name: "ours",

  ready: (rows) => {
    const g = (window as any).gantt;
    if (g?.ready !== true) return false;
    const s = g.status();
    return s.n === rows && s.frames >= 1 && s.rowCount > 0 && s.bars > 0;
  },

  scrollTarget: () => {
    const s = (window as any).gantt.status();
    const r = document.getElementById("timeline")!.getBoundingClientRect();
    const rowsHigh = r.height - s.axisHeight;
    return {
      x: r.x,
      y: r.y + s.axisHeight,
      width: r.width,
      height: rowsHigh,
      top: s.scrollY,
      left: s.t[0] * s.pxPerMinute,
      maxTop: Math.max(0, s.rowCount * s.rowHeight - rowsHigh),
      // Our time axis has no end: the viewport pans without a clamp (`Viewport::pan`).
      maxLeft: Infinity,
    };
  },

  // The bars the last frame drew (physical px on the canvas), less the summaries', as CSS px on the page. The page's
  // task ids are the dataset's: `mount` keeps a task's id as its activity ID, so a bar's dataset id is its details'.
  leafBars: () => {
    const g = (window as any).gantt;
    const r = document.getElementById("timeline")!.getBoundingClientRect();
    const dpr = window.devicePixelRatio;
    type Rect = { task: number; x: number; y: number; width: number; height: number };
    const bars = (g.app.test_bar_rects() as Rect[]).flatMap((b) => {
      const details = g.app.task_details(b.task);
      return details?.summary === true ? [] : [{ x: r.x + b.x / dpr, y: r.y + b.y / dpr, width: b.width / dpr, height: b.height / dpr, id: String(b.task), ...(details.activityId === "" ? {} : { datasetId: String(details.activityId) }) }];
    });
    return { bars, pxPerDay: g.status().pxPerMinute * 1440 };
  },

  // A committed drag is an edit, and every edit moves the revision.
  dragState: () => String((window as any).gantt.status().revision),

  backend: () => (window as any).gantt?.status().backend ?? null,

  inputBytes: async () => {
    const url = (window as any).ganttDataUrl;
    return typeof url === "string" ? (await (await fetch(url)).arrayBuffer()).byteLength : null;
  },

  rowsOnScreen: () => {
    const s = (window as any).gantt.status();
    return Math.floor((document.getElementById("timeline")!.getBoundingClientRect().height - s.axisHeight) / s.rowHeight);
  },

  // Vite's dev server injects its client script; a built page has none.
  debugBuild: () => document.querySelector('script[src="/@vite/client"]') !== null,

  // The dataset hash inside the file the bench page fetched, only when that file (fetched again) hashes to the
  // SHA-256 written beside it and holds the hash the page took out at load. Null for any other page.
  datasetSha256: async () => {
    const w = window as any;
    if (typeof w.ganttDataUrl !== "string" || typeof w.ganttDatasetSha256 !== "string") return null;
    const [file, side] = await Promise.all([fetch(w.ganttDataUrl), fetch(`${w.ganttDataUrl}.sha256`)]);
    if (!file.ok || !side.ok) return null;
    const bytes = await file.arrayBuffer();
    const hash = [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map((b) => b.toString(16).padStart(2, "0")).join("");
    const inside = JSON.parse(new TextDecoder().decode(bytes)).datasetSha256;
    return hash === (await side.text()).trim().split(/\s+/)[0] && inside === w.ganttDatasetSha256 ? inside : null;
  },
};

export default ours;
