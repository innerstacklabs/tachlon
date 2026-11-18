// The developer page (plan D2): it reads the URL knobs, then starts the chart through `mount` in `index.ts` and puts the app's chrome
// (`@gantt/chrome`, the Filters menu and the find box) into it, as the hosted app does. The status line is this page's own.
// `?n`, `?stress`, `?groups`, `?schedule`, `?backend=webgl`, `?fallback`, `?beforeEdit`, `?measure` (and its options, which also run on `?schedule`) and `?panic=init`
// are this page's; a host page gets none of them. Everything about the schedule and the drawing lives in the WASM
// module, and the chart's wiring lives in `index.ts`.
import { measureConfig } from "./measure.js";
import { setPageKnobs } from "./page-knobs.js";
import { fallBackToWebgl, taskCount } from "./boot.js";
import { element } from "./dom.js";
import { MountError, mount, type MountOptions } from "./index.js";
import { createStatusLine } from "./status-line.js";
import { installChrome } from "@gantt/chrome";

// This page sets `height: 100%; overflow: hidden` inline on `html` and `body`, which prints only the first sheet; lift it while printing.
const clipped = [document.documentElement, document.body].map((el) => ({ el, height: el.style.height, overflow: el.style.overflow }));
window.addEventListener("beforeprint", () => {
  for (const { el } of clipped) Object.assign(el.style, { height: "auto", overflow: "visible" });
});
window.addEventListener("afterprint", () => {
  for (const { el, height, overflow } of clipped) Object.assign(el.style, { height, overflow });
});

async function sha256(bytes: ArrayBuffer): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return Array.from(digest, (b) => b.toString(16).padStart(2, "0")).join("");
}

async function main(): Promise<void> { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const root = document.getElementById("app") as HTMLDivElement;
  // The message line is the chart's `#overlay`, which `mount` puts in the root; a message before that makes one.
  const say = (text: string): void => {
    let line = document.getElementById("overlay");
    if (line === null) {
      line = document.createElement("div");
      line.id = "overlay";
      root.append(line);
    }
    line.textContent = text;
  };
  const params = new URLSearchParams(location.search);
  const n = taskCount(params);
  const groups = Math.max(0, Math.trunc(Number(params.get("groups"))) || 0);
  const fallback = params.get("fallback");
  const forceWebgl = params.get("backend") === "webgl" || fallback !== null;
  // A measured run drags the stress fixture's head task (plan, M3), or a task it picks on the file `?schedule` loads.
  const measure = measureConfig(params);
  const stress = (measure !== null && !params.has("schedule")) || params.has("stress");

  let schedule: MountOptions["schedule"];
  if (params.has("schedule")) {
    if (stress) {
      say("could not load schedule: ?schedule cannot be combined with ?stress");
      return;
    }
    try {
      const response = await fetch("schedule.dat");
      if (!response.ok || response.headers.get("content-type")?.includes("text/html")) {
        throw new Error(
          "schedule.dat is not being served; start the page with demos/50k/run.sh --schedule <file.xml or file.xer>",
        );
      }
      const bytes = await response.arrayBuffer();
      // The measured result names the file by this hash, so a run can be matched to the schedule it ran on.
      if (measure) measure.scheduleSha256 = await sha256(bytes);
      schedule = new File([bytes], "schedule.dat");
    } catch (err) {
      const text = err instanceof Error ? err.message : String(err);
      say(`could not load schedule: ${text}`);
      return;
    }
  } else {
    schedule = stress ? { stress: n } : { demo: n };
  }

  try {
    // The knobs go by `page-knobs.ts`, not through `mount`'s options: the entry's types name no measure type.
    const statusLine = createStatusLine();
    setPageKnobs({
      statusLine,
      params,
      fallback,
      groups: stress ? groups : undefined,
      measure,
      measured: (outcome) => {
        if ("error" in outcome) return;
        // The download link is page chrome, so it is found on the document, as the overlay is.
        const link = element<HTMLAnchorElement>(document, "result-link");
        link.href = URL.createObjectURL(new Blob([JSON.stringify(outcome, null, 2)], { type: "application/json" }));
        link.hidden = false;
      },
      // A panic leaves the module unusable, so this page starts again: on WebGL2 when WebGPU panicked, as it is otherwise.
      restart: (why) => (why !== null && !forceWebgl ? fallBackToWebgl(why) : location.reload()),
    });
    const gantt = await mount(root, { schedule, forceWebgl, beforeEdit: params.has("beforeEdit") ? () => undefined : undefined });
    installChrome(gantt, root);
    gantt.on("toggle-status", statusLine.toggle);
  } catch (err) {
    if (!(err instanceof MountError)) throw err;
    if (err.stage === "engine") {
      say(`${err.message} ${err.fix ?? ""}`);
    } else if (err.stage === "schedule") {
      say(`${params.has("schedule") ? "could not load schedule" : "could not build schedule"}: ${err.message}`);
    } else if (forceWebgl) {
      say(`render error: ${err.message}`);
      console.error(err.cause);
    } else {
      fallBackToWebgl(err.message);
    }
  }
}

main().catch((err: unknown) => {
  const overlay = document.getElementById("overlay");
  if (overlay) overlay.textContent = `failed to start: ${String(err)}`;
  console.error(err);
});
