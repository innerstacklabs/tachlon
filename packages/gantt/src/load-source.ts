// `Gantt.load`: read a schedule from a document or a file and swap it into the running chart.
import { last_panic, LoadedSchedule } from "./wasm/app.js";
import type { PublicApp } from "./api.js";
import { reason } from "./boot.js";
import { documentText, type LoadResult, type LoadSource } from "./document.js";
import type { OpenGuard } from "./open-guard.js";
import type { OpenedFile } from "./open-schedule.js";
import type { MountOptions } from "./index.js";

/** Today's date in the page's time zone as days since 1970-01-01, which `LoadedSchedule.blank` turns into the Monday on or before it. */
function today(): number {
  const now = new Date();
  return Math.round(Date.UTC(now.getFullYear(), now.getMonth(), now.getDate()) / 86_400_000);
}

/** Load `source` into `app`; `adopt` resets the page for the new schedule and `rename` sets the name Save uses. Never throws. */
export async function loadSource(
  app: PublicApp,
  source: LoadSource,
  adopt: (file: OpenedFile, lines: string[], ms: number, expected: readonly string[], fileName: string | null, unsaved?: boolean) => void,
  rename: (name: string) => void,
  guard: OpenGuard,
  capabilities: () => MountOptions["capabilities"],
): Promise<LoadResult> {
  const busy = guard.busy();
  if (busy !== null) return { ok: false, error: guard.say("not-loaded", { why: busy }) };
  const turn = guard.take();
  const started = performance.now();
  try {
    let loaded: LoadedSchedule;
    let fileName: string;
    let size: number;
    if ("blank" in source) {
      loaded = LoadedSchedule.blank(today());
      fileName = "";
      size = 0;
    } else if ("document" in source) {
      const text = JSON.stringify(source.document);
      loaded = LoadedSchedule.parse_document(text);
      const own = (source.document as { name?: unknown } | null)?.name;
      fileName = typeof own === "string" ? own : "";
      size = text.length;
    } else {
      const bytes = new Uint8Array(await source.file.arrayBuffer());
      const text = await documentText(bytes);
      loaded = text === null ? LoadedSchedule.parse_bytes(bytes, "all") : LoadedSchedule.parse_document(text);
      fileName = source.file.name;
      size = bytes.length;
    }
    const profile = capabilities();
    if (profile !== undefined) loaded.conform(JSON.stringify(profile));
    const format = loaded.format();
    const lines = loaded.unsupported() as string[];
    // The read took time: a newer open or load may have started, and an edit or a drag may have begun.
    const stop = guard.stale(turn) ? guard.say("newer-file") : guard.refusal(turn);
    if (stop !== null) {
      loaded.free(); // `App.open` takes the schedule; a swap that does not happen must release it.
      return { ok: false, error: guard.say("not-loaded", { why: stop }) };
    }
    app.open(loaded);
    adopt({ name: fileName, bytes: size, format, tasks: app.task_count() }, lines, performance.now() - started, [], "file" in source ? fileName : null, "unsaved" in source && source.unsaved === true);
    // A document keeps its own name, dots and all, and a blank schedule is `untitled`; a file is named by its file name without the extension.
    if (!("file" in source)) rename(fileName === "" ? "untitled" : fileName);
    return { ok: true };
  } catch (err) {
    return { ok: false, error: last_panic() ?? reason(err) };
  }
}
