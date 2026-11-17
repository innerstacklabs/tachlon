// The hosted app's "Open a sample" menu and `?sample=<id>` (gs#511 S6a). `scripts/build-samples.mjs` writes the
// catalog, `samples/samples.json`, and one gzip file per sample next to it; this module reads the catalog, lists it,
// and turns a chosen sample into a `File` for the chart's own `open`, so the open timings in the status line are the
// real ones.

/** One row of `samples.json`, as `build-samples.mjs` writes it. */
export interface Sample {
  id: string;
  label: string;
  /** "xer" for a generated XER file, "mspdi" for the MS Project XML file, "demo" for a schedule the chart builds itself. `scripts/build-samples.mjs` is type-checked against this union (gs#621), so a member it writes cannot be removed here unseen. */
  format: "xer" | "mspdi" | "demo";
  /** The file's name under `samples/` (gzip), or null for a built-in schedule. */
  file: string | null;
  /** Task rows in the file; what the menu shows. */
  activities: number;
  /** What the chart's `#row-count` shows once the sample is open: every row of the file, the activities plus its summary rows (WBS for an XER, summary tasks for an MSPDI); a built-in schedule has no summary rows. */
  rows: number;
  /** Size of the gzip file served; 0 for a built-in schedule. */
  bytes: number;
  sha256: string | null;
  /** One line saying what the sample is for; the menu shows it under the label. Absent for a catalog written before it existed. */
  description?: string;
  /**
   * The beginnings of the import notes this generated file reports by construction (a table the reader skips, an option it does not apply). The chart leaves a line that starts with one out of its toolbar notice. Absent for a sample with none; no line about the file's own dates is ever listed here.
   */
  expectedNotes?: string[];
}

/** The catalog, or why there is none. */
export type Catalog = { samples: Sample[] } | { error: string };

const BASE = "samples/";

/** What every failure sentence ends with: the person can still open a file of their own. */
const STILL_WORKS = "Open a file… still works.";

/** The cause of a failed fetch in words: no connection, a file that is not there (with the status), or a server error. */
function causeOf(status: number | null, what: string): string {
  if (status === null) return `there is no connection, so ${what} could not be fetched`;
  if (status === 404 || status === 410) return `${what} is missing on the server (HTTP ${status})`;
  if (status >= 500) return `the server had an error fetching ${what} (HTTP ${status})`;
  return `the server refused ${what} (HTTP ${status})`;
}

/** A fetch that failed, already worded for the status line: the cause, then that Open a file… still works. */
export class SampleFetchError extends Error {}

function failure(status: number | null, what: string, startsSentence = true): string {
  const cause = causeOf(status, what);
  return `${startsSentence ? cause.charAt(0).toUpperCase() : cause.charAt(0)}${cause.slice(1)}. ${STILL_WORKS}`;
}

/** Read `samples.json`. A build without `build-samples.mjs` has none; the answer says so instead of throwing. */
export async function loadCatalog(): Promise<Catalog> {
  try {
    const response = await fetch(`${BASE}samples.json`);
    if (!response.ok) return { error: failure(response.status, "the sample list") };
    const body = (await response.json()) as { samples?: Sample[] };
    if (!Array.isArray(body.samples) || body.samples.length === 0) return { error: `The sample list is empty. ${STILL_WORKS}` };
    return { samples: body.samples };
  } catch (err) {
    // fetch() rejects with a TypeError when nothing was received; a body that is not JSON rejects with a SyntaxError.
    if (err instanceof SyntaxError) return { error: `The sample list could not be read (it is not valid JSON). ${STILL_WORKS}` };
    return { error: failure(null, "the sample list") };
  }
}

/** The gzip magic number: a server that already undid the gzip hands over the bytes without it. */
function isGzip(bytes: Uint8Array): boolean {
  return bytes.length >= 2 && bytes[0] === 0x1f && bytes[1] === 0x8b;
}

/** How long a sample's download may take before it is given up, in milliseconds. The limit covers the fetch and reading the body, not the chart's own open. */
export const FETCH_LIMIT_MS = 30_000;

/** A download that ran past `FETCH_LIMIT_MS`. */
export class SampleTimeout extends Error {}

/** The download that is still current: a newer pick or a newer `fetchSample` aborts it, and its `fetchSample` stops. */
let current: AbortController | null = null;

/** Counts the picks `choose` has seen, so an older pick can tell a newer one has replaced it. */
let latestPick = 0;

/** The download `prefetchSample` started, for `fetchSample` to take over when it asks for the same sample. */
let early: { id: string; pick: AbortController; response: Promise<Response> } | null = null;

/** Start a file sample's download now, before the chart is up (gs#1336); a `fetchSample` of the same sample takes it over, and a built-in schedule has nothing to fetch. */
export function prefetchSample(sample: Sample): void {
  if (sample.file === null) return;
  const pick = new AbortController();
  const response = fetch(`${BASE}${sample.file}`, { signal: pick.signal });
  response.catch(() => undefined);
  early = { id: sample.id, pick, response };
}

/**
 * Fetch a file sample, undo its gzip in the browser, and make the `File` the chart opens. It stops, and never returns a file, when a
 * newer pick has replaced this one (the fetch is aborted) or when the download passes `FETCH_LIMIT_MS` (it throws `SampleTimeout`).
 */
export async function fetchSample(sample: Sample): Promise<File> {
  if (sample.file === null) throw new Error(`${sample.id} is built in and has no file`);
  // Each download owns a fresh controller, stored so any later pick or download cancels it; an old, already aborted one is never reused.
  current?.abort();
  const taken = early?.id === sample.id ? early : null;
  if (early !== null && taken === null) early.pick.abort();
  early = null;
  const pick = taken?.pick ?? new AbortController();
  current = pick;
  const { signal } = pick;
  const what = `the sample "${sample.label}"`;
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    pick.abort();
  }, FETCH_LIMIT_MS);
  try {
    let response: Response;
    try {
      response = await (taken?.response ?? fetch(`${BASE}${sample.file}`, { signal }));
    } catch (err) {
      if (signal.aborted) throw err;
      throw new SampleFetchError(failure(null, what, false));
    }
    if (!response.ok) throw new SampleFetchError(failure(response.status, what, false));
    let buffer: ArrayBuffer;
    try {
      buffer = await response.arrayBuffer();
    } catch (err) {
      if (signal.aborted) throw err;
      throw new SampleFetchError(failure(null, what, false));
    }
    signal.throwIfAborted();
    const packed = new Uint8Array(buffer);
    if (isGzip(packed) && typeof DecompressionStream === "undefined") throw new SampleFetchError("This browser cannot unpack the sample; use Open a file…");
    const bytes = isGzip(packed)
      ? new Uint8Array(await new Response(new Blob([packed]).stream().pipeThrough(new DecompressionStream("gzip"))).arrayBuffer())
      : packed;
    signal.throwIfAborted();
    return new File([bytes], sample.file.replace(/\.gz$/, ""), { type: "application/octet-stream" });
  } catch (err) {
    if (timedOut) throw new SampleTimeout(sample.id);
    throw err;
  } finally {
    clearTimeout(timer);
  }
}

const COUNT = new Intl.NumberFormat("en-US");

/** "3.2 MB", "323 KB": the size of the download. */
export function sizeText(bytes: number): string {
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1000))} KB`;
}

/** What the menu shows under a sample's label. */
export function metaText(sample: Sample): string {
  const activities = `${COUNT.format(sample.activities)} activities`;
  return sample.file === null ? `${activities} · built in, no download` : `${activities} · ${sizeText(sample.bytes)} download`;
}

/** What the page does when the person picks a sample. */
export interface SampleActions {
  /** Open a file sample in the running chart. */
  openFile(sample: Sample): Promise<void>;
  /** Start the page again on a built-in schedule (the chart takes it only when it mounts). */
  openBuiltIn(sample: Sample): void;
  /** Choose a file of the person's own: the chart's own file chooser. */
  openOwnFile(): void;
}

/** The elements the menu drives: the chart's "Open…" button, and the list and status line from `index.html`. */
export interface MenuElements {
  button: HTMLButtonElement;
  list: HTMLElement;
  status: HTMLElement;
}

/** Show `text` (or nothing) beside the button, where a sample's progress or reason goes. */
export function say(status: HTMLElement, text: string): void {
  status.textContent = text;
}

/** Put `?sample=<id>` in the address bar, without a history entry, so a reload or a copied address returns to this sample. */
function addressNames(sample: Sample): void {
  const url = new URL(window.location.href);
  url.searchParams.set("sample", sample.id);
  window.history.replaceState(window.history.state, "", url);
}

/** Open one sample, reporting what happens beside the button. */
export async function choose(sample: Sample, actions: SampleActions, status: HTMLElement): Promise<void> {
  // A newer pick cancels the older download, so a late first fetch never opens after the second sample has.
  current?.abort();
  const mine = ++latestPick;
  if (sample.file === null) {
    say(status, `Opening ${sample.label}…`);
    actions.openBuiltIn(sample);
    return;
  }
  say(status, `Fetching ${sample.label}, ${sizeText(sample.bytes)}…`);
  try {
    await actions.openFile(sample);
    if (mine !== latestPick) return;
    addressNames(sample);
    say(status, "");
  } catch (err) {
    if (mine !== latestPick) return;
    if (err instanceof SampleTimeout) {
      say(status, `The sample "${sample.label}" did not arrive within ${FETCH_LIMIT_MS / 1000} seconds, so it was not opened. ${STILL_WORKS}`);
      return;
    }
    // Every failure keeps the "<id> did not open: " lead; a fetch failure's reason is already worded, with the Open a file… line at its end.
    say(status, `${sample.id} did not open: ${err instanceof Error ? err.message : String(err)}`);
  }
}

/** One row of the menu: a label and, under it, what opening it costs. */
function menuItem(label: string, meta: string, description?: string): HTMLButtonElement {
  const item = document.createElement("button");
  item.type = "button";
  item.setAttribute("role", "menuitem");
  const name = document.createElement("span");
  name.className = "sample-label";
  name.textContent = label;
  const detail = document.createElement("span");
  detail.className = "sample-meta";
  detail.textContent = meta;
  item.append(name);
  if (description !== undefined) {
    const purpose = document.createElement("span");
    purpose.className = "sample-meta";
    purpose.textContent = description;
    item.append(purpose);
  }
  item.append(detail);
  return item;
}

/**
 * Build the menu from the catalog and make the chart's "Open…" button its button: a click on it opens "Open a file…" first, then the
 * samples. With no catalog the button is left alone (it opens the file chooser, as in the chart) and the status line says why. Returns the
 * call that makes a new "Open…" button the menu's, for a chart that started again in its box.
 */
export function installMenu(elements: MenuElements, catalog: Catalog, actions: SampleActions): (button: HTMLElement) => void {
  const { list, status } = elements;
  let button: HTMLElement = elements.button;
  if ("error" in catalog) {
    say(status, catalog.error);
    return (next) => {
      button = next;
    };
  }
  // The menu's button is the chart's "Open…" button; a chart that started again (the switch to WebGL2) has a new one, and `bind` takes it.
  const bind = (next: HTMLElement): void => {
    button = next;
    button.setAttribute("aria-haspopup", "menu");
    button.setAttribute("aria-expanded", "false");
    button.setAttribute("aria-controls", list.id);
  };
  bind(button);
  const close = (): void => {
    list.hidden = true;
    button.setAttribute("aria-expanded", "false");
  };
  const own = menuItem("Open a file…", "a P6 XER or MS Project XML file");
  own.dataset.openFile = "true";
  own.addEventListener("click", () => {
    close();
    actions.openOwnFile();
  });
  list.append(own);
  for (const sample of catalog.samples) {
    const item = menuItem(sample.label, metaText(sample), sample.description);
    item.dataset.sampleId = sample.id;
    item.addEventListener("click", () => {
      close();
      void choose(sample, actions, status);
    });
    list.append(item);
  }
  // The chart's own click handler opens the file chooser; this one runs first (the capture phase of the document) and keeps the click from reaching it.
  document.addEventListener(
    "click",
    (event) => {
      if (!(event.target instanceof Node) || !button.contains(event.target)) return;
      event.stopPropagation();
      const open = list.hidden;
      list.hidden = !open;
      button.setAttribute("aria-expanded", String(open));
    },
    true,
  );
  document.addEventListener("pointerdown", (event) => {
    if (!list.hidden && event.target instanceof Node && !list.contains(event.target) && !button.contains(event.target)) close();
  });
  document.addEventListener("keydown", (event) => {
    if (list.hidden) return;
    if (event.key === "Escape") {
      close();
      button.focus();
      return;
    }
    const inList = document.activeElement instanceof Node && list.contains(document.activeElement);
    if (event.key === "Tab" && inList) {
      // Tab out of the menu closes it; focus goes to the button first so the Tab carries on from there, not from a hidden item.
      close();
      button.focus();
      return;
    }
    if (!inList && document.activeElement !== button) return;
    // Every enabled item takes part, whatever its element: the buttons, the new-tab link and the export and print divs. A disabled one has no focus.
    const items = Array.from(list.querySelectorAll<HTMLElement>('[role="menuitem"]:not([aria-disabled="true"])'));
    const at = items.findIndex((item) => item === document.activeElement);
    let next: number;
    if (event.key === "ArrowDown") next = at < 0 ? 0 : (at + 1) % items.length;
    else if (event.key === "ArrowUp") next = at < 0 ? items.length - 1 : (at - 1 + items.length) % items.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = items.length - 1;
    else return;
    event.preventDefault();
    items[next]?.focus();
  });
  return bind;
}

/** The sample a `?sample=` value names, or the reason the page opens nothing. */
export function lookup(catalog: Catalog, wanted: string): { sample: Sample } | { reason: string } {
  if ("error" in catalog) return { reason: catalog.error };
  const sample = catalog.samples.find((candidate) => candidate.id === wanted);
  if (sample !== undefined) return { sample };
  const known = catalog.samples.map((candidate) => candidate.id).join(", ");
  return { reason: `There is no sample "${wanted}" (the samples are ${known}); nothing was opened.` };
}
