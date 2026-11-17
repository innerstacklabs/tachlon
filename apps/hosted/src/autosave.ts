// The hosted page's copy of the open schedule (gantt#2077): after each edit it writes `Gantt.toDocument()` to IndexedDB, and when the page opens again it
// asks once whether to bring that schedule back. The page does this, never `packages/gantt/`, which ships to npm and must not write a customer app's storage.
import type { Gantt } from "gantt-core";
import { say } from "./samples";

const DB = "gantt-autosave";
const STORE = "copy";
const KEY = "schedule";
/** The page writes this long after the last edit, so a drag or a run of typing is one write. */
const QUIET_MS = 500;

function settle<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

/** One request on the copy's store. The connection closes after it, so a second tab is never held up by this one. */
async function onStore<T>(mode: IDBTransactionMode, act: (store: IDBObjectStore) => IDBRequest<T>): Promise<T | undefined> {
  try {
    const opening = indexedDB.open(DB, 1);
    opening.onupgradeneeded = () => opening.result.createObjectStore(STORE);
    const db = await settle(opening);
    try {
      return await settle(act(db.transaction(STORE, mode).objectStore(STORE)));
    } finally {
      db.close();
    }
  } catch (err) {
    console.warn("hosted: the schedule copy is not kept in this browser", err); // storage blocked: the page works without it
    return undefined;
  }
}

const read = (): Promise<unknown> => onStore("readonly", (store) => store.get(KEY));
const write = (document: unknown): Promise<unknown> => onStore("readwrite", (store) => store.put(document, KEY));
const clear = (): Promise<unknown> => onStore("readwrite", (store) => store.delete(KEY));

/** Ask whether to bring the copy back; true when the copy is held. Only Cancel drops it: a load the chart refuses (a cell editor is open, or the copy is unreadable) keeps it, and the page then leaves it alone, so the next load asks again. */
async function offer(gantt: Gantt, status: HTMLElement, copy: unknown): Promise<boolean> {
  if (window.confirm("Bring back the schedule you were working on? OK brings it back; Cancel throws it away.")) {
    const result = await gantt.load({ document: copy, unsaved: true });
    if (!result.ok) say(status, `${result.error} The copy is held: this page will not overwrite it.`);
    return !result.ok;
  }
  await clear();
  return false;
}

/** Keep a copy of the schedule after each edit, offer the last one once, and drop it when a save or a new file leaves no unsaved changes. */
export async function installAutosave(gantt: Gantt, status: HTMLElement): Promise<void> {
  const copy = await read();
  const held = copy !== undefined && (await offer(gantt, status, copy));
  let timer: ReturnType<typeof setTimeout> | undefined;
  let unsaved = gantt.state().unsaved;
  gantt.on("edit", () => {
    clearTimeout(timer);
    timer = setTimeout(() => void (!held && gantt.state().unsaved && write(gantt.toDocument())), QUIET_MS);
  });
  gantt.on("change", (state) => {
    if (unsaved && !state.unsaved && !held) {
      clearTimeout(timer);
      void clear();
    }
    unsaved = state.unsaved;
  });
}
