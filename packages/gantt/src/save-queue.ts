// The host's save queue (decision 0022, rule 3.3): each commit the person makes goes to the host's `save` function as a change set, one call at a time, in `seq`
// order. A `SaveRefused` puts that commit and every commit behind it back through `App.rollback_to`; any other throw leaves the commit unsaved and stops the queue.
import type { Commit } from "./events.js";
import type { ChangeSet, Handlers, Save, SaveError } from "./gantt-types.js";
import { callSave } from "./save-handlers.ts"; // `.ts`, so the Node tier can load this file

/** Throw this from `save` when the server judged the change and said no; `message` is the line the person sees. */
export class SaveRefused extends Error {
  readonly key: string | undefined;
  readonly field: string | undefined;
  constructor(refusal: { message: string; key?: string; field?: string }) {
    super(refusal.message);
    this.name = "SaveRefused";
    this.key = refusal.key;
    this.field = refusal.field;
  }
}

/** The two commands of the app the queue calls. */
export interface SaveApp {
  acknowledge(seq: number): void;
  rollback_to(seq: number): string;
}

/** How long a put-back waits when the app is busy (a cell or a bar is held), in milliseconds. */
const BUSY_WAIT = 50;

const LIVE = Symbol.for("gantt.liveSave");

/** What a wrapper hands `mount` as `save`: the queue asks `get` for the host's current function when it sends a commit, and with none it writes no change set and acknowledges nothing. A symbol key, so the entry bundles agree on it. */
export function liveSave(get: () => Save | Handlers | undefined): Save {
  return Object.assign((changeSet: ChangeSet) => callSave(get()!, changeSet), { [LIVE]: get });
}

/** A commit that is not saved yet, its change set (written at the first call) and how many times `save` has run for it. */
interface Waiting { commit: Commit; set?: ChangeSet; calls: number }

export class SaveQueue {
  private readonly app: SaveApp;
  private readonly save: Save | Handlers;
  private readonly failed: (error: SaveError) => void;
  private readonly waiting: Waiting[] = [];
  private running = false;
  private stopped = false;
  private broken = false;
  // The seq of the last rollback commit: a commit before it that is read late was taken back with the rest.
  private floor = 0;

  /** `failed` is told of each put-back once, after it. */
  constructor(app: SaveApp, save: Save | Handlers, failed: (error: SaveError) => void) {
    this.app = app;
    this.save = save;
    this.failed = failed;
  }

  /** Queue the commits an `edit` event carried and start sending. A rollback commit and a commit with no change are not sent. */
  add(commits: readonly Commit[]): void {
    if (this.stopped) return;
    for (const commit of commits) if (commit.source !== "rollback" && commit.changes.length > 0 && commit.seq >= this.floor) this.waiting.push({ commit, calls: 0 });
    this.go();
  }

  /** Call the commit a throw left unsaved again, with the same change set. */
  resume(): void {
    this.broken = false;
    this.go();
  }

  /** The chart is going down: a call that is running finishes and its result is ignored, and no other call is made. */
  stop(): void {
    this.stopped = true;
    this.waiting.length = 0;
  }

  private go(): void {
    void this.pump().catch((err: unknown) => {
      this.broken = true;
      console.error("the save queue stopped", err);
    });
  }

  private async pump(): Promise<void> {
    if (this.running) return;
    this.running = true;
    try {
      while (!this.stopped && !this.broken && this.waiting.length > 0) {
        const head = this.waiting[0]!;
        const live = (this.save as { [LIVE]?: () => Save | Handlers | undefined })[LIVE];
        const save = live === undefined ? this.save : live();
        if (save === undefined) {
          this.waiting.shift();
          continue;
        }
        head.set ??= write(head.commit);
        head.calls++;
        try {
          await callSave(save, head.set);
        } catch (err) {
          if (this.stopped) return;
          if (!(err instanceof SaveRefused)) {
            this.broken = true;
            console.error("a save threw, so the chart stopped saving", err);
            return;
          }
          await this.putBack(err, head);
          continue;
        }
        if (this.stopped) return;
        this.app.acknowledge(head.commit.seq);
        this.waiting.shift();
      }
    } finally {
      this.running = false;
    }
  }

  /** Take `head` and every commit behind it back, then tell `failed`. A commit the app no longer holds sets `reload`. */
  private async putBack(refusal: SaveRefused, head: Waiting): Promise<void> {
    const rollback = (): { ok: boolean; reason?: string; seq: number } => JSON.parse(this.app.rollback_to(head.commit.seq)) as { ok: boolean; reason?: string; seq: number };
    let answer = rollback();
    while (!this.stopped && !answer.ok && answer.reason === "busy") {
      await new Promise((resolve) => setTimeout(resolve, BUSY_WAIT));
      if (!this.stopped) answer = rollback();
    }
    if (this.stopped) return;
    const undone = answer.ok ? this.waiting.length : 0;
    if (answer.ok) this.floor = answer.seq;
    this.waiting.length = 0;
    this.failed({
      source: "save", message: refusal.message, fatal: false, reason: "refused", calls: head.calls,
      ...(refusal.key === undefined ? {} : { key: refusal.key }), ...(refusal.field === undefined ? {} : { field: refusal.field }),
      seq: head.commit.seq, undone, reload: !answer.ok,
    });
  }
}

/** The change set of a commit: a new `id`, and `once` on each change. The session slots (`task`, `link`) mean nothing after a reload, so they stay out of what the host stores. */
function write(commit: Commit): ChangeSet {
  const id = uuid();
  const changes = commit.changes.map(({ task, link, ...change }, n) => ({ ...change, once: `${id}:${n}` }));
  return { id, seq: commit.seq, source: commit.source, changes };
}

/** A v4 UUID. A page that is not a secure context has no `crypto.randomUUID`, so the id comes from `crypto.getRandomValues`. */
function uuid(): string {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  const b = crypto.getRandomValues(new Uint8Array(16));
  b[6] = (b[6]! & 0x0f) | 0x40;
  b[8] = (b[8]! & 0x3f) | 0x80;
  const hex = Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
