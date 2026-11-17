// The schedule document's file side (gs#740): the wasm module writes and reads the JSON text, and the browser gzips it
// (`CompressionStream`) and gunzips it (`DecompressionStream`). Nothing here makes a network request.

/** The two bytes every gzip stream starts with. */
const GZIP_MAGIC = [0x1f, 0x8b];
/** A document's text starts with its `format` key: `"gantt"`. */
const DOCUMENT_START = /^\s*\{\s*"format"\s*:\s*"(gantt)"/;

/** The bytes of `source` run through `transform` (a compression or decompression stream). */
async function through(source: BlobPart, transform: GenericTransformStream): Promise<Uint8Array<ArrayBuffer>> {
  const stream = new Blob([source]).stream().pipeThrough(transform);
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

/** `text` as a gzip file's bytes. */
export function gzip(text: string): Promise<Uint8Array<ArrayBuffer>> {
  return through(text, new CompressionStream("gzip"));
}

/**
 * The document text `bytes` holds, or `null` when `bytes` is not a gzip file whose JSON says `"format":"gantt"` (a P6
 * or MS Project file, or any other gzip). A gzip file that cannot be read rejects.
 */
export async function documentText(bytes: Uint8Array): Promise<string | null> {
  if (bytes.length < 2 || bytes[0] !== GZIP_MAGIC[0] || bytes[1] !== GZIP_MAGIC[1]) return null;
  const text = new TextDecoder().decode(await through(new Uint8Array(bytes), new DecompressionStream("gzip")));
  return DOCUMENT_START.test(text) ? text : null;
}

/** Hand `bytes` to the browser as a download called `fileName`, of media type `type` (a gzip file unless said). Returns when the download has started. */
export function download(fileName: string, bytes: Uint8Array<ArrayBuffer>, type = "application/gzip"): void {
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  link.click();
  // The browser has the blob by the time a task has run; releasing it earlier can cancel the download.
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/** The schedule's name for a file: its name without the extension, `untitled` when nothing is left. */
export function documentName(fileName: string | undefined): string {
  const name = (fileName ?? "").replace(/\.[^./\\]*$/, "");
  return name === "" ? "untitled" : name;
}

/** What `Gantt.load` takes: a document (the JSON of `toDocument`; `unsaved: true` shows it as unsaved, for a copy of work no save holds), a file, or `{ blank: true }`, a schedule with no tasks that starts on this week's Monday. */
export type LoadSource = { document: unknown; unsaved?: boolean } | { file: File } | { blank: true };

/** What `Gantt.load` resolves with. A bad source gives `ok: false` and a line for a person; the schedule is as it was. */
export type LoadResult = { ok: true } | { ok: false; error: string };

/** What an export returns: the file's bytes and one line for each thing it cannot carry. */
export interface ExportResult {
  bytes: Uint8Array<ArrayBuffer>;
  leftOut: string[];
}

/** The `{ bytes, left_out }` object of `App.export_xer` and `App.export_mspdi`, as an `ExportResult`. */
export function toExportResult(raw: unknown): ExportResult {
  const { bytes, left_out } = raw as { bytes: Uint8Array<ArrayBuffer>; left_out: string[] };
  return { bytes, leftOut: left_out };
}

/**
 * The schedule-quality counts `Gantt.checks` returns. Each is a count of tasks or links, and none says pass or fail. `invalidDates` is
 * `null` when the schedule has no data date, and `missingResources` when the file names no resource, because a 0 would read as a check
 * that ran and found nothing.
 */
export interface ChecksReport {
  /** Unfinished tasks with no predecessor or no successor; summary tasks are not counted. */
  missingLogic: number;
  /** Unfinished tasks with no predecessor; summary tasks are not counted. */
  openStarts: number;
  /** Unfinished tasks with no successor; summary tasks are not counted. */
  openEnds: number;
  /** Links with a negative lag. */
  leads: number;
  /** Links with a positive lag. */
  lags: number;
  /** All links. */
  links: number;
  /** Links of type finish to start. */
  finishToStart: number;
  /** Unfinished tasks with more than 44 working days of total float. */
  highFloat: number;
  /** Tasks longer than 44 working days. */
  highDuration: number;
  /** Unfinished tasks a fixed date holds: a start-no-later-than, a finish-no-later-than or a pin they still obey, summary tasks not counted. */
  hardConstraints: number;
  /** Unfinished tasks with a total float below 0, summary tasks not counted. */
  negativeFloat: number;
  /** Tasks with an actual start or finish after the data date; `null` without a data date. */
  invalidDates: number | null;
  /** Unfinished tasks with a duration and no resource, summary tasks not counted; `null` when the file names no resource. */
  missingResources: number | null;
  /** Tasks the baseline had finishing by the data date that finished later or have not finished; 0 with no baseline or no data date. */
  missed: number;
  /** Tasks finished over tasks the baseline had finishing by the data date, 0 to 1; `null` with no baseline, no data date, or none due. */
  executionIndex: number | null;
}

/** The JSON text of `App.checks`, as a `ChecksReport`. */
export function toChecksReport(json: string): ChecksReport {
  const raw = JSON.parse(json) as Record<string, number | null>;
  return {
    missingLogic: raw.missing_logic!,
    openStarts: raw.open_starts!,
    openEnds: raw.open_ends!,
    leads: raw.leads!,
    lags: raw.lags!,
    links: raw.links!,
    finishToStart: raw.finish_to_start!,
    highFloat: raw.high_float!,
    highDuration: raw.high_duration!,
    hardConstraints: raw.hard_constraints!,
    negativeFloat: raw.negative_float!,
    invalidDates: raw.invalid_dates!,
    missingResources: raw.missing_resources!,
    missed: raw.missed!,
    executionIndex: raw.execution_index!,
  };
}
