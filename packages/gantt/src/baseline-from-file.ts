// Last month's file as the comparison (gs#703): `Commands.setBaselineFromFile` reads a second schedule file, parses it, and hands it to
// `App.set_baseline_from`, which matches its tasks to the open schedule's by ID and makes them the baseline in one undo step.
import type { PublicApp } from "./api.js";
import { type CompareResult, type Refusal } from "./edit-result.js";
import { MAX_OPEN_BYTES } from "./open-schedule.js";
import { LoadedSchedule } from "./wasm/app.js";

const refuse = (message: string): Refusal => Object.freeze({ ok: false, reason: "other", message });

/** Read `file` and set it as the baseline. Any file the reader or the match refuses (empty, too big, another format, another project) is a refusal that changes nothing. */
export async function setBaselineFromFile(app: PublicApp, file: File, reason: (err: unknown) => string): Promise<CompareResult> {
  if (file.size === 0 || file.size > MAX_OPEN_BYTES) return refuse(`${file.name} is ${file.size === 0 ? "empty" : "too big to compare"}.`);
  let loaded: LoadedSchedule | undefined;
  try {
    loaded = LoadedSchedule.parse_bytes(new Uint8Array(await file.arrayBuffer()), undefined);
    const lists = JSON.parse(app.set_baseline_from(loaded)) as Record<"file_only" | "here_only" | "file_only_links" | "here_only_links", string[]>;
    return Object.freeze({
      ok: true,
      fileOnly: Object.freeze(lists.file_only),
      hereOnly: Object.freeze(lists.here_only),
      fileOnlyLinks: Object.freeze(lists.file_only_links),
      hereOnlyLinks: Object.freeze(lists.here_only_links),
    });
  } catch (err) {
    return refuse(`could not compare with ${file.name}: ${reason(err)}`);
  } finally {
    loaded?.free();
  }
}
