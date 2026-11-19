// The events a host reacts to with `Gantt.on` (gs#1086). They are read from the `Status` the loop already takes
// once per frame: `watch` compares it with the one before and fires only on a change, so an idle chart fires nothing.
import type { Status } from "./api.js";
import { foldChanges, type FieldChange } from "./change-set.js";
import type { SaveError } from "./gantt-types.js";
import type { PickerProject } from "./open-schedule.js";
import type { ViewState } from "./view-state.js";

/** Where a commit came from (decision 0019, rule 1). */
export type CommitSource = "typed" | "drag" | "dialog" | "paste" | "proposal" | "undo" | "redo" | "rollback";

/**
 * One task, link or the schedule a commit changed, as one record (decision 0022, section 2): `record` is `"task"`, `"link"` or `"schedule"`, `op`
 * is `"update"`, `"create"` or `"delete"`. `key` is the host's key or `null`; `task` or `link` is the session slot, valid until the next load, open
 * or remount, so never stored. An update's `set` holds the members that changed with their new values and `was` the same members as they were; a
 * create's `set` is the whole record; a delete's `was` is the whole record. `cascade`: a link delete that follows from a task delete in the same commit.
 */
export interface Change {
  record: "task" | "link" | "schedule";
  op: "update" | "create" | "delete";
  key: string | null;
  task?: number;
  link?: number;
  set?: Record<string, unknown>;
  was?: Record<string, unknown>;
  cascade?: true;
}

/** One undo step: `seq` counts from 1 in a session and is never reused (decision 0019, rule 1). */
export interface Commit {
  seq: number;
  source: CommitSource;
  changes: Change[];
}

/** What each event carries. */
export interface GanttEvents {
  /** The selected task changed to `task`, whose key is `key` (`null` when the host gave it none). A row or a bar was clicked, or a key moved the selection. Clearing the selection fires nothing. */
  select: { task: number; key: string | null };
  /**
   * The schedule changed by an edit, an undo or a redo; `revision` is `Status.revision` after it. `commits` are the undo steps made since the
   * last `edit`, oldest first, none while a bar is held: the release fires an `edit` with the drag's one commit, even when its revision is the last one's. `seq` is the latest commit's.
   * `snapshot`: commits were lost (more than 1,000 unread, or the switch to WebGL2), so the host saves `snapshot()` instead.
   */
  edit: { revision: number; commits: Commit[]; snapshot: boolean; seq: number };
  /** A bar drag was released and moved `task`, whose key is `key` and whose early start is now `start` (`YYYY-MM-DDTHH:mm`). */
  move: { task: number; key: string | null; start: string };
  /** A file was opened in place; `epoch` is `Status.epoch` after it. */
  open: { epoch: number };
  /** The first frame was drawn. A handler added later is still called, once. */
  load: Record<string, never>;
  /** The view state a toolbar shows (`Gantt.state()`) changed: a filter, the lookahead window, the row or task count, the zoom level, the selection, the move report. At most once a frame. */
  change: ViewState;
  /**
   * A file could not be opened, a frame failed, or WebGPU failed and the chart started again on WebGL2 in its root (`backend`, once, with
   * the reason), or the page's `beforeEdit` function threw or answered with a promise (`beforeEdit`, not fatal, the change refused); `message` is the line a person sees. `fatal`: the chart has stopped (a panic, frames that keep failing, or a switch to
   * WebGL2 that could not start), and only a reload of the page brings it back; the host decides whether to do that.
   * A save the host refused puts the edits back and fires `error` once with the source `save` and a `SaveError`.
   */
  error: { source: ErrorSource; message: string; fatal: boolean; projects?: readonly PickerProject[] } | (SaveError & { projects?: undefined });
  /** The chart's find key (Ctrl+F, Cmd+F) was pressed while focus was in it: a host puts focus in its find box. With a handler the browser's own find does not open. */
  find: Record<string, never>;
  /** The chart's status key (`s`) was pressed while focus was in it: a host shows or hides its status line. */
  "toggle-status": Record<string, never>;
  /** The chart's open key (Ctrl+O, Cmd+O) was pressed while focus was in it: a host shows its file chooser. With no handler the key is left to the browser. */
  "choose-file": Record<string, never>;
  /** The chart's keys key (`?`) was pressed while focus was in it: a host shows or hides its list of keys. */
  "toggle-keys": Record<string, never>;
}

/** What an `error` event is about. */
export type ErrorSource = "open" | "frame" | "backend" | "beforeEdit" | "save";

/** The events a key of the chart asks for: the host acts on them. */
export type KeyRequest = "find" | "toggle-status" | "choose-file" | "toggle-keys";

export type GanttEventName = keyof GanttEvents;
export type GanttHandler<K extends GanttEventName> = (detail: GanttEvents[K]) => void;

/**
 * `Gantt.on`: call `handler` when the chart `select`s a task, `edit`s the schedule, `move`s a bar by a drag, `open`s a file, `load`s (its
 * first frame), its view state `change`s or it reports an `error`, and when its find key asks for the host's `find` box or its status
 * key asks to `toggle-status`. Each fires once per change, read from the loop's `Status`, never per frame. Returns the function
 * that removes the handler. The detail of each is `GanttEvents`.
 */
export type On = <K extends GanttEventName>(name: K, handler: GanttHandler<K>) => () => void;

/** The member `Gantt` takes from here. */
export interface HasOn {
  on: On;
}

export interface Events {
  /** Call `handler` for each `name` event. Returns the function that removes it. */
  on: On;
  /** Fire the events `s` shows against the status before it. */
  watch(s: Status): void;
  /** Fire `error`; `fatal` when the chart has stopped. */
  fail(source: ErrorSource, message: string, fatal?: boolean, projects?: readonly PickerProject[]): void;
  /** Fire `error` for a save the host refused, after the chart put the edits back. */
  putBack(error: SaveError): void;
  /**
   * A new app drives the chart (the switch to WebGL2): the next status is compared with nothing, so its counters fire no event, and its
   * view state fires one `change`, since the new start's view is not the one the last `change` carried. The commits the old app held unread are
   * lost, so the next `edit` carries `snapshot: true`.
   */
  restart(): void;
  /** Fire a key's event. True when a handler heard it, so the key is taken from the browser. */
  request(name: KeyRequest): boolean;
}

/** What the events read from the app that drives the chart now. */
export interface EventReads {
  /** Turn a frame's `Status` into the view state; it returns the same object while nothing the state carries moved. */
  view(s: Status): ViewState;
  /** `App.take_commits`: the commits not yet read, as JSON. */
  commits(): string;
  /** `App.task_ref`: a task's key and early start, as JSON; undefined for a slot the app does not hold. */
  ref(task: number): string | undefined;
  /** Put the changes of a commit in the unit and date form the host mounted with (`changeForm`); this read of the commits may read the saved document again. */
  form(changes: readonly Change[]): Change[];
}

/** `App.take_commits` read as an `edit` carries it: each commit's changes folded to one per record, in the host's unit and date form. */
function parseCommits(json: string, form: EventReads["form"]): Omit<GanttEvents["edit"], "revision"> {
  const read = JSON.parse(json) as { commits: (Omit<Commit, "changes"> & { changes: FieldChange[] })[]; snapshot: boolean; seq: number };
  return { ...read, commits: read.commits.map((commit) => ({ ...commit, changes: form(foldChanges(commit.changes)) })) };
}

/** `reads` is the current start's: the events outlive a switch to WebGL2, which gives them a new app. */
export function createEvents(reads: () => EventReads): Events { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const handlers = new Map<GanttEventName, Set<(detail: never) => void>>();
  let loaded = false;
  let previous: Status | null = null;
  let dragFrom: number | null = null;
  // The latest commit's `seq`, and whether the commits a new app never held were lost: the next `edit` says so with `snapshot`.
  let seq = 0;
  let lost = false;
  // The view state the last `change` carried (or the first frame's).
  let shown: ViewState | null = null;

  // A host's handler must not stop the chart's loop: every call, late `load` included, goes through here.
  const call = (handler: (detail: never) => void, detail: unknown): void => {
    try {
      (handler as (detail: unknown) => void)(detail);
    } catch (err) {
      console.error("an event handler threw", err);
    }
  };
  const emit = <K extends GanttEventName>(name: K, detail: GanttEvents[K]): void => {
    for (const handler of [...(handlers.get(name) ?? [])]) call(handler, detail);
  };
  const ref = (task: number): { key: string | null; start: string } | undefined => {
    const json = reads().ref(task);
    return json === undefined ? undefined : (JSON.parse(json) as { key: string | null; start: string });
  };

  return {
    on(name, handler) {
      const set = handlers.get(name) ?? new Set();
      handlers.set(name, set);
      const h = handler as (detail: never) => void;
      set.add(h);
      if (name === "load" && loaded) queueMicrotask(() => { if (set.has(h)) call(h, {}); });
      return () => void set.delete(h);
    },
    // The loop calls this once a frame and it emits `change` at most once per call: that is the "at most once a frame".
    watch(s) {
      const before = previous;
      previous = s;
      const state = reads().view(s);
      if (!loaded) {
        loaded = true;
        shown = state;
        emit("load", {});
        return;
      }
      if (before === null) {
        // The first status after `restart`: nothing to compare the counters with, and a view state of a new start.
        if (state !== shown) {
          shown = state;
          emit("change", state);
        }
        return;
      }
      if (s.epoch !== before.epoch) {
        emit("open", { epoch: s.epoch });
        dragFrom = null;
      } else {
        const dragging = s.drag?.active === true;
        const released = !dragging && dragFrom !== null;
        if (s.selected !== null && s.selected.task !== before.selected?.task) {
          emit("select", { task: s.selected.task, key: ref(s.selected.task)?.key ?? null });
        }
        if (s.revision !== before.revision || released) {
          // A held bar makes no commit until its release, which can leave the revision where the last frame had it: it reads too.
          const read = dragging ? { commits: [], snapshot: false, seq } : parseCommits(reads().commits(), reads().form);
          seq = read.seq;
          const snapshot = read.snapshot || (!dragging && lost);
          if (!dragging) lost = false;
          if (s.revision !== before.revision || read.commits.length > 0 || snapshot) emit("edit", { revision: s.revision, commits: read.commits, snapshot, seq });
        }
        if (dragging && dragFrom === null) dragFrom = before.revision;
        if (!dragging && dragFrom !== null) {
          const drag = s.drag;
          // A drag that ends where it began leaves the revision as it was, and is not a move.
          const at = drag !== null && s.revision !== dragFrom ? ref(drag.task) : undefined;
          if (drag !== null && at !== undefined) emit("move", { task: drag.task, key: at.key, start: at.start });
          dragFrom = null;
        }
      }
      // `view` hands back the object it built last while nothing it carries moved, so a new object is a change.
      if (state !== shown) {
        shown = state;
        emit("change", state);
      }
    },
    fail(source, message, fatal = false, projects) {
      emit("error", projects === undefined ? { source, message, fatal } : { source, message, fatal, projects });
    },
    putBack(error) {
      emit("error", error);
    },
    restart() {
      previous = null;
      dragFrom = null;
      shown = null;
      lost = true;
    },
    request(name) {
      if ((handlers.get(name)?.size ?? 0) === 0) return false;
      emit(name, {});
      return true;
    },
  };
}
