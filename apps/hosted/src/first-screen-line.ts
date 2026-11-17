// The line on the first screen (gantt#1280): what the chart is and how to open your own schedule. The page loads this module
// itself; it puts the line from `index.html`'s template on the page, over the chart's bottom edge, unless the viewer dismissed it
// before. It goes when the viewer opens their own file (the chooser or a drop) or starts a new schedule, and it never shows with the start-failure block.
const KEY = "gantt-first-screen-line-dismissed";
// The line's height, set while it is open: index.html borders the chart's bottom by it, so the line covers neither the legend row nor the baseline note.
const HEIGHT = "--first-screen-line-h";

function dismissed(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false; // storage blocked: the line shows each time, and a dismissal lasts until the page reloads
  }
}

const template = document.getElementById("first-screen-line-template");
if (template instanceof HTMLTemplateElement && !dismissed()) {
  const line = template.content.firstElementChild!.cloneNode(true) as HTMLElement;
  const stop = new AbortController();
  const remove = (): void => {
    line.remove();
    stop.abort();
    watcher.disconnect();
    reserve.disconnect();
    document.documentElement.style.removeProperty(HEIGHT);
  };
  line.querySelector("button")!.addEventListener("click", () => {
    remove();
    try {
      localStorage.setItem(KEY, "1");
    } catch {
      // not kept
    }
  });
  // Opening a file is the chart chrome's: the chooser's change (`#open-input`) and a drop on the page. Both bubble to the document.
  document.addEventListener("change", (e) => (e.target as Element | null)?.id === "open-input" && remove(), { capture: true, signal: stop.signal });
  document.addEventListener("drop", remove, { capture: true, signal: stop.signal });
  // main.ts fires this once the file menu's "New schedule" has started the empty schedule: the sample the line describes is gone.
  document.addEventListener("gantt:new-schedule", remove, { signal: stop.signal });
  // main.ts puts #start-failure in the chart's root when the engine cannot start.
  const watcher = new MutationObserver(() => document.getElementById("start-failure") && remove());
  watcher.observe(document.body, { childList: true, subtree: true });
  document.body.append(line);
  const reserve = new ResizeObserver(() => document.documentElement.style.setProperty(HEIGHT, `${Math.ceil(line.getBoundingClientRect().height)}px`));
  reserve.observe(line);
}
