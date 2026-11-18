# The host save contract

Status: proposed, 2026-11-18. A draft for the owner to read and correct before anything is built; none of it is built. A line marked `Owner decided:` is one of the owner's eleven decisions of 2026-11-18, in three groups: four on the save contract (who makes keys by default, what a network failure does, the field map, one `save` call per commit); four on the URL default (a base URL is the default and there is no separate helper, the route convention is Tachlon's, the chart reads the schedule from `src`, the URL default is built with `save`); and three that followed (a 401 or a 403 is not a refusal and is never repeated by itself, one `headers` option, `src` beside `tasks` or `links` is refused). A line marked `Decided here:` is the drafter's choice where the study and the outside review disagree or leave a point open. Once accepted it amends [decision 0016](0016-host-identity.md) (rules 7 and 8), [decision 0017](0017-host-record-vocabulary.md) (rule 9 and its closing note on mandatory dates) and [decision 0019](0019-commit-pipeline.md) (rules 7, 8, 10 and 12).

A *host* is the product that embeds the chart. This record designs the host's side first: the code a developer, and their coding assistant, writes to connect the chart to a backend. Every rule after the first section exists to make that code true.

## Connect the chart to your backend

### A backend that follows the convention

```tsx
import { Gantt } from "tachlon/react";
import "tachlon/gantt.css";

export const Schedule = () => <Gantt src="/api/schedule" save="/api" style={{ height: 480 }} />;
```

With no framework:

```ts
import { mount } from "tachlon";

const chart = await mount(root, { src: "/api/schedule", save: "/api" });
```

The chart reads the rows from `src` and sends each change to a route under `save`, one request at a time, in the order of the changes (rule 2.6). The convention a backend implements (rule 10.2):

| Request | Body | A 2xx answers |
|---|---|---|
| `GET /api/schedule` (the `src`) | none | `{ tasks, links }`, decision 0017's plain data, with the top-level members a `schedule` change writes |
| `POST /api/tasks`, `POST /api/links` | a create's `set`: the whole row, with the key the chart made under `keys: "chart"` | the stored row; under `keys: "server"` it holds the key the server made (rule 10.5) |
| `PATCH /api/tasks/:key`, `PATCH /api/links/:key` | an update's `set`: the members that changed | 204, or the stored row |
| `PATCH /api/schedule` | the `schedule` change's `set` | 204, or the stored members |
| `DELETE /api/tasks/:key`, `DELETE /api/links/:key` | none | 204 |

- Names are the chart's own (decision 0017's, and `key`), or the host's where `fields` maps them (rule 8.4). Durations are in `durationUnit` and dates in `dateForm` (decision 0017, rules 5 and 7). A reference holds a key (rule 2.5). `:key` is the row's key, URL-encoded. A body is JSON and is sent with `content-type: application/json`; a 2xx other than 204 answers JSON.
- Deleting a task deletes its links on the server: the chart sends no link delete marked `cascade` (rule 2.7).
- Every request but the `GET` carries `Idempotency-Key: <change set id>:<n>`, `n` being the change's position in its change set, from 0. A request with a key the server applied is answered as that one was, for at least 24 hours (rule 3.12).
- A 4xx other than a 401, a 403, a 408 or a 429 is a refusal and answers `{ message, key?, field? }`; the chart puts the commit back (rule 3.5).
- A 401 or a 403 says the person must sign in again. It judged nothing: the chart keeps the edits and asks the person at once, with no automatic call (rule 3.6).
- A 408, a 429 or a 5xx says the server applied nothing. The server stores no answer for it, so the repeat runs the request again (rule 3.6). A server that applied a request answers it with a 2xx.

### An API that is not the convention

The `save` function connects any other API; this sub-section is the *function form*, and the one above is the *URL form*. The backend here follows the convention in everything but its field names (`title`, `plannedStart`, `days`, `parentId`; a link's `source` and `target`), so `src`, `save` and `fields` would connect it too; its `api` and `save` below are what a `save` string runs (rule 10.1), written out. A host whose API differs in its routes, headers or answers starts from them and changes what differs. In React:

```tsx
import { Gantt, SaveRefused, SaveStopped, type Save } from "tachlon/react";
import "tachlon/gantt.css";
import type { Task, Link } from "./api-types"; // the host's own rows

const fields = { // the chart's names on the left, this API's on the right; any other member of a row comes back untouched
  tasks: { key: "id", name: "title", start: "plannedStart", duration: "days", parent: "parentId" },
  links: { key: "id", from: "source", to: "target" },
};

const api = async (method: string, path: string, body?: unknown, once?: string) => {
  const headers = { "content-type": "application/json", ...(once ? { "idempotency-key": once } : {}) };
  const res = await fetch(`/api${path}`, { method, headers, body: JSON.stringify(body), signal: AbortSignal.timeout(10_000) });
  const signIn = res.status === 401 || res.status === 403;
  const judged = res.status >= 400 && res.status < 500 && !signIn && res.status !== 408 && res.status !== 429;
  if (judged) throw new SaveRefused(await res.json());                 // the server said no: the edit is put back
  if (signIn) throw new SaveStopped("Sign in again, then press Try again."); // judged nothing: the chart asks at once, with no automatic call
  if (!res.ok) throw new Error(`The server answered ${res.status}.`);  // a 408, a 429 or a 5xx judged nothing: the chart calls again
  return res.status === 204 ? undefined : res.json();
};

const save: Save = async ({ id, changes }) => {
  for (const [i, c] of changes.entries()) {
    const path = `/${c.record}s`, k = encodeURIComponent(String(c.key)), once = `${id}:${i}`; // once: the same on a repeated call, so the server applies each request once
    if (c.record === "schedule") await api("PATCH", "/schedule", c.set, once);
    else if (c.op === "create") await api("POST", path, c.set, once); // c.set.id is the key the chart made
    else if (c.op === "update") await api("PATCH", `${path}/${k}`, c.set, once);
    else if (!c.cascade) await api("DELETE", `${path}/${k}`, undefined, once); // the database drops a deleted task's links itself
  }
};

export function Schedule({ tasks, links, reload }: { tasks: Task[]; links: Link[]; reload: () => void }) {
  return (
    <Gantt tasks={tasks} links={links} fields={fields} durationUnit="days" dateForm="day"
      save={save} onError={(e) => e.source === "save" && reload()} style={{ height: 480 }} />
  );
}
```

With no framework, the same `fields`, `api` and `save`:

```ts
import { mount } from "tachlon";

const chart = await mount(root, { ...(await api("GET", "/schedule")), fields, durationUnit: "days", dateForm: "day", save });
chart.on("error", async (e) => { if (e.source === "save") chart.setData(await api("GET", "/schedule")); });
serverEvents.onmessage = (m) => chart.update(JSON.parse(m.data)); // every row the server changed, this page's own saves included (rule 4.9)
```

That is all of it. With the convention the host writes two URLs and no fetch, no state and no reload handler: the chart reads `src` again after a put-back (rule 10.3). With the function it writes the function and that read (rule 3.7). In both, the host writes no rename function, no save queue, no retry, no put-back, no diff, no "previous value" and no key bookkeeping, and the rows are as the API returns them (rule 8.4). The chart makes the key of a new row; a server that makes its own ids mounts with `keys="server"`, and its `POST` answers the id (rule 10.5) or the function returns it (rule 5). A lost connection puts nothing back: the chart keeps the edits on screen, calls `save` again, and then asks the person, and an expired login asks at once (rule 3.6), so an error whose `source` is `"save"` always means edits were put back. An API that cannot take the same request twice mounts with `retry={false}` (rule 3.12).

## Context

Embedding the chart in one real product took 659 lines of host code. A study of those lines found that a third (216) exist because the page API has no save function, no put-back and no in-place data update, and another third (224) translate vocabulary: per-field changes folded back into per-record saves, dates cut to a day, six limit fields mapped to one typed constraint. The pieces exist below the page API: the `edit` event carries commits (decision 0019, rule 8), the engine can take commits back (`App.rollback_to` and `App.acknowledge` in `docs/WASM_API.md`), and `update(records)` sets five members by key. The React component starts the chart again when `tasks` or `links` changes (`packages/gantt/src/react.tsx`), so a host cannot pass its own state.

The three components this one is compared with all own the save path. DHTMLX's data processor sends one request per added, updated or deleted record and takes the new id from the response (`tid`) [S1]. Bryntum's project sends one request per sync with every change in it and maps a client-made `$PhantomId` to the server's id [S2, S3]. Syncfusion binds a `DataManager` with an optional batch URL [S4]. None of the pages read asks the host to turn change reports into save calls. An outside review of the plan asked for the contract to be written before any of it is built, with type signatures, and tested against a fake server that misbehaves.

## Decision

Terms used below. *Host data* is the `tasks` and `links` the host gives the chart, or that the chart reads from `src` (rule 10.3). A *commit* is one undo step (decision 0019, rule 1). A *call* is one run of `save`; one change set may be called more than once (rule 3.6). A commit's save *settles* when a call resolves with a result the chart can use (*saved*) or when the chart puts the commit back (*put back*: rules 3.5, 3.6, 3.8 and 3.12). A call that fails and will be repeated settles nothing. A commit is *unsaved* from the moment it is made until its save settles: *queued* before its first call, and *in flight* from that call to the settle, the waits between calls included. A field is unsaved while an unsaved commit writes it; a row is unsaved while an unsaved commit creates or deletes it. Rules name fields by the chart's names; rule 8.4 says where a host sees its own.

### 1. The target

1. **The first section is the contract's test.** A change to this record that makes that code longer, or adds state the host must hold, needs a reason stated beside it. The example app (rule 11) is that code against the fake server, and the README's "Connect to your backend" section is that code, unchanged.

### 2. The shape of a change

```ts
type Key = string;
type Ref = { ref: string }; // names a row created in the same change set that has no key yet (keys: "server" only)

interface Change {
  record: "task" | "link" | "schedule";
  op: "create" | "update" | "delete"; // a "schedule" change is always an update
  key: Key | null;                     // null for the schedule and for a new row under keys: "server" (an undone delete keeps its key, rule 5.6)
  ref?: string;                        // on a create under keys: "server": the name SaveResult.keys answers
  set?: Record<string, unknown>;       // create: the whole record. update: the members that changed, new values
  was?: Record<string, unknown>;       // update: the same members, old values. delete: the whole record as it was
  cascade?: true;                      // a link delete that follows from a task delete in this change set
}

interface ChangeSet { id: string; seq: number; source: CommitSource; changes: readonly Change[] } // id: a UUID, new for every change set, the same on every call of it
```

1. **One change per record.** A commit that sets three fields of one task carries one change with three members in `set` and the same three in `was`. A record appears at most once in a change set. This replaces the one-change-per-field shape of decision 0019, rule 7 (`field`, `old`, `new`), because a host saves records, and folding fields back into records was most of the 224 vocabulary lines.
2. **`set` and `was`.** An update carries only the members that changed. A create carries the whole record and no `was`: every input member, the key under `keys: "chart"` (rule 5.3) and the row's custom fields (rule 8.3). A delete carries the whole record in `was`, its key included, and no `set`, so a host that keeps an audit trail or its own undo has what it removed. Member names in `set` and `was` are the host's (rule 8.4). `was` is the field's *server value*, read when the change set is written (rule 3.3). The chart keeps one server value per field, and it is whichever of these came last: a value that host data or `update` gave the field and the chart applied; a value that the `changed` of one of this chart's own saves gave it, from the moment that result arrives, applied or not yet; the `set` of the last saved commit that wrote it (a `changed` that sets the field comes after the `set` of the save it answers). A value that is held (rule 4.3) is not the server value until it is applied. So a server that compares `was` with its own row (rule 4.10) does not refuse an edit because of a change it made in answer to this chart's own save, and it does refuse an edit made over an outside change the person never saw.
3. **One unit, the one the host mounted with.** Durations and lags in `set` and `was` are in `durationUnit`. This fixes a stated inconsistency: `docs/WASM_API.md` (`App.take_commits`) says a commit's duration is "a number of days of the task's calendar", while `update` reads `durationUnit`. Decision 0017, rule 5 wins: one unit, in and out, on every surface. A host that mounts with hours gets hours.
4. **Dates in the form the host chose.** Under `dateForm: "day"` every date in a change is `YYYY-MM-DD`, converted as decision 0017, rule 7 says (a finish-kind date is the day of its instant less one minute). `was` uses the same form, so a host compares it with what it stored.
5. **References are keys.** `parent`, `after`, `from`, `to` and `calendar` hold the other record's key. The one exception is a row whose create in the same change set carries a `ref` (`keys: "server"`, the undo of a delete included, rule 5.6): every reference to it in that set is `{ ref }`, an object, so it cannot be read as a key and it follows whatever key the server answers. A change set never holds a session slot. `Decided here:` this replaces `{ task: slot }` at the save boundary, because a slot is valid only inside one session and a host that stored one would be wrong after a reload.
6. **Order inside a change set is the order a server can apply top to bottom.** A create comes before any change that names its row (a parent before its children, both tasks before the link that joins them), a link delete comes before the delete of a task it touches, and a child's delete comes before its parent's.
7. **A link delete that follows from a task delete is marked `cascade: true`, and nothing else is.** Deleting a task deletes its links (decision 0017, "A link"); each of those link deletes carries the mark. A host whose database drops a deleted task's links skips marked changes; a host whose database does not applies every change in order. When one delete takes other tasks with it (a summary task and the tasks under it), each of those is an ordinary delete with no mark, so a host that skips marked changes still deletes every task and its server never holds a child of a deleted parent. The mark is only ever on a link's delete: the undo of that delete lists the links as ordinary creates, because the host must make them again.
8. **The `edit` event and the before-edit payload carry the same changes,** with the session slot added (`task` or `link`), because a chart with no `save` may hold rows with no key. A chart with `save` never needs the slot. Both are written when the commit is made, so their `was` is the server value at that moment; the change set that `save` gets is written later (rule 3.3), and its `was` and keys may differ.

### 3. `save`

```ts
type Save = (changeSet: ChangeSet) => Promise<SaveResult | void>;

/** Rows the server changed: whole or partial records by key, and keys it deleted. */
interface ServerChanges { tasks?: readonly TaskRecord[]; links?: readonly LinkRecord[]; deleted?: { tasks?: readonly Key[]; links?: readonly Key[] } }

interface SaveResult {
  keys?: readonly { ref: string; key: Key }[]; // keys: "server" only: exactly one for each create that has a ref
  changed?: ServerChanges;                      // applied by key, with no undo step and no save call
}

/** Throw this from `save` when the server judged the change and said no. Any other throw means "no answer" (rule 3.6). */
class SaveRefused extends Error { constructor(refusal: { message: string; key?: Key; field?: string }) }

/** Throw this from `save` when the server judged nothing and a repeat cannot help until the person acts (a 401 or a 403): rule 3.6. */
class SaveStopped extends Error { constructor(message: string) }

/** The `error` event of a put-back; it fires once per put-back, after it. `calls`: how many times `save` ran for the change set.
 *  `reload`: the chart could not put itself back, so its next data update replaces everything (rule 3.11). */
interface SaveError { source: "save"; message: string; fatal: false; reason: "refused" | "failed" | "result"; calls: number;
  key?: Key; field?: string; seq: number; undone: number; reload: boolean }

type Retry = readonly number[] | false; // the waits before each repeated call, in milliseconds; default [1000, 3000, 9000] (rule 3.6)
```

1. **`save` is a mount option and a prop:** a `Save` function, or a base URL (rule 10.1). `CommitSource` is `typed`, `drag`, `dialog`, `paste`, `proposal`, `assistant`, `host`, `undo` or `redo`; a `rollback` commit is never sent. A commit with no changes for the host is not sent. With no `save`, the chart behaves as 0.3.1 does and the host reads the `edit` event. Each call uses the `save` the chart holds at that moment, so a `save` that is a new function on every render is safe; `keys` and `retry` are read once, at start.
2. **One call per commit, with the whole change set.** `Owner decided:` as drafted, not one call per record. The commit is the unit the chart can put back, so it must be the unit the host saves: a commit saved in three calls that fails on the second has no clean way back. A host coming from a batched library (Bryntum sends every store's `added`, `updated` and `removed` in one request [S2]) posts the change set to one endpoint and saves it in one transaction: `const save: Save = (cs) => api("POST", "/changes", cs, cs.id)`. A host coming from a per-record library (DHTMLX sends one request per record [S1]) loops over `changes` in order, as the first section does. Under `keys: "server"` that loop also keeps a map from each `ref` to the id its `POST` returned and replaces a `{ ref }` it meets later in the same set: about four lines, which a change-set endpoint or chart-made keys never need.
3. **One save in flight, in commit order.** The person keeps editing while a save runs; those commits queue. The chart calls `save` for the next commit only after the one before is saved and its keys have been assigned (the rest of its result is applied in the same step, or when a gesture ends: rule 3.10), and it never has two calls running: a change set is called again only after its last call rejected. A change set is written at its first call, not when the commit is made, so it names every key that earlier saves assigned; a repeated call passes the same change set, unchanged. `seq` rises with every commit and is never reused in a session; the gaps a `rollback` commit leaves are expected. Queued commits are not merged into one call (rule 14).
4. **Resolving means saved.** A result is read for `keys` and `changed` only. Anything else `save` resolves with is ignored, so it may resolve with whatever its endpoint answers. With neither member, the chart takes it that the server holds the change as sent. A server that changes more than it was sent says so in `changed`; a `save` that returns nothing, as the first section's does, leaves those changes to host data or `update` (rule 4.9). `keys` assigns server-made keys (rule 5). `changed` is what the server changed beyond what it was sent: rows it recalculated, a name it trimmed, a row a rule of its own deleted. The chart applies `changed` by key with no undo step, no `save` call and no before-edit call, by the rules of section 4.
5. **`SaveRefused` means the server said no, and the chart puts back at once.** A *put-back* restores the state before that commit, takes back every commit queued behind it as well (they were made on top of it; decision 0019, rules 10 and 11), fires `error` with a `SaveError`, and shows `message` in the chart's alert line with the number of commits undone. Here `reason` is `"refused"`; for a `key` the chart selects that row, and for a `field` (the host's name for it) it shows the message at that cell. A refused change set is never called again, so a host throws `SaveRefused` only for an answer that judged the change; a 401, a 403, a 408 or a 429 did not. Rows whose commit is unsaved are dimmed as `setPending` dims them, by the chart, until the commit settles, or until a call fails and rule 3.6's mark replaces the dim on every one of them.
6. **Any other throw is a failure, and a failure puts nothing back.** `Owner decided:` a lost connection, a timeout or a 5xx says nothing about the change, so the chart keeps every unsaved edit on screen and calls `save` again with the same change set (rule 3.12 says what that asks of the server). `Decided here:` the details.
   - *The calls.* Up to three more: the first 1 second after the failure, the second 3 seconds after its own failure, the third 9 seconds after its own. That is `retry: [1000, 3000, 9000]`, the default. A host passes its own waits; `[]` means no automatic call, and the chart asks the person after the first failure.
   - *What the person sees.* From the first failure, every row an unsaved commit touches carries a "not saved" mark where it was dimmed, and the alert line reads "Not saved yet. Trying again." Both go when the commit is saved.
   - *Editing goes on.* New commits queue behind the failed one as always. None of them is called before it is saved, so order holds, and they are put back only if it is.
   - *When the last call fails,* the chart stops calling. The alert line reads "Not saved: " and the message, with two buttons: "Try again" and "Discard N changes", N being the number of unsaved commits, kept current. It stays until the person chooses, and editing still goes on; rule 3.12 says what changes after 24 hours. "Try again" calls once more and, if that fails, runs the waits again. "Discard" is the put-back of rule 3.5 for the failed commit and all behind it, with `reason: "failed"`.
   - *An expired login.* `Owner decided:` a 401 or a 403 is not a refusal, and a repeat cannot fix it until the person signs in again. `save` says so by throwing `SaveStopped`, as the first section's `api` does; any other throw is repeated as above. On `SaveStopped` the chart makes no automatic call and goes straight to the stop above: the mark, "Not saved: " and the message, and both buttons. "Try again" calls once and, if that meets `SaveStopped` again, stops again at once; it works once the person has signed in. Under `retry: false` it is put back as any failure is (rule 3.12).
   - *For the host.* No `error` fires while the chart is repeating or stopped: `error` with `source: "save"` always means edits were put back. `chart.state().save` is `"idle"`, `"saving"`, `"retrying"` (a call failed and another will follow) or `"stopped"` (the chart is asking the person); a host with its own controls calls `chart.commands.retrySave()` and `chart.commands.discardUnsaved()`, which do what the buttons do and do nothing in any other state.
   - *No timeout.* The chart never gives up on a call that is running; a promise that never settles holds the queue, and `unsaved` shows it. A `save` that should give up aborts its own request and throws, as the first section does.
7. **After a put-back, the host reads its data again.** The chart has put the edits back; the host's data then says what the server kept. This covers a discard after a lost response (the server did save: the edit comes back with the fresh data) and a per-record loop refused part of the way (the server holds half: the fresh data shows it). The first section's function form does this in `onError`; with `src` the chart does it (rule 10.3).
8. **A result the chart cannot use is put back, not repeated.** A missing, extra or duplicate entry in `keys`, a key another row holds, `keys` under `keys: "chart"`, or a record in `changed` that the validator refuses: the chart puts back as rule 3.5 does, with `reason: "result"` and a message that names the mistake. Calling again would get the same answer. The server may hold the rows; the host's fresh data shows them.
9. **Unsaved work at teardown.** `chart.state().unsaved` is the number of unsaved commits. `chart.saved()` resolves when every commit that was unsaved at the call has been saved, at once if there was none, and rejects with the `SaveError` if one of them is put back. `destroy()`, and so a React unmount, does not wait: a call that is running finishes and its result is ignored, no further call is made, and every other unsaved commit is dropped. A host that must not lose them warns on leave while `unsaved` is above 0, or awaits `saved()`.
10. **A settle never waits for the person and never changes a value under their hands.** If a put-back comes while a cell is being edited or a bar is held, the chart cancels that gesture first. A failed call that will be repeated cancels nothing. If a save succeeds during a gesture, keys are assigned at once and the next commit is called; `changed`, held values and waiting rows (section 4) are applied when the gesture ends. If more than one save settled during the gesture, their deferred steps are applied in settle order, and each is judged as it would have been at its own settle: a field that was unsaved when commit 1 settled, because commit 2 had changed it, is still held against commit 1's `changed` (rule 4.6) even though commit 2 has since been saved, and commit 2's own step then decides it. A saved edit is never replaced on screen by an older answer.
11. **When a put-back is due and the chart cannot do it** (the history no longer holds the commit after 200 steps, or the chart restarted on WebGL2 since the commit was made), `SaveError.reload` is true, the queue and the history are emptied, and the next data update replaces everything the chart shows: every member applies and a row it lacks is removed (rule 4.5). The host does what it does after any put-back, as in the first section; it needs no other call. `Decided here:` the method `Gantt.save()`, which downloads the document, is renamed `download()` in the release that adds the `save` option, so that "save" has one meaning.
12. **`save` must be safe to repeat, or the host turns repeats off.** A repeated call must leave the server as one successful call would.
    - *The change set's `id` is the idempotency key.* A server that has applied an `id` answers a repeat as it did the first time, with the same `keys` and `changed`, before any other check; the `was` comparison of rule 4.10 would otherwise refuse the server's own earlier write. It keeps answers for at least 24 hours, because a person may press "Try again" hours later, and a repeat that arrives while the first is still being applied waits for it. `Decided here:` the chart holds up its end: it never calls a change set more than 24 hours after that set's first call. The 24 hours are wall-clock time (`Date.now()`), read at each call and at each press of "Try again", never a timer: a timer stops while the machine sleeps, and the server's stored answers do not. Past that, "Try again" and `retrySave()` are gone, the alert line says the changes are too old to send again, and "Discard" is the one way on. `seq` is not a key: it starts again at 1 in every session.
    - *Per-record routes* send the `id` with the change's position as each request's key, as the first section does. A repeat then goes through the requests already applied, each answered as before, and carries on with the rest.
    - *`retry: false`* is for an API that cannot do this. The chart calls `save` once per change set and never again: any failure is put back at once, as rule 3.5 does, with `reason: "failed"`, and there are no buttons.
    - *What an unsafe repeat costs.* Under `keys: "chart"` a repeated create carries the same key, so a server with a unique key refuses it, and the read after that put-back shows one row. Under `keys: "server"` nothing marks the repeat, and the server stores the row twice. So `keys: "server"` needs the idempotency key honored, or `retry: false`.

### 4. Who wins between host data and unsaved edits

1. **Data props update the chart in place, by key.** A change to `tasks` or `links` (in React, Vue and Angular) or a call to `chart.setData({ tasks, links })` (plain `mount`) never starts the chart again. It is not an edit: no undo step, no `save` call, no before-edit call, and the selection and scroll position stay.
2. **A data update is the difference from the one before it.** The chart remembers the last host data. For each key it compares the new record with the remembered one, member by member and by value: a member that differs is a new value, a key that is new is a new row, a key that is gone is a removed row. A member that did not change does nothing, whatever the chart shows. `Decided here:` this, and not "host data replaces what the chart shows". Reason: a host passes old data all the time without meaning to (a render that builds a new array, a read that began before a save and ends after it), and under the other rule each of those would undo a saved edit or bring back a deleted row. The cost: the host must keep its data current, because a change that never appears in host data is never seen. Passing the same array again is skipped without comparing.
3. **An unsaved field keeps its local value.** A new host value for it is *held* (the latest one wins) and does not show. Other fields of the same record update at once. So an outside change to a row with a change in flight shows right away in every field the person did not touch.
4. **When the commit is saved,** the chart applies, in this order and in one synchronous step: the keys in `keys`; then `changed`; then held values; then waiting rows (rule 4.7). For a held value of a field that commit wrote, the first test that matches decides:
   - if `changed` sets that member, `changed` is applied and the held value for that member is dropped: the server has answered. Held values of members `changed` does not set are judged by the next two tests, and a `changed` member whose field a later commit still writes is held under rule 4.6;
   - if the held value arrived before the commit's first call, it is dropped: the save is newer;
   - if it arrived later, at any time from the first call to the settle, it is applied: the chart cannot know which the server took last, and the next data update corrects it if the save won.

   Rule 3.10 is the one exception to "one step": during a gesture, everything after the keys waits for the gesture to end. A `changed` that waited counts as having arrived when its save settled; a held value keeps the time it arrived (rule 3.10).

   A field that a later unsaved commit also writes stays unsaved. A held value that this save's tests did not drop waits for that commit under the same three tests; one they dropped is gone.
5. **When the commit is put back,** the commits behind it go with it, and every held value for a field that is no longer unsaved is applied. The chart also forgets what it remembered (rule 4.2) for every record those commits touched, because the server may hold part of them or all of them. For those records the next data update is the truth and is not compared: every member applies whatever its value, and a touched row that the update lacks is removed, a row the chart created included. A commit made after the put-back keeps its say: a field it leaves unsaved keeps its local value and the update's value is held (rule 4.3), and a row it leaves unsaved is not removed before that commit settles (rule 4.7). With `reload: true` (rule 3.11) this holds for every record. So the read the host makes after a put-back (rule 3.7) always lands.
6. **`changed` and `chart.update(changes)` are patches.** Each takes `ServerChanges`: records by key holding only the members to set. A member is applied at once unless its field is unsaved, in which case it is held (rule 4.3) and judged by the tests of rule 4.4 (a `changed` value is always older than any commit still queued, so it is dropped when that commit is saved). A patch does not alter the remembered host data. `update` keeps taking the 0.3.1 array of task records.
7. **Rows.** A key that is new in host data and unknown to the chart is added, with one exception: under `keys: "server"`, while a commit that creates a row is in flight, an unknown key *waits* until that save settles, however many calls that takes, because it may be the server's copy of the row the chart is still saving (rule 5.4). When the save settles as a put-back, the waiting rows are added after it. A key that is new in host data and already in the chart (a row the chart created, coming back) is compared with the chart's values member by member, and the differences apply as in rule 4.3. A key that is gone from host data is removed with its links; if the row has an unsaved change, the removal is held like a value. A row the chart created is not removed because host data lacks it: only a key that host data once held can go missing. Two things do remove it: the data update after a put-back that touched it (rule 4.5), and a `deleted` entry in `changed` or `update`. `Decided here:` this stays a stated limit. A row the chart created, that host data has never listed, and that another person deletes, stays until one of those two happens. Chart-made keys (rule 5.3) do not close it: the server knows the row by its key from the create on, but a data update that lacks the key may be a read that began before the create was saved, and telling that from a delete needs a version, which rule 14 leaves out. Two things keep it small. It ends the first time host data lists the row. And an edit of the leftover row reaches a server that does not hold its key; that server's refusal is a put-back that touched the row, so the read after it removes the row. A row in `changed` or `update` with an unknown key is added if the record is whole and refused (`unknownKey`) if it is not. Under `keys: "server"`, while a create is in flight, a record in `update` with an unknown key waits as one in host data does, and is judged when the save settles: a server that pushes its new row before it answers the create must not make a second row.
8. **What the person deleted stays deleted** while host data still lists it, because an unchanged record is no difference. A member of it that changes in host data is remembered and changes nothing: the row comes back only when its key leaves host data and later returns. If the delete is put back, the row comes back.
9. **A late result is a data update like any other.** A server that recalculates asynchronously and reports minutes later sends its changes through `update` (or the host puts them in its data), and rules 4.2 to 4.7 apply as written: fields with an unsaved change keep the person's value, every other field takes the late one. The chart has no version check, so three duties fall to the server. It sends only the rows and members it changed, never a whole schedule read before the last save settled. What it sends covers every write, this page's own saves included, unless `save` returns the saved rows in `changed`: after the last test of rule 4.4 the chart may show an outside value that the person's save has since replaced on the server, and only the server's word for that field, in `changed` or in later data, corrects it. And it sends inputs only: a computed date is not an input. `start` in a record is a wanted start and is kept as a start-no-earlier-than limit (decision 0017), so a server sends `start` only where it means that. Comparing a server's computed dates with the chart's own is row `tl2087hd`, not this record. While a late result is expected, the host marks the rows with `pending`, which is a hint and not a lock.
10. **`was` is the server's conflict check.** The chart resolves nothing between two people. A server that must not let a save overwrite an outside change compares `was` with what it holds and refuses, after it has checked for a repeated `id` (rule 3.12).

### 5. Keys for new rows

1. **The chart makes the key of a new row, unless the host says the server does.** `Owner decided:` `keys: "chart"` is the default, because a row that has its key from the start cannot appear twice. `keys: "server"` is the one-word opt-in for a host whose server makes ids. Both modes are first-class and pass the same scenarios (rule 11). The default has one trap, a server that makes its own ids behind a host that left `keys` out; rule 5.3 says how it shows.
2. **Every row has a key.** With `save`, a task or link in host data with no `key` is refused (`noKey`) at `mount` and in `setData`, and so are two tasks, or two links, with one key (`duplicateKey`; keys are unique per record kind, decision 0016, rule 3). A record that has a `key` may leave `id` out, and `parent`, `from` and `to` then name keys; a payload names records by `id` throughout or by `key` throughout, never both. `Decided here:` this loosens decision 0016, rule 8, so a host passes its rows in the shape that changes come back in.
3. **`keys: "chart"`.** The chart makes the key when the row is made, before the commit is written, with `crypto.randomUUID()` or the host's `newKey: () => string`. The create carries it in `key` and, under the host's name for the key, in `set`; there is no `ref`, and a reference to the row is its key from the first change on. The server stores that key as the row's id, or in a unique column beside its own id, and gives it back as the row's key in host data. The row is never without a key, so the server's copy and the chart's row are one row whenever the copy arrives. A server that ignores the key and gives the row an id of its own breaks this: its row arrives in host data under a key the chart does not know and shows beside the chart's own, and the chart cannot tell that from another person's new row. There are two guards. A result that carries `keys` in this mode is put back with a message that says to mount with `keys: "server"` (rule 3.8). And a data update that lacks the key of a create already saved, and adds an unknown key of the same record type, logs one console warning per session that names both keys and the `keys` setting.
4. **`keys: "server"`.** The create carries `key: null` and a `ref`. `save` returns one `{ ref, key }` for it. The key is assigned when the save settles, as the first thing in the synchronous step of rule 4.4: before `changed`, before any held value and before any waiting row. Until then an unknown key in host data or in `update` waits (rule 4.7). So when the server's copy of the row reaches the chart, early or late, the chart's row already holds that key and the two are one row. Edits the person made to the new row meanwhile are queued, and their change sets are written after the key exists (rule 3.3). A repeated call must be answered with the same key, or the server holds the row twice (rule 3.12).
5. **A create that is put back** takes the row away, and every queued commit with it (rule 3.5).
6. **The undo of a delete** is a create that carries the key the row had. Under `keys: "server"` it also carries a `ref`, and the result may answer with a new key: the chart then gives the row the new key and the old one is free. History entries follow, because the history holds slots and never keys (decision 0016, rule 1).

### 6. Undo

1. **The host says who owns the history: `history: "chart"` (default) or `history: "host"`.** Both are first-class.
2. **`"chart"`: one history, in the chart.** Undo and redo are commits with `source: "undo"` and `"redo"`, and they reach `save` as any commit does: the undo of an edit is an update with `set` and `was` swapped, the undo of an insert is a delete, and the undo of a delete is a create (rule 5.6) followed by creates for its links. `save` may refuse one; the put-back then restores the state before the undo, and the step is on the undo list again. What the host needs: an API that accepts the reverse of everything it accepts, which means it can make a deleted row again from the record in `set`, with its custom fields and, under `keys: "chart"`, under the key it had. For its own toolbar it reads `chart.state().canUndo` and `canRedo` and calls `chart.commands.undo()`. A host-only action is an entry on the same history, as decision 0019, rule 12 says.
3. **`"host"`: the host owns history and the chart's own undo is off.** The undo and redo keys fire `undo` and `redo` events, as the find key fires `find`, and change nothing. The host records what it needs from each change set (`set` and `was`), runs its own reverse against its API, and the result arrives as host data (section 4), never as a commit. The chart still puts a commit back when its save is refused or discarded (rules 3.5 and 3.6); that is not undo. What the host needs: its own stack and a reverse for each action.
4. **Entries a put-back makes stale.** A put-back removes the commit and those behind it from the undo list and leaves no redo step for them (`App.rollback_to` in `docs/WASM_API.md`).
5. **Entries a server change makes stale.** An undo or redo step is stale when a server change (`changed`, a data update or `update`), at the moment it is applied, alters the value the chart shows for a field the step wrote, or removes a record the step touches. `Decided here:` a stale undo step leaves the history together with every step older than it; newer steps stay. If any redo step is stale, the redo list is emptied. Reason: steps are taken back newest first, and an older step was recorded against a state the stale step can no longer restore. The chart says so once in its alert line ("3 undo steps were removed: the server changed those rows") and `canUndo` follows. A server change that leaves the shown value as it is, such as the host's data coming back with what the chart just saved, makes nothing stale.

### 7. `beforeEdit` beside `save`

1. **Keep both.** `beforeEdit` is synchronous and runs at the gate before anything changes (decision 0019, rules 3 to 6). `save` is asynchronous and runs after the edit shows.
2. **`beforeEdit` is for rules the page can answer from what it already holds:** a locked row, a role, a summary row the host treats as a folder. It makes no network call. A refusal there costs the person nothing: no change, no put-back.
3. **`save` is where the server's rules land:** validation, conflicts, locks, permissions only the server knows. A refusal there is a `SaveRefused` with `message`, `key` and `field`.
4. **A rule that never varies belongs in the capability profile,** which is checked before `beforeEdit` and also removes the affordance. The order for one action: read-only, capabilities, `beforeEdit`, the edit shows, `save`.
5. **`beforeEdit` is not asked again at a drag's release.** Snapping (rule 8.2) makes the released value one the host can store; the server refuses the rest in `save`.

### 8. What the host says once, at start

1. **These settings describe the host; none is stored in the document.** Each is an option of `mount` and a prop of the components.

   | Setting | Values | Says |
   |---|---|---|
   | `src` | a URL | where the chart reads host data itself (rule 10.3) |
   | `headers` | an object, or a function that returns one | headers added to the chart's own requests (rule 10.6) |
   | `fields` | a map for tasks and one for links | the host's name for each chart field (rule 8.4) |
   | `durationUnit` | `"days"`, `"hours"`, `"minutes"` | the unit of every duration and lag, in and out (decision 0017, rule 5) |
   | `dateForm` | `"minute"`, `"day"` | the form of every date out (decision 0017, rule 7) |
   | `keys` | `"chart"` (default), `"server"` | who makes the key of a new row (rule 5) |
   | `retry` | waits in milliseconds (default `[1000, 3000, 9000]`), `false` | when a failed call is repeated (rules 3.6 and 3.12) |
   | `history` | `"chart"` (default), `"host"` | who owns undo (rule 6) |
   | `capabilities` | the profile below | what the host's database can take (decision 0018, rules 9 to 12) |

2. **The profile gains four members.** `constraints: { types: ConstraintType[] }` lists the constraint types a person may set (`[]` for none; rule 9). `durationStep` and `lagStep` are `"minute"`, `"hour"` or `"day"`: a typed value that is not a whole step is refused, and a drag snaps to the step. `editable.structure` splits into `rows` (insert and delete), `outline` (indent, outdent and move) and `paste`. The profile is a prop of the React, Vue and Angular components. Each member is enforced at the gate and changes no computed number.
3. **Custom fields pass through.** A member of a task or link in host data that is not a chart field, once the map of rule 8.4 is applied, is a *custom field*. `Owner decided:` the chart keeps it with the record, never reads, shows or edits it, and gives it back unchanged under the same name, in a create's `set` and a delete's `was`. So the create that undoes a delete gives the host back its own columns, as they were when the row was deleted (rule 6.2). A data update or patch sets a custom field as it sets any member, compared by value; no edit writes one, so it is never unsaved and never held. A row the person makes in the chart has none. This amends decision 0017, rule 9, which left an unknown member of a task or link out and counted it in `leftOut`. The cost: a misspelled member (`durration`) is kept and no longer counted. `chart.state().customFields` lists the names seen for tasks and for links, so a developer finds it there. The chart does not show custom fields (columns are rule 14), and the document does not store them yet.
4. **The field map.** `Owner decided:` the map is in this record, so a host writes no rename function (Syncfusion's `taskFields` is the same idea [S4]).

   ```ts
   interface Fields {
     tasks?: { [chartField: string]: string | { type: string; date: string } }; // the object form is for `constraint` only
     links?: { [chartField: string]: string };
   }
   ```

   - *What it says.* Each entry gives one chart field (decision 0017's names, and `key`) the host's name for it: `{ name: "title", start: "plannedStart", duration: "days" }`. A chart field with no entry keeps its own name. The map is read once, at start, so an object written inline in a render is safe and a later change to it does nothing. The map renames members and converts no value: units are `durationUnit`, dates are `dateForm`, and a reference holds a key (rule 2.5).
   - *Where it applies.* Wherever a task or link crosses between host and chart: host data at `mount`, in props and in `setData`; `update` and `changed`; `set` and `was` in every change (`save`, the `edit` event, the before-edit payload); and `field` in `SaveRefused` and `SaveError`. It does not apply to a change's own members (`record`, `op`, `key`, `ref`, `cascade`), to a `schedule` change, or to the document and `snapshot()`, which keep the chart's names.
   - *The key.* `key: "id"` says the host's `id` member is the row's key. A create under `keys: "chart"` then carries the new key as `set.id`, and `parent`, `from` and `to` hold such ids under whatever names the map gives those fields.
   - *The typed constraint* (section 9) maps as one member that holds `{ type, date }` or `null` (`constraint: "limit"`), or as two flat members (`constraint: { type: "constraintType", date: "constraintDate" }`). In the flat form both members are `null` for no constraint, a change that writes the constraint writes both in `set` and both in `was`, and a record with only one of the two set is refused. The `type` values are the six of section 9. An entry for `constraint` under `constraintForm: "separate"` is refused at `mount`.
   - *One to one, checked at `mount` before any data is read.* Three mistakes make `mount` reject, with a message that names the fields: an entry for a chart field that does not exist; one host name given to two chart fields; and a host name that is the own name of another chart field that has no entry (`{ name: "start" }` alone, where the host's `start` would feed two fields). One exception: `key: "id"` needs no entry for the chart's own `id`, which a record with a key leaves out (rule 5.2).
   - *Two host members never feed one chart field.* Once a chart field has an entry, a host member that carries the chart's own name for that field is not that field: with `name: "title"`, a row's own `name` member is a custom field and passes through (rule 8.3).
   - *A host name no row carries.* When the first host data that holds rows has none with a mapped name, the chart logs one console warning that names it; the misspelling would otherwise show only as an empty column.
   - *Out of scope.* Nested paths (`"dates.start"`), computed fields (a finish made from two columns), value conversion (a status word to a percent, another spelling of a link type or constraint type) and ids that are numbers (a key is text: decision 0016, rule 3). A host with any of those maps them in its own code, before `tasks` and inside `save`, and rule 12 counts those lines.

### 9. The typed constraint at the boundary

```ts
type ConstraintType = "snet" | "snlt" | "fnet" | "fnlt" | "mso" | "mfo";
type Constraint = { type: ConstraintType; date: string } | null;
```

1. **A task has one member, `constraint`,** in host data, changes, patches and the before-edit payload. `null` is no constraint. A host that stores it as two columns says so in the field map (rule 8.4). The type names are decision 0017's limit names. Host stores and exchange files hold one typed constraint per activity, and the reference host's 71 lines of limit mapping exist only because the chart speaks six separate fields.
2. **`constraint` and the six date-limit members are two spellings of the same engine bounds.** A task in host data that carries both is refused. Which spelling the chart writes is one more setting of the kind in rule 8.1: `constraintForm: "typed" | "separate"`. `Decided here:` the default stays `"separate"` through 0.3.x so no host breaks, and becomes `"typed"` at 0.4.0. `alap` and `resume` are not date limits and stay their own members.
3. **The mapping.**

   | `type` | Engine bound (decision 0017, "A task") |
   |---|---|
   | `snet` | `snet`, the start floor |
   | `snlt` | `limit.snlt`, the start ceiling |
   | `fnet` | `limit.fnet`, the finish floor |
   | `fnlt` | `limit.fnlt`, the finish ceiling |
   | `mso` | `limit.pin_start` when `mandatoryDates` is `"pin"`; `snet` and `limit.snlt` on the same instant when it is `"bounds"` |
   | `mfo` | `limit.pin_finish` when `"pin"`; `limit.fnet` and `limit.fnlt` on the same instant when `"bounds"` |

4. **"Must start on" is a schedule option, not a mapping the host writes.** `mandatoryDates` is a member of `options` (decision 0018, rule 4) with the values `"pin"` (default: the date holds even when logic pushes past it, as today's `mso`) and `"bounds"` (a floor and a ceiling on the same day, so logic can push past and float goes negative). The two give different dates, so it travels with the schedule. Under `"bounds"` a task whose floor and ceiling sit on the same instant reads back as `mso` or `mfo`. This replaces the closing note of decision 0017, which tells the host to send two fields.
5. **Under `"typed"` a task holds one constraint.** Setting one replaces every date bound the task had, as one change with `set: { constraint }` and `was: { constraint }`. `start` in host data is still a wanted start kept as `snet`, unless the record has a `constraint` member (`null` included), which wins. A task that arrives from a file with more than one bound keeps them all in the engine; its `constraint` reads the first that is set in the order `mso`, `mfo`, `snlt`, `fnlt`, `snet`, `fnet`, and `leftOut` counts the rest.

### 10. The URL default

1. **A `save` given as a string is the first section's `save` function, run by the chart.** `Owner decided:` the default that needs almost no host code is a base URL, and there is no separately imported helper. `save: "/api"` is the `api` and `save` of the first section's function form, with `/api` replaced by the string, the members of `headers` added to every request (rule 10.6), and, under `keys: "server"`, rule 3.2's map from each `ref` to the key its `POST` answered (rule 10.5). That is the definition, so every rule of section 3 applies to it unchanged (one call per commit, order, refusal, failure and repeats, `SaveStopped`, the 24-hour limit, teardown), `retry` and `keys` mean what they mean there, and there is no second queue. A change to the convention is a change to that code, and the other way round.
2. **The convention is Tachlon's.** `Owner decided:` the table and the list in the first section are the one specification of the routes, the field names, the refusal and the idempotency key. A host's coding assistant builds or adapts the backend to it; the fake server (rule 11.1) is a backend that speaks it. An API that differs only in its field names uses `fields` (rule 10.4); one that differs in anything else uses the `save` function.
3. **`src`: the chart reads the schedule itself.** `Owner decided:` the chart sends `src` a `GET`, with `headers` and the 10-second limit of the first section's `api`, at mount and after every `error` whose `source` is `"save"` (every put-back fires one, rule 3.5, and so does rule 3.11's): that is rule 3.7's read, done by the chart. Nothing else starts a read: no timer, no focus, no reconnect. A read started while another runs drops the older one's answer. The answer at mount is read as `mount` reads plain data. A re-read is `setData` (rule 4.1), so it is host data, and rules 4.2 to 4.8 judge it against unsaved edits; after a put-back, rule 4.5 makes it the truth for the records the put-back touched.
   - *A failed read.* A network error, a 408, a 429 or a 5xx is read again at `retry`'s waits (rule 3.6; none under `retry: false`); another status, a body that is not JSON, or data the validator refuses is not. When the last read at mount fails, `mount` rejects with a message that names the URL and the status, and a component fires `error` with `source: "src"` and `fatal: true` and shows no chart. When the last re-read fails, the chart keeps what it shows, fires `error` once with `source: "src"` and `fatal: false`, and the alert line reads "The schedule could not be read again. Reload the page to see what was saved."; editing and saving go on, and the next put-back reads again.
   - *`src` beside `tasks` or `links`.* `Owner decided:` refused at mount: `mount` rejects, and a component fires `error` with `source: "src"` and `fatal: true`, with a message that names both options and says to pick one. A host that passes `tasks` and `links` itself makes rule 3.7's read itself, as the first section's function form does; a `save` string works with either.
   - *A new `src`.* A new value is a new document. A component starts the chart again, as at mount, and the unsaved commits are dropped as at `destroy()` (rule 3.9); with `mount`, the host calls `destroy()` and mounts again. A read still running for the old value is dropped. A host that must keep the unsaved commits awaits `saved()` first.
4. **`fields` with the URL form.** Allowed, and applied as rule 8.4 says: to what `src` answers as to host data, to request bodies as to `set`, and to a refusal's `field`. A `:key` in a path is the key's value; the map renames members only.
5. **Keys.** Under `keys: "chart"`, the default (rule 5.1), a `POST` carries the key in its body (rule 5.3) and the body of its answer is not read. Under `keys: "server"` the `POST` must answer a JSON object that holds the new row's key under the host's name for `key` (the stored row does). The chart takes that as the create's `{ ref, key }` (rule 5.4) and puts the key in place of every later `{ ref }` to the row in the same change set. An answer without that member is a result the chart cannot use (rule 3.8). A repeated `POST`, answered as the first one was, gives the same key (rule 3.12).
6. **Credentials and headers.** `Owner decided:` requests use `fetch`'s default credentials, so a same-origin API with a session cookie needs nothing. An API that needs a token sets `headers`: an object, or a function that returns one (`Decided here:` or a promise of one, so a token from an asynchronous source works). The chart reads it, or calls it, before each request for `src` and `save`, so a renewed token is used from the next request on. There is no `fetch` option: a cross-origin cookie, another timeout or any other request shape uses the `save` function, and passes its data itself.
7. **Server changes are not part of the URL form.** A stated limit: the URL form reads no `changed` from an answer and reads `src` only when rule 10.3 says. A server that changes rows it was not sent, or pushes another person's changes, reaches the chart through `chart.update` (rules 4.6 and 4.9), which the host calls, as the first section's plain `mount` does with `serverEvents`, or through a `save` function that returns `changed` (rule 3.4).
8. **Errors.** In the URL form, `error` with `source: "save"` still fires once per put-back (rule 3.5), and the chart's own read follows it; a host needs no handler. `error` with `source: "src"` fires only as rule 10.3 says. Rule 3.6 holds: neither fires while a call or a read is being repeated, or while the chart is asking the person.

### 11. The fake server and its scenarios

1. **The example app is the first section's code against a fake server that misbehaves on purpose.** The server lives under `examples/`, holds its rows in memory, is deterministic from a seed, and takes a script: latency per request, which request to refuse and with what, when to answer a 401 or a 500, when to change a row from outside, when to push a late result, when to drop a request or a response, and whether to honor a repeated idempotency key. It speaks the first section's convention (routes, answers, the refusal, the idempotency key), so it is also a reference backend a host's assistant can read. Its rows use the chart's names or, as the script says, its own, as the function form's backend does. It serves both route styles (per record, and one change-set endpoint) and both key modes. The suite drives the chart in a browser, through both forms of the first section: the URL form, given the function form's `fields`, `durationUnit` and `dateForm` when the rows use the server's own names, and the function form. Every scenario runs through both forms except these: scenario 6 (the change-set endpoint needs a function); scenarios 7, 8 and 9 and the second case of scenario 3, in which host data arrives from outside (rule 10.7); and scenarios 18 to 20, which need `src` and run through the URL form only. Under `keys: "chart"` it runs the first section's code as written. Under `keys: "server"` it runs a copy that differs in that setting and, in the function form, in a `save` that returns the keys (rule 3.2), and in nothing else. A scenario that names a setting (`retry: false` in scenario 15) or the change-set endpoint (scenario 6) changes that and nothing else.
2. **The scenarios.**
   1. *Slow saves.* With 800 ms of latency, three typed edits reach the server as three calls in commit order, never two at once.
   2. *A field refusal.* The server refuses a duration over 100 days with a message, the key and `field: "days"`, its own name for it; the cell goes back, the row is selected and the message shows at the duration cell.
   3. *A new row's key.* An inserted row appears once, under the key the chart made or the key the result gives, also when the host's fresh data with that row lands before the result does.
   4. *An edit before the create is saved.* A row is inserted and renamed before its save returns; the rename is saved second and names the row's key.
   5. *A refused create with edits behind it.* The server refuses the insert of scenario 4; the row and its rename both go, and one message says two changes were undone.
   6. *A server recalculation.* On the change-set endpoint, whose `save` is the one line of rule 3.2 and returns what the endpoint answers, saving a duration makes the server change the constraint of two rows the person did not touch and return them in `changed`; they update with no undo step, and undo takes back only the person's edit.
   7. *An outside change during a save.* While a save is in flight another user changes the same row; the field the person did not touch shows at once, and the field they did touch waits and ends on the server's value.
   8. *A late result.* Two minutes after a save the server pushes new values for five rows, one of which has an unsaved edit; four update and the fifth keeps the edit until its save settles.
   9. *An old read.* A full read that began before a save, and before a delete, returns after both; the saved value stays and the deleted row does not come back.
   10. *A lost response.* The server stores a create and drops the response; the row stays, marked not saved, the chart calls again with the same `id`, the server answers as it did the first time, and the row appears once with nothing put back.
   11. *A failure part of the way.* On per-record routes the second of three requests of one commit is refused; the chart puts the commit back, the host reads again, and the chart shows what the server kept. When that request is dropped instead, the repeated call goes through the first request again, the server applies it once, and the commit is saved whole.
   12. *Undo across the server.* A task with two links and a column the chart does not know is deleted and the delete undone: the server gets the task create and then the link creates, the column's value is intact, and when the server refuses that undo the task is gone again and the step is back on the undo list.
   13. *The server is away.* It drops every request while the person makes three edits. All three stay on screen, marked not saved; the first is called four times at the default waits and the others not at all; the chart then stops and asks. The server comes back, the person presses "Try again", and the three reach the server once each, in commit order.
   14. *Discard.* As scenario 13, but the person presses "Discard 3 changes": the three are put back, `error` fires once with `reason: "failed"`, `calls: 4` and `undone: 3`, and the host reads again.
   15. *Repeats off.* With `retry: false`, a dropped response puts the commit back at once, `save` has run once, and the host's read shows the row the server kept, once.
   16. *The host's own names.* Every `set` and `was` the server receives uses its own field names, a create carries the new key as `id`, and a row that has both `title` and a `name` member of its own keeps both.
   17. *A server that does not honor the key.* Told to ignore repeated idempotency keys, the fake server refuses the repeated create of scenario 10 under `keys: "chart"`, and the read after the put-back shows one row; under `keys: "server"` it stores two rows and the chart shows two. The suite expects exactly that, as the record of what rule 3.12 warns of.
   18. *Mount from `src`.* The chart mounts with `src` and `save` and no data of its own, shows the server's rows, and the host's code makes no request.
   19. *`src` fails at mount.* The server answers the first read with a 503 and the second with its rows: the chart mounts after one more read. Told to answer 503 to every read, `mount` rejects after four reads at the default waits and `error` fires once with `source: "src"` and `fatal: true`; told to answer 404, after one read.
   20. *A refused save, read again by the chart.* On per-record routes the second of three requests of one commit is refused: the chart puts the commit back, `error` fires once, the chart reads `src` once and shows the first request's write, which the server kept, and no host code runs.
   21. *A 500 that applied nothing.* The server answers a `PATCH` with a 500, applies nothing and stores no answer; the repeat a second later, with the same `Idempotency-Key`, applies it. The commit is saved, nothing is put back, no `error` fires and `src` is not read.
   22. *An expired login.* The server answers a save with a 401: no call follows, the alert line asks at once with both buttons, and no `error` fires. The script then accepts the session again, the person presses "Try again", and the commit is saved once.

### 12. How we will know it worked

| Measure | Target |
|---|---|
| Lines of host code around the chart in the reference host, measured before and after (the headline number) | 100 or fewer, from 659; the study's floor is about 60. No rename function is among them; what rule 8.4 leaves out of scope is counted on its own line |
| State the host holds for the chart: refs, state hooks, queues, diffs | none for saving or syncing |
| Chart members the example touches | 12 props or fewer, and no method on the chart handle in React |
| The example, every file except the fake server | 80 lines or fewer |
| The scenario suite | every scenario of rule 11.2 in both key modes, with the example as rule 11.1 says |
| The host that follows the convention: the URL form of the first section | 5 lines or fewer in React and in plain `mount`, imports included; each scenario that rule 11.1 runs through both forms passes through both, and the server receives the same requests from each |
| A cold run: a fresh coding assistant given only the README and the fake server writes the integration | it passes the suite in each of three runs; lines, errors and turns are recorded for each |

A line count alone invites a toy, so no single row of this table passes the work; all seven do.

### 13. Build order, and what each step breaks for a 0.3.1 host

The order is the outside review's, and this record agrees with it: `save` takes changes, so their shape comes first; `save` without in-place data cannot be used, so in-place data comes before it; undo is built last, and its contract is decided here, before `save`, because an undo reaches `save`. Two additions: the fake server and the suite land first, failing, and steps 1 to 3 ship in one release, 0.4.0, because any one of them alone leaves the host writing today's glue.

| Step | What ships | Covered by | What breaks for a 0.3.1 host |
|---|---|---|---|
| 0 | The fake server, the scenario suite and the example, failing | none | nothing |
| 1 | Record-shaped changes (section 2), one unit, `dateForm: "day"`, the field map and custom fields (rules 8.3 and 8.4) | no row found; `tl2090dd` covers the displayed form only; the field map: not searched | `edit` commits and the before-edit payload change from `field`, `old`, `new` to `set`, `was`; a host mounted with hours or minutes gets durations in its unit, not days; an unknown member of a task or link is kept as a custom field, not left out and counted |
| 2 | Data in place by key (section 4): props, `setData`, `update` with every input field, links and deletes; `capabilities`, `pending` and `selectedKey` as props | `tl2087cp`, `tl2087uc`; links, creates and deletes by key: no row found | a changed `tasks` or `links` prop no longer starts the chart again; a host that relied on that sets a React `key` or calls `load` |
| 3 | `save`, the queue, repeated calls and `retry`, `SaveStopped`, put-back, `keys`, `changed`, `SaveError`, `state().unsaved` and `state().save` (sections 3 and 5); the URL default (section 10): `src`, a `save` string and `headers`, with the convention in the README | `tl2091sq`; its engine half is merged; the URL default: not searched | nothing without `save` or `src`; `Gantt.save()` is renamed `download()` |
| 4 | The typed constraint and `mandatoryDates` (section 9), then the profile values and snapping (rule 8.2) | no row found for the constraint; `tl2090cq`, `tl2090sn`; the `structure` split: no row found | `editable.structure` becomes `rows`, `outline` and `paste`; at 0.4.0 changes write `constraint` |
| 5 | Undo (section 6): `history`, `canUndo`, `canRedo`, stale steps, host-only entries | issue tl#2092; no row found | nothing under the default |

`Owner decided:` the URL default is built in step 3 with `save`, as a thin layer over the same queue (rule 10.1), not after it. The convention ships where the first section does: the package README's "Connect to your backend" section is the first section, unchanged (rule 1.1), and the README is the package's own documentation (decision 0016, rule 6), so a host's coding assistant finds it in the installed package.

The study's R8 (asking `beforeEdit` again at a drag's release) is dropped (rule 7.5).

### 14. Not in this record

- **Resources and assignments** in host data. A task keeps its display-only `resources` list (decision 0017).
- **Column configuration:** which columns, their order and width, cell renderers, and columns for custom fields.
- **Nested, computed and converted fields in the field map,** and ids that are numbers (rule 8.4).
- **Any transport but the convention.** The chart calls `fetch` only for `src` and a `save` string (section 10). Other routes, a change-set endpoint, GraphQL or a socket are the host's `save` function.
- **Merging queued commits** into one call, and an **offline queue.** Unsaved commits live in the page's memory and are lost with the page (rule 3.9).
- **Versions and merging between people.** No revision numbers, no ETags, no operational transforms. The server refuses in `save` (rule 4.10).
- **Calendar edits through `save`.** A person cannot edit a calendar in the chart today.
- **Custom fields in the document format.**
- **Comparing a server's computed dates with the chart's** (`tl2087hd`).

## Consequences

- Decision 0019, rule 7 keeps the commit and its `seq` and `source`; the change inside it takes the shape of section 2. Rule 10's queue is built as section 3 says, and `assignKey` becomes something the queue calls, not the host.
- Decision 0016, rule 7 stands for a chart with no `save`. With `save`, a row is keyless only between its create and that create's settle under `keys: "server"`, and never under `keys: "chart"`.
- Decision 0017, rule 9 changes for a task or link in host data: an unknown member is a custom field (rule 8.3). An unknown top-level or `options` member is still refused.
- `docs/WASM_API.md` (`App.take_commits`) and the README's `update` paragraph state one unit after step 1.
- `ErrorSource` gains `"save"` and `"src"`. The README gains "Connect to your backend", which is the first section of this record, the convention included.
- The React, Vue and Angular components stop listing `tasks` and `links` among the props that start the chart again, and list `src` (rule 10.3).

## What could not be verified

- Bryntum's own documentation pages did not open (the guide and API pages returned a 404 or a site overview). That its sync is one batched request with per-store `added`, `updated` and `removed`, and that a created record is answered with `$PhantomId` and `id`, is verified from a tutorial on the vendor's blog [S2] and the vendor's skill file [S3]. Whether a sync response may carry other rows the server changed, and what the component does after a failed sync: not verified. So rule 3.6's repeated calls and their waits are this record's own choice, checked against no other component; whether DHTMLX or Syncfusion repeat a failed request was not read either.
- Whether a backlog row already covers the field map or the repeated calls was not searched.
- Whether deleting a summary task deletes the tasks under it today was not checked. Rule 2.7 says how those deletes are listed when one delete takes other tasks with it.
- What P6 and MS Project do with a mandatory date when logic pushes past it (rule 9.4) is from memory: not verified.
- Nothing in this record was run. Every statement about today's behavior comes from reading `origin/main` at `45d86f2`.

## Rows this governs

`tl2091sq` (section 3), `tl2087cp` and `tl2087uc` (section 4), `tl2090cq` and `tl2090sn` (rule 8.2), issue tl#2092 (section 6), and the rows still to be written for steps 0, 1 (the field map and custom fields included), 2 (links, creates and deletes by key), 3 (repeated calls, `SaveStopped` and the URL default) and 4 (the typed constraint, the `structure` split).

## Sources

- [S1] DHTMLX Gantt, server-side integration: https://docs.dhtmlx.com/gantt/guides/server-side/
- [S2] Bryntum blog, connecting Bryntum Gantt to a planning backend (2025-02-21): https://bryntum.com/blog/how-to-connect-and-sync-bryntum-gantt-to-a-servicenow-project-portfolio-management-planning-chart/
- [S3] Bryntum skills repository, `bryntum-crud`: https://raw.githubusercontent.com/bryntum/skills/refs/heads/main/bryntum-crud/SKILL.md
- [S4] Syncfusion Gantt, data binding: https://help.syncfusion.com/gantt-sdk/typescript/gantt-chart/data-binding
