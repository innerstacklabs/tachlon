// The hosted app's page: it places the chart in its box through `gantt-core`, the built package, and shows which backend draws.
// It reads three URL knobs; `?theme=dark|light` is the third (below). `?backend=webgl` is added by the page itself when it reloads after a
// failure the chart could not get past by itself (a panic at the start or in a frame), with `?restarted=1`, so it reloads that way once. A WebGPU failure the chart switches past, to
// WebGL2 in its root, keeps the page and its edits: the page puts its menu back into the new toolbar. `?sample=<id>` opens one of the samples in `samples.json` (see `samples.ts`); with none, the page
// opens the office fit-out (gs#821). The developer page's other knobs belong to `packages/gantt/`.
import { type Gantt, MountError, PRODUCT_NAME, mount, preloadEngine } from "gantt-core";
import "gantt-core/gantt.css";
import { type Chrome, installChrome } from "@gantt/chrome";
import { installAutosave } from "./autosave";
import { placeCsvExport, placeExcelExport } from "./export-items";
import { keepHiddenColumns, restoreHiddenColumns } from "./hidden-columns";
import { applyLayout, layoutNames, saveLayout } from "./layouts";
import { type Catalog, type Sample, choose, fetchSample, installMenu, loadCatalog, lookup, prefetchSample, say } from "./samples";
import { installSpeedLine } from "./speed-line";

// The engine is the longest wait, so it starts before anything else is read and downloads and compiles beside the sample list (gs#1336).
void preloadEngine();

// The page names the product from the chart's entry (`packages/gantt/src/product.ts`), never in its own HTML, so a rename is one file.
document.title = PRODUCT_NAME;
const loadingLine = document.querySelector("#app > p[role=status]");
if (loadingLine !== null) loadingLine.textContent = `Loading ${PRODUCT_NAME}…`;

/** The schedule the chart mounts with when the sample is a file: the file opens over it. */
const DEMO_TASKS = 200;
/** The sample the page opens when the URL names none: the realistic small project, not the generated plan. */
const DEFAULT_SAMPLE = "office-fitout";

function find<T extends HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (found === null) throw new Error(`the host page has no #${id}`);
  return found as T;
}

/** The sample menu's list and status line, built from the page's `<template>`; they join the toolbar once the chart has made it. */
function sampleControls(): DocumentFragment {
  return find<HTMLTemplateElement>("sample-controls").content.cloneNode(true) as DocumentFragment;
}

/** The menu's not-yet-built items go under the samples; each one that carries `data-coming-soon` is disabled for assistive tech and has no handler. */
function placeComingSoon(list: HTMLElement): void {
  const items = find<HTMLTemplateElement>("coming-soon-items").content.cloneNode(true) as DocumentFragment;
  for (const item of items.querySelectorAll("[data-coming-soon]")) item.setAttribute("aria-disabled", "true");
  list.append(items);
}

/** "New schedule" goes first: choosing it closes the menu and starts an empty schedule (`Gantt.newSchedule` asks first when there are unsaved changes); a refusal is a line beside the button. */
function placeNewSchedule(list: HTMLElement, button: () => HTMLElement, gantt: Gantt, status: HTMLElement): void {
  const items = find<HTMLTemplateElement>("new-schedule-item").content.cloneNode(true) as DocumentFragment;
  items.querySelector("[data-new-schedule]")!.addEventListener("click", () => {
    list.hidden = true;
    button().setAttribute("aria-expanded", "false");
    // A started schedule is announced on the document (`first-screen-line.ts` takes its sample line away); a refusal announces nothing.
    void gantt.newSchedule().then((result) => (say(status, result.ok ? "" : result.error), result.ok && document.dispatchEvent(new Event("gantt:new-schedule"))), (err: unknown) => console.error("hosted: new schedule failed", err));
  });
  list.prepend(items);
}

/**
 * "Compare with a file..." goes right after "Open a file…" (the first item). Choosing it closes the menu and opens a chooser for a second file, such as
 * last month's; the file becomes the baseline in one undo step, and a file that is not a schedule is a line in the chart's alert line.
 */
function placeCompare(list: HTMLElement, button: () => HTMLElement, compare: (file: File) => Promise<void>): void {
  const items = find<HTMLTemplateElement>("compare-item").content.cloneNode(true) as DocumentFragment;
  const item = items.querySelector<HTMLElement>("[data-compare]")!;
  const input = items.querySelector<HTMLInputElement>("#compare-input")!;
  item.addEventListener("click", () => {
    list.hidden = true;
    button().setAttribute("aria-expanded", "false");
    input.click();
  });
  input.addEventListener("change", () => {
    const file = input.files?.[0];
    // The same file chosen twice must fire again.
    input.value = "";
    if (file !== undefined) void compare(file).catch((err: unknown) => console.error("hosted: compare failed", err));
  });
  const open = list.querySelector("[data-open-file]");
  if (open === null) list.prepend(items);
  else open.after(items);
}

/** "Open another schedule in a new tab" goes right after "Compare with a file...": a link to this page that the browser opens in a second tab, so choosing it only closes the menu and this tab keeps its schedule. */
function placeNewTab(list: HTMLElement, button: () => HTMLElement): void {
  const items = find<HTMLTemplateElement>("new-tab-item").content.cloneNode(true) as DocumentFragment;
  const item = items.querySelector<HTMLElement>("[data-new-tab]")!;
  const choose = (): void => {
    list.hidden = true;
    button().setAttribute("aria-expanded", "false");
  };
  item.addEventListener("click", choose);
  item.addEventListener("keydown", (event) => {
    // A link follows Enter by itself; Space does the same here, as on the other items.
    if (event.key !== " ") return;
    event.preventDefault();
    item.click();
  });
  list.querySelector("[data-compare]")!.after(items);
}

/** The Save item goes under the samples; choosing it closes the menu and makes the call Ctrl+S makes, so the file downloads and the unsaved mark clears. */
function placeSave(list: HTMLElement, button: () => HTMLElement, save: () => Promise<void>): void {
  const items = find<HTMLTemplateElement>("save-item").content.cloneNode(true) as DocumentFragment;
  items.querySelector("[data-save]")!.addEventListener("click", () => {
    list.hidden = true;
    button().setAttribute("aria-expanded", "false");
    void save().catch((err: unknown) => console.error("hosted: save failed", err));
  });
  list.append(items);
}

/** "Copy the list" goes under "Export the list as Excel"; choosing it (a click, Enter or Space) closes the menu and puts the visible rows on the clipboard as tab-separated text, so a tablet reader needs no Ctrl+A and Ctrl+C. */
function placeCopyList(list: HTMLElement, button: () => HTMLElement, gantt: Gantt): void {
  const items = find<HTMLTemplateElement>("copy-list-item").content.cloneNode(true) as DocumentFragment;
  const item = items.querySelector<HTMLElement>("[data-copy-list]")!;
  const choose = (): void => {
    list.hidden = true;
    button().setAttribute("aria-expanded", "false");
    void gantt.copyList().catch((err: unknown) => console.error("hosted: copy the list failed", err));
  };
  item.addEventListener("click", choose);
  item.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    choose();
  });
  list.append(items);
}

/** "Save image (PNG)" goes under "Copy the list"; choosing it (a click, Enter or Space) closes the menu and downloads a picture of the first printed page as `<name>.png` (`untitled` for a sample). */
function placeImageExport(list: HTMLElement, button: () => HTMLElement, gantt: Gantt): void {
  const items = find<HTMLTemplateElement>("export-image-item").content.cloneNode(true) as DocumentFragment;
  const item = items.querySelector<HTMLElement>("[data-export-image]")!;
  const choose = (): void => {
    list.hidden = true;
    button().setAttribute("aria-expanded", "false");
    void gantt.exportImage().catch((err: unknown) => console.error("hosted: save image failed", err));
  };
  item.addEventListener("click", choose);
  item.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    choose();
  });
  list.append(items);
}

/** "Print..." and "Print on A4..." go under "Save image (PNG)"; choosing one (a click, Enter or Space) closes the menu and prints the pages: "Print..." as Ctrl+P does, "Print on A4..." on A4 paper for this one print. */
function placePrint(list: HTMLElement, button: () => HTMLElement, gantt: Gantt): void {
  const prints = [
    ["print-item", () => window.print()],
    ["print-a4-item", () => gantt.printOn("a4")],
  ] as const;
  for (const [template, print] of prints) {
    const items = find<HTMLTemplateElement>(template).content.cloneNode(true) as DocumentFragment;
    const item = items.querySelector<HTMLElement>("[data-print]")!;
    const choose = (): void => {
      list.hidden = true;
      button().setAttribute("aria-expanded", "false");
      print();
    };
    item.addEventListener("click", choose);
    item.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      choose();
    });
    list.append(items);
  }
}

/**
 * "Save layout..." and "Apply layout..." go under "Print on A4..."; choosing one (a click, Enter or Space) closes the menu and asks for a name: "Save layout..." keeps the print setup on screen under it in
 * this browser, "Apply layout..." puts a kept one back (`layouts.ts`). The line the page says goes in the status line, with the parts the schedule could not take.
 */
function placeLayouts(list: HTMLElement, button: () => HTMLElement, gantt: Gantt, status: HTMLElement): void {
  const asks = {
    save: () => {
      const name = window.prompt("Name this layout")?.trim();
      return name ? saveLayout(gantt.commands, name) : "";
    },
    apply: () => {
      const names = layoutNames();
      if (names.length === 0) return "No layout is kept in this browser yet: Save layout... keeps one.";
      const name = window.prompt(`Apply which layout?\n${names.join("\n")}`, names[names.length - 1])?.trim();
      return name ? applyLayout(gantt.commands, name) : "";
    },
  } as const;
  for (const [template, ask] of [["save-layout-item", asks.save], ["apply-layout-item", asks.apply]] as const) {
    const item = find<HTMLTemplateElement>(template).content.cloneNode(true) as DocumentFragment;
    const choose = (): void => {
      list.hidden = true;
      button().setAttribute("aria-expanded", "false");
      say(status, ask());
    };
    const menuitem = item.querySelector<HTMLElement>("[data-layout]")!;
    menuitem.addEventListener("click", choose);
    menuitem.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      choose();
    });
    list.append(item);
  }
}

/** Put the list and the status line in the file group, under the chart's own "Open…" button, which becomes the menu's button. */
function placeInToolbar(controls: readonly Node[]): void {
  find("toolbar-file").append(...controls);
}

/** The page has started again once already (`?restarted=1`): a chart that stops again shows why, so a start that always fails cannot reload forever. */
const RESTARTED = "restarted";

/**
 * A panic leaves the chart's module unusable, so the page starts again, on WebGL2 (a failed WebGPU canvas cannot take a WebGL context).
 * It does that once: a page that has already started again shows the reason instead.
 */
function reloadOnWebgl(why: string): void {
  const params = new URLSearchParams(location.search);
  if (params.has(RESTARTED)) {
    showStartFailure(why);
    return;
  }
  console.warn(`hosted: the chart stopped, reloading on WebGL2: ${why}`);
  params.set("backend", "webgl");
  params.set(RESTARTED, "1");
  location.replace(`${location.pathname}?${params}`);
}

/** How long the chart after a restart draws before the page counts its start as a success and drops `?restarted=1`. */
const RESTART_SETTLED_MS = 5000;

/**
 * The start after a restart drew and kept drawing for `RESTART_SETTLED_MS`: `?restarted=1` goes from the address bar, so a later panic
 * reloads once again (here, or on a sample chosen from the menu). A chart that panics before then shows why, so a chart that always
 * panics at or soon after its start cannot reload in a loop.
 */
function clearRestarted(): void {
  const params = new URLSearchParams(location.search);
  if (!params.has(RESTARTED)) return;
  params.delete(RESTARTED);
  const query = params.toString();
  history.replaceState(history.state, "", `${location.pathname}${query === "" ? "" : `?${query}`}${location.hash}`);
}

/** A built-in schedule reaches the chart only when it mounts, so choosing one starts the page again on it. */
function reloadOnSample(sample: Sample): void {
  const params = new URLSearchParams(location.search);
  params.set("sample", sample.id);
  location.assign(`${location.pathname}?${params}`);
}

/** The schedule to mount with: a built-in sample's own, else the small demo. A file sample opens after the mount. */
function startSchedule(picked: Sample | undefined): { demo: number } {
  return { demo: picked !== undefined && picked.file === null ? picked.activities : DEMO_TASKS };
}

/** The engine cannot start: replace the chart's small corner line with the page's alert block (a `<template>`), which names the reason and what the page needs. */
function showStartFailure(reason: string): void {
  const block = find<HTMLTemplateElement>("start-failure-template").content.cloneNode(true) as DocumentFragment;
  block.querySelector("[data-reason]")!.textContent = reason;
  document.getElementById("start-failure")?.remove();
  find("app").append(block);
  const overlay = document.getElementById("overlay");
  if (overlay !== null) overlay.textContent = "";
}

async function main(): Promise<void> {
  const root = find<HTMLDivElement>("app");
  const label = find<HTMLSpanElement>("backend-label");
  const fragment = sampleControls();
  const status = fragment.querySelector<HTMLSpanElement>("#sample-status")!;
  const list = fragment.querySelector<HTMLElement>("#sample-list")!;
  // The nodes, kept: the toolbar they go into is the chart's, and a switch to WebGL2 makes a new one they go back into.
  const controls = Array.from(fragment.childNodes);
  const params = new URLSearchParams(location.search);
  const forceWebgl = params.get("backend") === "webgl";
  const wanted = params.get("sample") ?? DEFAULT_SAMPLE;
  // `?theme=dark` or `?theme=light` forces the chart's set (data-theme) and the page's own color-scheme; any other value is ignored.
  const theme = params.get("theme");
  if (theme === "dark" || theme === "light") {
    root.setAttribute("data-theme", theme);
    document.documentElement.style.colorScheme = theme;
  }

  const catalog: Catalog = await loadCatalog();
  let picked: Sample | undefined;
  const found = lookup(catalog, wanted);
  if ("sample" in found) picked = found.sample;
  else if (params.has("sample")) say(status, found.reason);

  // A file sample downloads while the engine starts; `data-start` (below) still marks where the page opens it, once the chart is made.
  if (picked !== undefined) prefetchSample(picked);

  let gantt: Gantt;
  let chrome: Chrome;
  let menuButton: HTMLButtonElement;
  let bindMenu: ((button: HTMLElement) => void) | null = null;
  const showBackend = (): void => {
    label.textContent = gantt.backend();
    find("toolbar").title = `Drawing with ${label.textContent}`;
  };
  try {
    gantt = await mount(root, { schedule: startSchedule(picked), forceWebgl });
    // The Filters menu, the find box and the file controls (`apps/chrome`); they go back into the chart's new toolbar after a switch to WebGL2 by themselves.
    chrome = installChrome(gantt, root);
    // `s` with focus in the chart shows the open time and the frame numbers (gantt#1790).
    installSpeedLine(gantt, find("speed-line"));
    gantt.on("error", ({ source, message, fatal }) => {
      // The chart has stopped (a panic, or a switch that could not start WebGL2): only a fresh page brings it back.
      if (fatal) {
        reloadOnWebgl(message);
        return;
      }
      // The chart switched to WebGL2 in its box, with the schedule and its edits: its toolbar is new, so the menu goes back into it.
      if (source === "backend") {
        placeInToolbar(controls);
        restoreHiddenColumns(root);
        menuButton = find<HTMLButtonElement>("open-button");
        bindMenu?.(menuButton);
        showBackend();
      }
    });
    let stopped = false;
    gantt.on("error", ({ fatal }) => void (stopped ||= fatal));
    gantt.on("load", () => setTimeout(() => stopped || clearRestarted(), RESTART_SETTLED_MS));
    showBackend();
    placeInToolbar(controls);
    restoreHiddenColumns(root);
    keepHiddenColumns(root);
  } catch (err) {
    if (!(err instanceof MountError)) throw err;
    if (err.stage === "render" && !forceWebgl) {
      reloadOnWebgl(err.message);
      return;
    }
    label.textContent = "nothing";
    showStartFailure(err.message);
    return;
  }

  const openFile = async (sample: Sample): Promise<void> => {
    // The benchmark driver times a schedule from here to the ready frame (demos/bench/side.ts).
    performance.mark("data-start");
    return chrome.open(await fetchSample(sample), { expectedNotes: sample.expectedNotes ?? [] });
  };
  const openOwnFile = (): void => find<HTMLInputElement>("open-input").click();
  const actions = { openFile, openBuiltIn: reloadOnSample, openOwnFile };
  menuButton = find<HTMLButtonElement>("open-button");
  bindMenu = installMenu({ button: menuButton, list, status }, catalog, actions);
  placeNewSchedule(list, () => menuButton, gantt, status);
  placeCompare(list, () => menuButton, chrome.compare);
  placeNewTab(list, () => menuButton);
  placeSave(list, () => menuButton, () => gantt.download());
  placeCsvExport(list, () => menuButton, gantt);
  placeExcelExport(list, () => menuButton, gantt);
  placeCopyList(list, () => menuButton, gantt);
  placeImageExport(list, () => menuButton, gantt);
  placePrint(list, () => menuButton, gantt);
  placeLayouts(list, () => menuButton, gantt, status);
  placeComingSoon(list);
  if (picked !== undefined && picked.file !== null) await choose(picked, actions, status);
  // Last: a copy kept from the last visit is offered over whatever the page opened, and edits from here on are kept.
  void installAutosave(gantt, status).catch((err: unknown) => console.error("hosted: autosave failed", err));
}

main().catch((err: unknown) => {
  console.error(err);
  showStartFailure(err instanceof Error ? err.message : String(err));
});
