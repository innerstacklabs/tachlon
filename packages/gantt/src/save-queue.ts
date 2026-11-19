// The host's save queue (decision 0022, rule 3.3): each commit the person makes goes to the host's `save` function as a change set, one call at a time, in `seq`
// order. A `SaveRefused` puts that commit and every commit behind it back through `App.rollback_to`; any other throw leaves the commit unsaved and stops the queue.
import type { Change, Commit } from "./events.js";
import type { ChangeSet, Handlers, Save, SaveError, SaveResult } from "./gantt-types.js";
import type { KeyNames } from "./save-handlers.js";
import { BadResult, callSave, restoredMismatch } from "./save-handlers.ts"; // `.ts`, so the Node tier can load this file

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

/** The function or object the host holds now: what `save` is, or what a wrapper's `liveSave` asks for. */
const current = (save: Save | Handlers): Save | Handlers | undefined => {
  const live = (save as { [LIVE]?: () => Save | Handlers | undefined })[LIVE];
  return live === undefined ? save : live();
};

/** Whether a host has a save now, for what only a host that saves has to keep (every row has a key). A wrapper with no `save` yet answers no. */
export const hasSave = (save: Save | Handlers | undefined): boolean => save !== undefined && current(save) !== undefined;

/** A commit that is not saved yet, its change set (written at the first call), how many times `save` has run for it, and why it cannot be sent (a key the app refused). */
interface Waiting { commit: Commit; set?: ChangeSet; calls: number; bad?: string }

/** How the queue gives a new row its key (decision 0022, rule 5): `mode` and `make` are the mount's `keys` and `newKey`, `names` the host's names for `key`, `assign` gives a row its key in the app and says why it refused, if it did. */
export interface KeyRules { mode: "chart" | "server"; make: () => string; names: KeyNames; assign(record: "task" | "link", slot: number, key: string): string | undefined }

/** The rules of an app that has `assign_task_key` and `assign_link_key`. */
export function keyRules(app: { assign_task_key(slot: number, key: string): string; assign_link_key(slot: number, key: string): string }, keys: "chart" | "server" | undefined, newKey: (() => string) | undefined, names: KeyNames): KeyRules {
  const assign = (record: "task" | "link", slot: number, key: string): string | undefined => {
    const answer = JSON.parse(record === "task" ? app.assign_task_key(slot, key) : app.assign_link_key(slot, key)) as { ok: boolean; message?: string };
    return answer.ok ? undefined : answer.message;
  };
  return { mode: keys ?? "chart", make: newKey ?? uuid, names, assign };
}

/** With `save`, every row a host passes needs a key and no two rows of a kind share one (decision 0022, rule 5.2). The mistake in `rows` (in the chart's names), or undefined. */
export function keyMistake(kind: "task" | "link", rows: readonly object[] | undefined): string | undefined {
  const keys = (rows ?? []).map((row) => (row as { key?: unknown }).key);
  const none = keys.findIndex((key) => typeof key !== "string" || key === "");
  if (none >= 0) return `noKey: with save, every ${kind} needs a key, and ${kind} ${none} (from 0) has none.`;
  const same = twice(keys);
  return same === undefined ? undefined : `duplicateKey: two ${kind}s have the key ${JSON.stringify(same)}.`;
}

const CHART_KEYS: KeyRules = { mode: "chart", make: () => uuid(), names: { task: "key", link: "key" }, assign: () => "the queue was given no way to assign keys" };

const slotOf = (c: Change): number | undefined => (c.record === "task" ? c.task : c.record === "link" ? c.link : undefined);
/** The name a row has inside the queue, `task:3`: the `ref` of its create when the server makes the key. */
const at = (c: Change): string => `${c.record}:${slotOf(c)}`;
const twice = (list: readonly unknown[]): unknown => list.find((x, i) => list.indexOf(x) !== i);

export class SaveQueue {
  private readonly app: SaveApp;
  private readonly save: Save | Handlers;
  private readonly failed: (error: SaveError) => void;
  private readonly keys: KeyRules;
  // The key each row the queue has met holds, by `at`: made here, or answered by the server. A later commit names the row by it.
  private readonly made = new Map<string, string>();
  private readonly waiting: Waiting[] = [];
  // The seq of each host commit made while a commit waits: a put-back takes the ones after its commit back too.
  private hosted: number[] = [];
  private running = false;
  private stopped = false;
  private broken = false;
  // The seq of the last rollback commit: a commit before it that is read late was taken back with the rest.
  private floor = 0;
  private readonly counted: (unsavedCommits: number) => void;
  private told = 0;

  /** `failed` is told of each put-back once, after it. `counted` is told the number of unsaved commits each time it changes (`unsavedCommits` reads it). */
  constructor(app: SaveApp, save: Save | Handlers, failed: (error: SaveError) => void, keys: KeyRules = CHART_KEYS, counted: (unsavedCommits: number) => void = () => undefined) {
    this.app = app;
    this.save = save;
    this.failed = failed;
    this.keys = keys;
    this.counted = counted;
  }

  /** The commits the queue holds that no `save` call has settled: one whose call is running counts; one put back, dropped below the put-back's floor, or dropped by `stop` does not. */
  get unsavedCommits(): number {
    return this.waiting.length;
  }

  private tell(): void {
    if (this.told === this.waiting.length) return;
    this.told = this.waiting.length;
    this.counted(this.told);
  }

  /** Queue the commits an `edit` event carried and start sending. A rollback commit, a host commit (the host made that change itself) and a commit with no change are not sent. */
  add(commits: readonly Commit[]): void {
    if (this.stopped) return;
    for (const commit of commits) {
      if (commit.source === "host") this.hosted.push(commit.seq);
      else if (commit.source !== "rollback" && commit.changes.length > 0 && commit.seq >= this.floor) this.waiting.push(this.keyed({ commit, calls: 0 }));
    }
    if (this.waiting.length === 0) this.hosted = [];
    this.tell();
    this.go();
  }

  /** With `keys: "chart"`, give each row the commit creates with no key one now (the app holds it from here on), before the commit's change set is written. A host with no save keeps the keys it gives the rows itself (`assignKey`). */
  private keyed(waiting: Waiting): Waiting {
    for (const c of waiting.commit.changes) {
      if (this.keys.mode !== "chart" || !hasSave(this.save) || c.op !== "create" || c.key !== null || slotOf(c) === undefined) continue;
      const key = this.keys.make();
      const why = this.keys.assign(c.record as "task" | "link", slotOf(c)!, key);
      if (why === undefined) this.made.set(at(c), key);
      else waiting.bad ??= `the key ${JSON.stringify(key)} for a new ${c.record} was refused: ${why}`;
    }
    return waiting;
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
    this.tell();
  }

  private go(): void {
    void this.pump().catch((err: unknown) => this.halt("the save queue stopped", err));
  }

  /** The queue stops: no further call is made and the commits it holds stay unsaved (`unsavedCommits` keeps counting them). `failed` is told once, with `reason: "stopped"` and `held`. */
  private halt(note: string, err: unknown): void {
    this.broken = true;
    console.error(note, err);
    const head = this.waiting[0];
    this.failed({ source: "save", message: err instanceof Error ? err.message : String(err), fatal: false, reason: "stopped", calls: head?.calls ?? 0, seq: head?.commit.seq ?? 0, undone: 0, reload: false, held: this.waiting.length });
  }

  private async pump(): Promise<void> {
    if (this.running) return;
    this.running = true;
    try {
      while (!this.stopped && !this.broken && this.waiting.length > 0) {
        const head = this.waiting[0]!;
        const save = current(this.save);
        if (save === undefined) {
          this.waiting.shift();
          this.tell();
          continue;
        }
        if (head.bad !== undefined) {
          await this.putBack(new BadResult(head.bad), head);
          continue;
        }
        head.set ??= this.write(head.commit);
        head.calls++;
        let result: SaveResult | void;
        try {
          result = await callSave(save, head.set, () => this.stopped, this.keys.mode === "server" ? this.keys.names : undefined);
        } catch (err) {
          if (this.stopped) return;
          if (!(err instanceof SaveRefused) && !(err instanceof BadResult)) {
            this.halt("a save threw, so the chart stopped saving", err);
            return;
          }
          await this.putBack(err, head);
          continue;
        }
        if (this.stopped) return;
        const bad = this.take(head.set, result);
        if (bad !== undefined) {
          await this.putBack(new BadResult(bad), head);
          continue;
        }
        this.app.acknowledge(head.commit.seq);
        this.waiting.shift();
        this.tell();
      }
    } finally {
      this.running = false;
    }
  }

  /** Take `head` and every commit behind it back, then tell `failed`. A commit the app no longer holds sets `reload`. */
  private async putBack(refusal: SaveRefused | BadResult, head: Waiting): Promise<void> {
    const rollback = (): { ok: boolean; reason?: string; seq: number } => JSON.parse(this.app.rollback_to(head.commit.seq)) as { ok: boolean; reason?: string; seq: number };
    let answer = rollback();
    while (!this.stopped && !answer.ok && answer.reason === "busy") {
      await new Promise((resolve) => setTimeout(resolve, BUSY_WAIT));
      if (!this.stopped) answer = rollback();
    }
    if (this.stopped) return;
    const undone = answer.ok ? this.waiting.length + this.hosted.filter((seq) => seq > head.commit.seq).length : 0;
    this.hosted = [];
    if (answer.ok) this.floor = answer.seq;
    this.waiting.length = 0;
    this.tell();
    this.made.clear(); // the put-back frees the slots of its creates, and a later create may take one
    const own = refusal instanceof SaveRefused ? refusal : undefined;
    this.failed({
      source: "save", message: refusal.message, fatal: false, reason: own === undefined ? "result" : "refused", calls: head.calls,
      ...(own?.key === undefined ? {} : { key: own.key }), ...(own?.field === undefined ? {} : { field: own.field }),
      seq: head.commit.seq, undone, reload: !answer.ok,
    });
  }

  /** The change set of a commit: a new `id`, and `once` on each change. The session slots (`task`, `link`) mean nothing after a reload, so they stay out of what the host stores.
   * A row the commit names by its slot (`{ task: n }`, in `parent`, `after`, `from` or `to`) is named by its key, and a create carries its key in `key` and in `set`. With `keys: "server"` a create carries a `ref`, and every `{ task: n }` that names a row with no key yet is `{ ref }`. */
  private write(commit: Commit): ChangeSet {
    const id = uuid();
    const asks = (c: Change): boolean => this.keys.mode === "server" && c.op === "create" && slotOf(c) !== undefined;
    const asked = new Set(commit.changes.filter((c) => asks(c) && c.key === null).map(at)); // the rows with no key yet: a row that has one is named by it
    const point = (v: unknown): unknown => {
      const slot = typeof v === "object" && v !== null && Object.keys(v).length === 1 ? (v as { task?: unknown }).task : undefined;
      return typeof slot === "number" ? this.made.get(`task:${slot}`) ?? (asked.has(`task:${slot}`) ? { ref: `task:${slot}` } : v) : v;
    };
    const named = (members: Record<string, unknown>): Record<string, unknown> => Object.fromEntries(Object.entries(members).map(([m, v]) => [m, point(v)]));
    const changes = commit.changes.map((c, n) => {
      const { task, link, set, was, ...change } = c;
      const key = c.key ?? this.made.get(at(c)) ?? null;
      const row = set && (key !== null && c.op === "create" ? { ...named(set), [this.keys.names[c.record as "task" | "link"]]: key } : named(set));
      return { ...change, key, ...(asks(c) ? { ref: at(c) } : {}), ...(row === undefined ? {} : { set: row }), ...(was === undefined ? {} : { was: named(was) }), once: `${id}:${n}` };
    });
    return { id, seq: commit.seq, source: commit.source, changes };
  }

  /** What `save` answered, set against what the change set asked: with `keys: "server"` one key for each `ref`, no other, no key twice, each one a key the app accepts, which the row now holds. The mistake, or undefined. */
  private take(set: ChangeSet, result: SaveResult | void): string | undefined {
    const given = (result as SaveResult | undefined)?.keys;
    if (this.keys.mode === "chart") return given === undefined ? undefined : 'the save result has keys, but the chart made the keys of the new rows: mount with keys: "server" when the server makes them';
    if (given !== undefined && !Array.isArray(given)) return "the save result's keys is not a list of { ref, key }";
    const answers = (given ?? []) as { ref: string; key: unknown }[];
    const malformed = (answers as unknown[]).findIndex((a) => typeof (a as { ref?: unknown } | null)?.ref !== "string");
    if (malformed >= 0) return `keys[${malformed}] of the save result is not an object with a text ref`;
    const refs = answers.map((a) => a.ref);
    const asked = set.changes.flatMap((c) => (c.ref === undefined ? [] : [c.ref]));
    // A create that carries its key (the undo of a delete, the redo of a create) may be answered with that key, or not at all.
    const had = new Map(set.changes.flatMap((c) => (c.ref !== undefined && c.key !== null ? [[c.ref, c.key] as const] : [])));
    // Keys are unique for each kind of row (rule 5.2): a task and a link may hold the same one.
    const [missing, extra, again] = [asked.find((r) => !refs.includes(r) && !had.has(r)), refs.find((r) => !asked.includes(r)), twice(refs) !== undefined ? JSON.stringify(twice(refs)) : twice(answers.map((a) => `${a.ref.split(":")[0]} ${JSON.stringify(a.key)}`))];
    if (missing !== undefined) return `the save result has no key for the new row ${missing}`;
    if (extra !== undefined) return `the save result has a key for ${extra}, which no create in the change set asked for`;
    if (again !== undefined) return `the save result names ${String(again)} twice`;
    for (const { ref, key } of answers) {
      const old = had.get(ref);
      if (old !== undefined) {
        if (key !== old) return restoredMismatch(ref.split(":")[0]!, key, old);
        continue;
      }
      const [record, slot] = ref.split(":");
      const why = this.keys.assign(record as "task" | "link", Number(slot), typeof key === "string" ? key : "");
      if (why !== undefined) return `the key ${JSON.stringify(key)} the save result gives ${ref} was refused: ${why}`;
      this.made.set(ref, key as string);
    }
    return undefined;
  }
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
