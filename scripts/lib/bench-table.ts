// The page's speed table, its notes and its three headline numbers and the hero's speed figure (apps/site/index.html, the regions between <!-- bench:NAME --> marks),
// written from a benchmark results file, never by hand. Times are in seconds.
// Called by scripts/bench-table.ts; packages/gantt/tests-node/bench-page.test.ts checks the page against it.
import { readFileSync } from "node:fs";
import { PRODUCT_NAME } from "../../packages/gantt/src/product.ts";

interface Medians { [key: string]: number }
export interface Results { date: string; command: string; machine: string; cells: Record<string, { datasetSha256: string; matched: number }>; assembled?: string; medians: Record<string, Record<string, Medians>> }
export interface Versions { machine: { model: string }; ours: { release: string; commit: string }; dhtmlx: { version: string } }

// [row heading, tasks, the key in the results file]. A time shows in seconds to two decimals, memory in MB; the drag (dragP95Ms) is in the notes.
const ROWS: [string, number, string][] = [
  ["Open 50,000 tasks", 50000, "loadMedianMs"],
  ["Scroll 50,000 tasks, slowest frame", 50000, "scrollMaxMs"],
  ["Open 10,000 tasks", 10000, "loadMedianMs"],
  ["Whole-browser memory at 50,000 tasks", 50000, "memoryFootprintMB"],
];
const SHAPES = ["tree4", "chain"];
const esc = (s: string): string => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");

const SIZES = [10000, 50000];

function median(r: Results, c: string, side: string, key: string): number {
  const v = r.medians[c]?.[side]?.[key];
  if (v === undefined) throw new Error(`no ${key} for ${side} at ${c}`);
  return v;
}

/** Milliseconds as seconds, to `digits` decimals. */
export const sec = (ms: number, digits = 2): string => (ms / 1000).toFixed(digits);

/** One median: memory in MB to one decimal, a time in seconds to two. */
const figure = (r: Results, side: string, shape: string, tasks: number, key: string): string => {
  const v = median(r, `${shape}-${tasks}`, side, key);
  return key === "memoryFootprintMB" ? `${v.toLocaleString("en-US", { minimumFractionDigits: 1 })} MB` : `${sec(v)} s`;
};

/** `f` of each cell, in the table's order: per size, the shapes joined by " / ". */
const perCell = (f: (c: string) => string): string => SIZES.map((n) => `${n.toLocaleString("en-US")} tasks ${SHAPES.map((s) => f(`${s}-${n}`)).join(" / ")}`).join(", ");

/** One side's cell, tree then chain: the figure in bold where this side's median is the smaller of the two (lower is better in every row). */
const cell = (r: Results, side: string, tasks: number, key: string): string => SHAPES.map((shape) => {
  const [mine, other] = [side, side === "ours" ? "dhtmlx" : "ours"].map((s) => median(r, `${shape}-${tasks}`, s, key));
  const text = figure(r, side, shape, tasks, key);
  return mine < other ? `<strong class="fast">${text}</strong>` : text;
}).join(" / ");

/** The table. SVAR and Bryntum were not in the run (`sides`), so the table has no column for them and the method note says so. */
export function tableBlock(r: Results): string {
  const rows = ROWS.map(([head, n, key]) =>
    `                <tr><th scope="row">${head}</th><td>${cell(r, "ours", n, key)}</td><td>${cell(r, "dhtmlx", n, key)}</td></tr>`);
  return `<div class="table-scroll">
            <table>
              <caption class="visually-hidden">${PRODUCT_NAME} against DHTMLX Gantt at 50,000 and 10,000 tasks; lower is better</caption>
              <thead><tr><th scope="col">Tree schedule / chain schedule</th><th scope="col">${PRODUCT_NAME}</th><th scope="col">DHTMLX Gantt (free edition)</th></tr></thead>
              <tbody>
${rows.join("\n")}
              </tbody>
            </table>
          </div>`;
}

/** The method paragraph: the machine, the date, the dataset hashes and the run command, all from the results file. */
export function methodBlock(r: Results, v: Versions): string {
  const runs = Math.min(...Object.values(r.cells).map((c) => c.matched));
  return `<p>Each cell is the median of ${runs} runs of each side on a ${SHAPES[0]} schedule, then on a ${SHAPES[1]} schedule (${SHAPES.join(" / ")}). Measured ${r.date} on a ${esc(v.machine.model)}: ${PRODUCT_NAME} ${v.ours.release} (build ${v.ours.commit.slice(0, 7)}), against DHTMLX Gantt ${v.dhtmlx.version}. We have not measured SVAR Gantt or Bryntum Gantt yet. Memory is the whole browser's total, 20 seconds after the page loads. Dataset SHA-256 (${SHAPES.join(" / ")}): ${perCell((c) => `<code>${r.cells[c]!.datasetSha256}</code>`)}. Each side loads its own file of it, in bytes: ${PRODUCT_NAME} ${perCell((c) => median(r, c, "ours", "inputBytes").toLocaleString("en-US"))}; DHTMLX ${perCell((c) => median(r, c, "dhtmlx", "inputBytes").toLocaleString("en-US"))}. The runs were made with <code>${esc(r.command)}</code>. ${r.assembled ? `${r.assembled} ` : ""}A script writes every figure on this page from that run's results file, demos/bench/results/${r.date}.json. Nobody types them in.</p>`;
}

/** The three headline numbers of the speed section: the open time at 50,000 tasks (the mean of the two shapes, one figure), the slowest scroll frame, and how much faster the chain opens. */
export function statsBlock(r: Results): string {
  const load = (side: string, shape: string): number => median(r, `${shape}-50000`, side, "loadMedianMs");
  const frames = (side: string): number[] => SHAPES.map((s) => median(r, `${s}-50000`, side, "scrollMaxMs"));
  const open = Number(((load("ours", "tree4") + load("ours", "chain")) / 2000).toPrecision(1));
  const times = Math.floor((load("dhtmlx", "chain") / load("ours", "chain")) * 10) / 10;
  return `<div class="stats">
            <div class="stat"><b>${open} s</b><span>to open 50,000 tasks, with the schedule calculated</span></div>
            <div class="stat"><b>${sec(Math.max(...frames("ours")), 3)} s</b><span>slowest frame while scrolling 50,000 tasks. DHTMLX: ${sec(Math.min(...frames("dhtmlx")), 3)} s</span></div>
            <div class="stat"><b>${times.toFixed(1)}&times;</b><span>faster than DHTMLX on a 50,000-task chain: ${sec(load("ours", "chain"))} s against ${sec(load("dhtmlx", "chain"))} s</span></div>
          </div>`;
}

/** The hero's speed figure, our side only: the open time at 50,000 tasks (the same figure as the stats), at 10,000 tasks, and the slowest scroll frame. `--t` is the same figure in seconds, which the page draws as a bar on a one-second axis. */
export function heroBlock(r: Results): string {
  const ours = (tasks: number, key: string): number[] => SHAPES.map((s) => median(r, `${s}-${tasks}`, "ours", key));
  const mean = (v: number[]): number => v.reduce((a, b) => a + b, 0) / v.length;
  const rows: [string, string][] = [
    [String(Number((mean(ours(50000, "loadMedianMs")) / 1000).toPrecision(1))), "to open 50,000 tasks, with the schedule calculated"],
    [sec(mean(ours(10000, "loadMedianMs"))), "to open 10,000 tasks"],
    [sec(Math.max(...ours(50000, "scrollMaxMs")), 3), "slowest frame while scrolling 50,000 tasks"],
  ];
  return `<figure class="speed">
${rows.map(([s, label]) => `        <p style="--t:${s}"><b>${s} s</b><span>${label}</span></p>`).join("\n")}
        <figcaption><span class="axis" aria-hidden="true"><i>0</i><i>1 second</i></span>Medians from our run of ${r.date}. <a href="#benchmark">See the full table</a></figcaption>
      </figure>`;
}

/** The fine print beside the table (gantt#1284), every figure from this run. A clause stands only while the run supports it and is left out when it does not; each figure is a span `bench-page.test.ts` checks against the file. */
export function finePrint(r: Results): string {
  const get = (c: string, side: string, key: string): number => median(r, c, side, key);
  const cells = SHAPES.flatMap((s) => SIZES.map((n) => `${s}-${n}`));
  const all = (side: string, key: string): number[] => cells.map((c) => get(c, side, key));
  const fig = (name: string, v: number | string): string => `<span data-fine="${name}">${v}</span>`;
  const five = (v: number): number => Math.round(v / 5) * 5;
  const fiveUp = (v: number): number => Math.ceil(v / 5) * 5;
  const [chainO, chainD] = [get("chain-50000", "ours", "loadMedianMs"), get("chain-50000", "dhtmlx", "loadMedianMs")];
  const times = Math.floor((chainD / chainO) * 10) / 10;
  const [frameO, frameD] = [Math.max(...all("ours", "scrollMaxMs")), Math.min(...all("dhtmlx", "scrollMaxMs"))];
  const [treeO, treeD] = [get("tree4-10000", "ours", "loadMedianMs"), get("tree4-10000", "dhtmlx", "loadMedianMs")];
  const less = (key: string): boolean => cells.every((c) => get(c, "dhtmlx", key) < get(c, "ours", key));
  const [memO, memD] = [all("ours", "memoryFootprintMB"), all("dhtmlx", "memoryFootprintMB")];
  /** A range of one figure per cell, rounded at each end; one figure when both ends round alike. */
  const range = (n: string, v: number[], lo: (x: number) => string | number, hi: (x: number) => string | number): string => {
    const [a, b] = [lo(Math.min(...v)), hi(Math.max(...v))];
    return a === b ? fig(n, a) : `${fig(`${n}Lo`, a)} to ${fig(`${n}Hi`, b)}`;
  };
  const s2 = (x: number): string => sec(x);
  const s3 = (x: number): string => sec(x, 3);
  const [tree5O, tree5D] = [get("tree4-50000", "ours", "loadMedianMs"), get("tree4-50000", "dhtmlx", "loadMedianMs")];
  const tree5 = tree5O < tree5D ? `, and the 50,000-task tree a little faster (${fig("tree50O", s2(tree5O))} s against ${fig("tree50D", s2(tree5D))} s)` : "";
  const open = times > 1 ? `${PRODUCT_NAME} opens the 50,000-task chain ${fig("times", times.toFixed(1))} times faster than DHTMLX (${fig("chainO", s2(chainO))} s against ${fig("chainD", s2(chainD))} s)${tree5}.` : "";
  const half = frameO / frameD >= 0.4 && frameO / frameD <= 0.6 ? `${open ? "Its" : `${PRODUCT_NAME}'s`} slowest scroll frame in every test is about half of DHTMLX's (${fig("frameO", s3(frameO))} s against ${fig("frameD", s3(frameD))} s).` : "";
  const all50 = (side: string, key: string): number[] => SHAPES.map((s) => get(`${s}-50000`, side, key));
  const [dragO, dragD] = [Math.max(...all50("ours", "dragP95Ms")), all50("dhtmlx", "dragP95Ms")];
  const drag = dragD.every((d) => Math.abs(d - dragO) / dragO <= 0.05) ? `Dragging a task at 50,000 is even (${fig("dragO", s3(dragO))} s against ${range("dragD", dragD, s3, s3)} s).` : "";
  const behind = [
    treeD < treeO ? `opens the 10,000-task tree faster (${fig("treeD", s2(treeD))} s against ${fig("treeO", s2(treeO))} s)${tree5D < tree5O ? ` and the 50,000-task tree faster (${fig("tree50D", s2(tree5D))} s against ${fig("tree50O", s2(tree5O))} s)` : ""}` : "",
    less("libraryMedianMs") ? `loads its library faster (${range("libD", all("dhtmlx", "libraryMedianMs"), s2, s2)} s against ${range("libO", all("ours", "libraryMedianMs"), s2, s2)} s)` : "",
    less("memoryFootprintMB") ? `uses less memory (about ${range("memD", memD, five, five)} MB against ${range("memO", memO, fiveUp, fiveUp)} MB)` : "",
  ].filter(Boolean);
  const ahead = [open, half, drag].filter(Boolean).join(" ");
  const dhtmlx = behind.length ? `DHTMLX ${behind.length > 1 ? `${behind.slice(0, -1).join(", ")}, and ${behind.at(-1)}` : behind[0]}.` : "";
  const heading = ahead && dhtmlx ? `<strong>Where ${PRODUCT_NAME} is ahead, and where it isn't yet.</strong> ` : "";
  const promise = behind.length === 3 ? " We're working on all three, and we re-run this table when a change moves one of its numbers beyond normal run-to-run variation; the date above is the last run." : "";
  const text = [ahead && `${heading}${ahead}`, dhtmlx && `${dhtmlx}${promise}`].filter(Boolean).join(" ");
  return text ? `<p data-fine-print>${text}</p>` : "";
}

/** `html` with each region between `<!-- bench:NAME -->` and `<!-- /bench:NAME -->` replaced by what the results say. */
export function withBenchTable(html: string, r: Results, v: Versions): string {
  const releases = JSON.parse(readFileSync(new URL("../../packages/gantt/releases.json", import.meta.url), "utf8")) as { version: string; date: string }[];
  // run: the run's date in the line under the stats; release: the items under "Latest releases", all from packages/gantt/releases.json.
  const regions: Record<string, string> = { hero: heroBlock(r), stats: statsBlock(r), table: tableBlock(r), ahead: finePrint(r), method: methodBlock(r, v), run: `${r.date} (${PRODUCT_NAME} ${v.ours.release})`, release: releases.map((x, i) => `<li><span class="ver">${x.version}</span>${x.date}${i === 0 ? ` <span class="latest">Latest</span>` : ""}</li>`).join(" ") };
  return html.replace(/(<!-- bench:(\w+) -->)[\s\S]*?(<!-- \/bench:\2 -->)/g, (_all, open: string, name: string, close: string) => {
    if (!(name in regions)) throw new Error(`no bench region named ${name}`);
    return `${open}${regions[name]}${close}`;
  });
}
