# The host save contract

Status: proposed, 2026-11-18. A draft for the owner to read and correct before anything is built; none of it is built. A line marked `Owner decided:` is one of the owner's sixteen decisions of 2026-11-18, in four groups: four on the save contract (who makes keys by default, what a network failure does, the field map with custom fields passing through, one `save` call per commit); four on the URL default (a base URL is the default and there is no separate helper, the route convention is Tachlon's, the chart reads the schedule from `src`, the URL default is built with `save`); three that followed (a 401 is not a refusal and is never repeated by itself while a 403 is a refusal, as decided at 11:10 in place of 10:55's "a 401 or a 403"; one `headers` option; `src` beside `tasks` or `links` is refused); and five on the forms, at 12:05 (handlers are the first form, repeat safety is per request, the URL form is second, the `save` function is third, and an event per saved commit for a host that owns its undo is left to a later record). A line marked `Decided here:` is the drafter's choice where the study, the outside review and the adversarial read (see Context) disagree or leave a point open. Once accepted it amends [decision 0016](0016-host-identity.md) (rules 7 and 8), [decision 0017](0017-host-record-vocabulary.md) (rule 9 and its closing note on mandatory dates) and [decision 0019](0019-commit-pipeline.md) (rules 7, 8, 10 and 12).

A *host* is the product that embeds the chart. This record designs the host's side first: the code a developer, and their coding assistant, writes to connect the chart to a backend. Every rule after the first section exists to make that code true.

## Connect the chart to your backend

There are three forms, one for each kind of backend: the *handlers form* for an API that already has a client, the *URL form* for a backend written to Tachlon's convention, and the *function form* for one request per commit. Each is a value of `save` (rule 3.1), and every rule after this section applies to all three unless it names one.

### An API that has its own client

For a product that already has an API and a client for it, which is most hosts. `Owner decided:` this is the first form. In React:

```tsx
import { Gantt, type Handlers } from "tachlon/react";
import "tachlon/gantt.css";
import { api, type Task, type Link } from "./api"; // your client: it sends c.once and c.signal, and throws on a non-2xx with a numeric `status`

const fields = { // the chart's names on the left, your API's on the right; any other member of a row comes back untouched
  tasks: { key: "id", name: "title", start: "plannedStart", duration: "days", parent: "parentId" },
  links: { key: "id", from: "source", to: "target" },
};

const save: Handlers = { // one call into your API per change, in the order the person made them
  createTask: (row, c) => api.createTask(row, c),
  updateTask: (key, set, c) => api.updateTask(key, set, c),
  deleteTask: (key, c) => api.deleteTask(key, c),
  createLink: (row, c) => api.createLink(row, c),
  updateLink: (key, set, c) => api.updateLink(key, set, c),
  deleteLink: (key, c) => api.deleteLink(key, c),
  updateSchedule: (set, c) => api.updateSchedule(set, c),
};

export function Schedule({ tasks, links, reload }: { tasks: Task[]; links: Link[]; reload: () => void }) {
  return <Gantt tasks={tasks} links={links} fields={fields} dateForm="day"
    save={save} onError={(e) => e.source === "save" && reload()} style={{ height: 480 }} />;
}
```

With no framework, the same `fields` and `save`:

```ts
import { mount } from "tachlon";

const chart = await mount(root, { ...(await api.readSchedule()), fields, dateForm: "day", save });
chart.on("error", async (e) => { if (e.source === "save") chart.setData(await api.readSchedule()); });
serverEvents.onmessage = (m) => chart.update(JSON.parse(m.data)); // every row the server changed, this page's own saves included (rule 4.9)
```

- **Throw when the server says no.** The chart reads a numeric `status` from 100 to 599 on what a handler throws (or its `response.status`, or `statusCode`; a 0, as a browser client reports a lost connection, is no status): a 401 asks the person to sign in again, a 408, a 429 or a 5xx is repeated, and any other 4xx puts the edit back with the error's `message` (rule 3.5). A throw with no status is repeated. A client whose errors carry no status adds `classify(error)` to the handlers to return one.
- **A server that makes ids** mounts with `keys="server"`. Its create returns the stored row, and the chart reads the new key from it and puts it in every later change that names the row (rule 3.1). Otherwise the chart makes the key, and the create's `row` carries it, here as `row.id` (rule 5.3).
- **`reload` reads your data again** after the chart has put edits back, so the screen shows what the server kept (rule 3.7). An `error` whose `source` is `"save"` fires only then.
- **The second argument,** `c`, carries `once`, the request's idempotency key; `signal`, aborted after 10 seconds; `was`, the old values; and `changeSet`, the whole commit (rule 3.1). A client that cannot send `once` leaves `c` out, and rule 3.12 says what is then repeated.

### A backend you are writing now

For a backend that you, or your coding assistant, write now: the chart reads and saves through Tachlon's routes, and you write no client. `Owner decided:` this is the second form.

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
| `POST /api/tasks`, `POST /api/links` | a create's `set`: the whole row; under `keys: "chart"` it holds the key the chart made, which the server stores as the row's key and returns in the `GET` (rule 5.3) | the stored row; under `keys: "server"` it holds the key the server made (rule 10.5) |
| `PATCH /api/tasks/:key`, `PATCH /api/links/:key` | an update's `set`: the members that changed | 204, or the stored row |
| `PATCH /api/schedule` | the `schedule` change's `set` | 204, or the stored members |
| `DELETE /api/tasks/:key`, `DELETE /api/links/:key` | none | 204 |

- Names are the chart's own (decision 0017's, and `key`), or the host's where `fields` maps them (rule 8.4). Durations are in `durationUnit` and dates in `dateForm` (decision 0017, rules 5 and 7). A reference holds a key (rule 2.5). A key is a string (decision 0016, rule 3); a server with number ids sends them as strings. `:key` is the row's key, URL-encoded. A body is JSON and is sent with `content-type: application/json`, and a request with no body sends no such header; a 2xx other than 204 answers JSON.
- The chart deletes a task's links before the task, one `DELETE` each (rules 2.6 and 3.1), so a server need not drop a deleted task's links itself.
- Every request but the `GET` carries `Idempotency-Key`, the change's `once`: `<change set id>:<n>`, `n` being the change's position in its change set, from 0. A request with a key the server applied is answered as that one was, for at least 24 hours (rule 3.12).
- A 4xx other than a 401, a 408 or a 429 is a refusal, a 403 included, and answers `{ message, key?, field? }`; the chart puts the commit back (rule 3.5).
- A 401 says the person must sign in again. It judged nothing: the chart keeps the edits and asks the person at once, with no automatic call (rule 3.6; under `retry: false`, put back: rule 3.12).
- A 408, a 429 or a 5xx says the server applied nothing. The server stores no answer for it, so the repeat runs the request again (rule 3.6; under `retry: false` there is no repeat and the commit is put back: rule 3.12). A server that applied a request answers it with a 2xx.

A `save` string is the handlers form over `fetch` (rule 10.1). The host writes two URLs and no fetch, no state and no reload handler: the chart reads `src` again after a put-back (rule 10.3).

### One request per commit

For a host that saves a whole commit at once: a batch route, a transaction, or a command stack of its own. `Owner decided:` this is the third form. `save` is a function that gets the commit's change set (section 2); this one sends it to the convention's routes, one request per change:

```tsx
import { Gantt, type Save } from "tachlon/react";
import "tachlon/gantt.css";
import type { Task, Link } from "./api-types"; // the host's own rows

const fields = {
  tasks: { key: "id", name: "title", start: "plannedStart", duration: "days", parent: "parentId" },
  links: { key: "id", from: "source", to: "target" },
};

const routes = { task: "/api/tasks", link: "/api/links", schedule: "/api/schedule" };
const methods = { create: "POST", update: "PATCH", delete: "DELETE" };

const save: Save = async ({ changes }) => {
  for (const c of changes) {
    const url = c.op === "create" || c.record === "schedule" ? routes[c.record] : `${routes[c.record]}/${encodeURIComponent(c.key!)}`;
    const headers = { ...(c.set ? { "content-type": "application/json" } : {}), "idempotency-key": c.once };
    const res = await fetch(url, { method: methods[c.op], headers, body: JSON.stringify(c.set), signal: AbortSignal.timeout(10_000) });
    if (!res.ok) throw res; // the chart reads the status, as for a handler: a refusal, an expired login, or a failure it repeats
  }
};

export function Schedule({ tasks, links, reload }: { tasks: Task[]; links: Link[]; reload: () => void }) {
  return <Gantt tasks={tasks} links={links} fields={fields} dateForm="day"
    save={save} onError={(e) => e.source === "save" && reload()} style={{ height: 480 }} />;
}
```

- The chart reads a thrown `Response` as it reads a handler's throw, and a refusal whose body is not JSON still shows a message (rule 3.5).
- A repeat calls `save` again with the same change set, and the loop goes through every request again; `once` makes the server answer the ones it applied as before (rule 3.12).
- A batch endpoint is one line, and under `keys: "server"` a loop like this one also replaces each `{ ref }` and returns the keys (rule 3.2).

In every form the host writes no rename function, no save queue, no retry, no put-back, no diff and no "previous value", and the rows are as the API returns them (rule 8.4); only the function form under `keys: "server"` keeps keys itself. A lost connection puts nothing back: the chart keeps the edits on screen, calls `save` again, and then asks the person, and an expired login asks at once (rule 3.6; rule 3.12 names the change sets that are put back instead), so an error whose `source` is `"save"` always means edits were put back. An API whose updates cannot take the same request twice mounts with `retry={false}`, and then every failure, an expired login included, is put back at once (rule 3.12).

## Context

Embedding the chart in one real product took 659 lines of host code. A study of those lines found that a third (216) exist because the page API has no save function, no put-back and no in-place data update, and another third (224) translate vocabulary: per-field changes folded back into per-record saves, dates cut to a day, six limit fields mapped to one typed constraint. The pieces exist below the page API: the `edit` event carries commits (decision 0019, rule 8), the engine can take commits back (`App.rollback_to` and `App.acknowledge` in `docs/WASM_API.md`), and `update(records)` sets five members by key. The React component starts the chart again when `tasks` or `links` changes (`packages/gantt/src/react.tsx`), so a host cannot pass its own state.

The three components this one is compared with all own the save path. DHTMLX's data processor sends one request per added, updated or deleted record and takes the new id from the response (`tid`) [S1]. Bryntum's project sends one request per sync with every change in it and maps a client-made `$PhantomId` to the server's id [S2, S3]. Syncfusion binds a `DataManager` with an optional batch URL [S4]. None of the pages read asks the host to turn change reports into save calls. An outside review of the plan asked for the contract to be written before any of it is built, with type signatures, and tested against a fake server that misbehaves. An adversarial read of this record's first section (2026-11-18) found that most hosts have an API client of their own, so a function over the whole change set makes them write a dispatcher over `record` and `op` and the chart's status policy into their code; the first section now leads with handlers.

## Decision

Terms used below. *Host data* is the `tasks` and `links` the host gives the chart, or that the chart reads from `src` (rule 10.3). A *commit* is one undo step (decision 0019, rule 1). A *call* is one run of `save`; one change set may be called more than once (rule 3.6). A commit's save *settles* when a call resolves with a result the chart can use (*saved*) or when the chart puts the commit back (*put back*: rules 3.5, 3.6, 3.8 and 3.12). A call that fails and will be repeated settles nothing. A commit is *unsaved* from the moment it is made until its save settles: *queued* before its first call, and *in flight* from that call to the settle, the waits between calls included. A field is unsaved while an unsaved commit writes it; a row is unsaved while an unsaved commit creates or deletes it. Rules name fields by the chart's names; rule 8.4 says where a host sees its own.

### 1. The target

1. **The first section's handlers form is the contract's test.** A change to this record that makes that code longer, or adds state the host must hold, needs a reason stated beside it; rule 12 holds it to 20 lines. The example app (rule 11) is the first section's code, all three forms, against the fake server, and the README's "Connect to your backend" section is the first section, unchanged.

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

interface ChangeSet { id: string; seq: number; source: CommitSource; changes: readonly (Change & { once: string })[] } // id: a UUID, new for every change set, the same on every call of it
// once: the change's idempotency key, `${id}:${n}`, n its position in `changes` from 0 (rule 3.12); only a change set carries it
```

1. **One change per record.** A commit that sets three fields of one task carries one change with three members in `set` and the same three in `was`. A record appears at most once in a change set. This replaces the one-change-per-field shape of decision 0019, rule 7 (`field`, `old`, `new`), because a host saves records, and folding fields back into records was most of the 224 vocabulary lines.
2. **`set` and `was`.** An update carries only the members that changed. A create carries the whole record and no `was`: every input member, the key under `keys: "chart"` (rule 5.3) and the row's custom fields (rule 8.3). A delete carries the whole record in `was`, its key included, and no `set`, so a host that keeps an audit trail or its own undo has what it removed. Member names in `set` and `was` are the host's (rule 8.4). `was` is the field's *server value*, read when the change set is written (rule 3.3). The chart keeps one server value per field, and it is whichever of these came last: a value that host data or `update` gave the field and the chart applied; a value that the `changed` of one of this chart's own saves gave it, from the moment that result arrives, applied or not yet; the `set` of the last saved commit that wrote it (a `changed` that sets the field comes after the `set` of the save it answers). A value that is held (rule 4.3) is not the server value until it is applied. So a server that compares `was` with its own row (rule 4.10) does not refuse an edit because of a change it made in answer to this chart's own save, and it does refuse an edit made over an outside change the person never saw.
3. **One unit, the one the host mounted with.** Durations and lags in `set` and `was` are in `durationUnit`. This fixes a stated inconsistency: `docs/WASM_API.md` (`App.take_commits`) says a commit's duration is "a number of days of the task's calendar", while `update` reads `durationUnit`. Decision 0017, rule 5 wins: one unit, in and out, on every surface. A host that mounts with hours gets hours.
4. **Dates in the form the host chose.** Under `dateForm: "day"` every date in a change is `YYYY-MM-DD`, converted as decision 0017, rule 7 says (a finish-kind date is the day of its instant less one minute). `was` uses the same form, so a host compares it with what it stored.
5. **References are keys.** `parent`, `after`, `from`, `to` and `calendar` hold the other record's key. The one exception is a row whose create in the same change set carries a `ref` (`keys: "server"`, the undo of a delete included, rule 5.6): every reference to it in that set is `{ ref }`, an object, so it cannot be read as a key and it follows whatever key the server answers. A change set never holds a session slot. `Decided here:` this replaces `{ task: slot }` at the save boundary, because a slot is valid only inside one session and a host that stored one would be wrong after a reload.
6. **Order inside a change set is the order a server can apply top to bottom.** A create comes before any change that names its row (a parent before its children, both tasks before the link that joins them), a link delete comes before the delete of a task it touches, and a child's delete comes before its parent's.
7. **A link delete that follows from a task delete is marked `cascade: true`, and nothing else is.** Deleting a task deletes its links (decision 0017, "A link"); each of those link deletes carries the mark. The handlers form and the URL form send every one (rule 3.1), which is correct on any database because rule 2.6 puts them first; a `save` function whose database drops a deleted task's links may skip marked changes. When one delete takes other tasks with it (a summary task and the tasks under it), each of those is an ordinary delete with no mark, so a host that skips marked changes still deletes every task and its server never holds a child of a deleted parent. The mark is only ever on a link's delete: the undo of that delete lists the links as ordinary creates, because the host must make them again.
8. **The `edit` event and the before-edit payload carry the same changes,** with the session slot added (`task` or `link`), because a chart with no `save` may hold rows with no key. A chart with `save` never needs the slot. Both are written when the commit is made, so their `was` is the server value at that moment; the change set that `save` gets is written later (rule 3.3), and its `was` and keys may differ.

### 3. `save`

```ts
type Save = (changeSet: ChangeSet) => Promise<SaveResult | Response | void>;

type Row = Record<string, unknown>; // a record in the host's names (rule 8.4), every { ref } in it replaced by a key

/** The second argument of every handler (rule 3.1). */
interface SaveContext { once: string; signal: AbortSignal; was?: Row; changeSet: ChangeSet }

/** `save` as one function per kind of change, each one call into the host's client (rule 3.1). */
interface Handlers {
  createTask?(row: Row, c: SaveContext): Promise<unknown>;
  updateTask?(key: Key, set: Row, c: SaveContext): Promise<unknown>;
  deleteTask?(key: Key, c: SaveContext): Promise<unknown>;
  createLink?(row: Row, c: SaveContext): Promise<unknown>;
  updateLink?(key: Key, set: Row, c: SaveContext): Promise<unknown>;
  deleteLink?(key: Key, c: SaveContext): Promise<unknown>;
  updateSchedule?(set: Row, c: SaveContext): Promise<unknown>;
  classify?(error: unknown): number | "refused" | "stopped" | "failed" | undefined; // rule 3.5
}

/** Rows the server changed: whole or partial records by key, and keys it deleted. */
interface ServerChanges { tasks?: readonly TaskRecord[]; links?: readonly LinkRecord[]; deleted?: { tasks?: readonly Key[]; links?: readonly Key[] } }

interface SaveResult {
  keys?: readonly { ref: string; key: Key }[]; // keys: "server" only: exactly one for each create that has a ref
  changed?: ServerChanges;                      // applied by key, with no undo step and no save call
}

/** Throw this when the server judged the change and said no in an answer that carries no status (rule 3.5). */
class SaveRefused extends Error { constructor(refusal: { message: string; key?: Key; field?: string }) }

/** The `error` event of a put-back; it fires once per put-back, after it. `calls`: how many times `save` ran for the change set.
 *  `reload`: the chart could not put itself back, so its next data update replaces everything (rule 3.11). */
interface SaveError { source: "save"; message: string; fatal: false; reason: "refused" | "failed" | "result"; calls: number;
  key?: Key; field?: string; seq: number; undone: number; reload: boolean }

type Retry = readonly number[] | false; // the waits before each repeated call, in milliseconds; default [1000, 3000, 9000] (rule 3.6)
```

1. **`save` is a mount option and a prop: a `Handlers` object, a base URL (rule 10.1) or a `Save` function.** A string is the URL form, a function the function form, and any other object the handlers form. `CommitSource` is `typed`, `drag`, `dialog`, `paste`, `proposal`, `assistant`, `host`, `undo` or `redo`; a `rollback` commit is never sent. A commit with no changes for the host is not sent. With no `save`, the chart behaves as 0.3.1 does and the host reads the `edit` event. Each call uses the `save` the chart holds at that moment, so a `save` that is a new function or object on every render is safe; `keys`, `retry`, `idempotent` and which handlers exist are read once, at start, and a change whose handler a later object lacks fails (rule 3.6).
   - *The handlers form is a `Save` function the chart builds,* so every rule of this section applies to it: one call per commit, one call in flight. That function takes the changes in order (rule 2.6) and calls one handler for each, after the one before has resolved: `createTask` or `createLink` with the create's `set`; `updateTask` or `updateLink` with the key and `set`; `deleteTask` or `deleteLink` with the key; `updateSchedule` with `set`. The second argument holds the change's `once` and `was`, the change set, and a `signal` that the chart aborts 10 seconds after the handler is called; the chart still waits for the handler to settle (rule 3.6, *No timeout*). The call resolves when the last handler has; it ends at the first handler that rejects, or that resolves with a `Response` whose status is not a 2xx, and is judged by that answer (rule 3.5).
   - *References.* Before a handler is called, every `{ ref }` in its row is replaced by the key that row's create answered, and the chart returns `SaveResult.keys` itself. A handler never sees a `ref`.
   - *A create's key,* under `keys: "server"`, is read from what its handler resolves with: the stored row, an object that holds the key under the host's name for `key` (rule 8.4); a 2xx `Response` whose JSON body is that row; or the key itself, a string. Anything else is a result the chart cannot use (rule 3.8). In every other case what a handler resolves with is not read, except a `Response`'s status (rule 3.5); rows the server changed reach the chart through `update` or a `save` function's `changed` (rule 3.4).
   - *Link deletes.* Every link delete is sent, marked `cascade` or not, before the task delete it follows (rule 2.6), so it succeeds on any database and `cascade` is not the host's concern.
   - *A repeat resumes at the change that failed:* the changes whose handlers resolved on an earlier call of the same change set are not sent again (rule 3.12). The chart keeps that point with the change set, not with the object.
   - *A missing handler.* A commit that holds a change whose handler is missing is refused at the gate, before the edit shows, as a capability refusal is (rule 7.4), and the gestures that always make such a change are turned off: with no `createLink`, no link can be drawn. At mount the chart logs one console warning that names each missing handler and the edits it turns off.
   - *A refusal that resolves.* A client that reports an error by resolving with it is read as saved (rule 3.4), and no status rule can catch it. So a handler that resolves with an object that has an `error`, `errors` or `userErrors` member logs one console warning per handler. `Decided here:` in every build, once per session.
2. **One call per commit, with the whole change set.** `Owner decided:` as drafted, not one call per record. The commit is the unit the chart can put back, so it must be the unit the host saves: a commit saved in three calls that fails on the second has no clean way back. A host coming from a batched library (Bryntum sends every store's `added`, `updated` and `removed` in one request [S2]) posts the change set to one endpoint, which saves it in one transaction and answers the `SaveResult`: `const save: Save = (cs) => fetch("/api/changes", { method: "POST", headers: { "content-type": "application/json", "idempotency-key": cs.id }, body: JSON.stringify(cs) })`. A host coming from a per-record library (DHTMLX sends one request per record [S1]) uses handlers, which the chart runs one per change inside the one call. A `save` function that loops over `changes` itself, as the first section's function form does, under `keys: "server"` also keeps a map from each `ref` to the key its create answered, replaces each `{ ref }` it meets later in the same set, and returns the keys: about five lines, which handlers, a change-set endpoint and chart-made keys never need.
3. **One save in flight, in commit order.** The person keeps editing while a save runs; those commits queue. The chart calls `save` for the next commit only after the one before is saved and its keys have been assigned (the rest of its result is applied in the same step, or when a gesture ends: rule 3.10), and it never has two calls running: a change set is called again only after its last call failed. A change set is written at its first call, not when the commit is made, so it names every key that earlier saves assigned; a repeated call passes the same change set, unchanged. `seq` rises with every commit and is never reused in a session; the gaps a `rollback` commit leaves are expected. Queued commits are not merged into one call (rule 14).
4. **Resolving with a 2xx, or with anything that is not a `Response`, means saved.** A result is read for `keys` and `changed` only. A `Response` is first read for its status (rule 3.5); a 2xx one is read as the result if its body is JSON, and as a result with neither member if it is not (`Decided here:` so rule 3.2's endpoint returns its answer as it comes). Anything else `save` resolves with is ignored, so it may resolve with whatever its endpoint answers. With neither member, the chart takes it that the server holds the change as sent. A server that changes more than it was sent says so in `changed`; a `save` that returns nothing, as the first section's forms do, leaves those changes to host data or `update` (rule 4.9). `keys` assigns server-made keys (rule 5). `changed` is what the server changed beyond what it was sent: rows it recalculated, a name it trimmed, a row a rule of its own deleted. The chart applies `changed` by key with no undo step, no `save` call and no before-edit call, by the rules of section 4.
5. **The chart reads what an answer means from its status, and a refusal puts back at once.** The status is the first number from 100 to 599 found: the `status` of a `Response` that is thrown, or that `save` or a handler resolves with; else a thrown error's `status`, `response.status` or `statusCode`. A 0, or any other number outside that range, counts as no status. This table is the chart's behavior in every form, stated here only; the first section's list says what a convention server sends.

   | Status | Resolved | Thrown |
   |---|---|---|
   | 2xx | saved (rule 3.4) | a failure: an error that says 200 is not a save |
   | 401 | the chart stops and asks the person (rule 3.6, *An expired login*) | the same |
   | 408, 429, 5xx | a failure, repeated (rule 3.6) | the same |
   | any other, a 403 included | a refusal | the same |

   The order is: a thrown `SaveRefused` is a refusal, whatever else it carries; then a handlers object's `classify(error)`, asked for each other throw: a number from 100 to 599 is judged by the table and any other number is no status, `"refused"`, `"stopped"` and `"failed"` are those outcomes, and `undefined` leaves the error to the next step; then the status, by the table; then a throw or a resolved `Response` with no status is a failure (rule 3.6). A 404 or a 410 counts as saved only for a delete whose request was sent on an earlier call of the change set (rule 3.12); to a delete's first request it is a refusal. A refusal's `message` is the `SaveRefused`'s, a thrown error's `message`, or the string `message` of a `Response`'s JSON body; a body that is not JSON or has no `message` gives "The server refused the change" and the status. `key` and `field` come from the `SaveRefused` or the body; in the handlers form a refusal with no `key` takes the key of the change whose handler was refused, for a task's change only; a refused link selects no task, whatever key it names, and in a `Save` function `key` names a task. `Decided here:` there is no `SaveStopped` class: a status of 401, or `classify` returning `"stopped"`, says the same.

   A *put-back* restores the state before that commit, takes back every commit queued behind it as well (they were made on top of it; decision 0019, rules 10 and 11), fires `error` with a `SaveError`, and shows `message` in the chart's alert line with the number of commits undone. Here `reason` is `"refused"`; for a `key` the chart selects that row, and for a `field` (the host's name for it) it shows the message at that cell. A refused change set is never called again. Rows whose commit is unsaved are dimmed as `setPending` dims them, by the chart, until the commit settles, or until a call fails and rule 3.6's mark replaces the dim on every one of them.
6. **A failure puts nothing back.** `Owner decided:` a lost connection, a timeout or a 5xx says nothing about the change, so the chart keeps every unsaved edit on screen and calls `save` again with the same change set (rule 3.12 says what that asks of the server, and which creates are put back instead). `Decided here:` the details.
   - *The calls.* Up to three more: the first 1 second after the failure, the second 3 seconds after its own failure, the third 9 seconds after its own. That is `retry: [1000, 3000, 9000]`, the default. A host passes its own waits; `[]` means no automatic call, and the chart asks the person after the first failure.
   - *What the person sees.* From the first failure, every row an unsaved commit touches carries a "not saved" mark where it was dimmed, and the alert line reads "Not saved yet. Trying again." Both go when the commit is saved.
   - *Editing goes on.* New commits queue behind the failed one as always. None of them is called before it is saved, so order holds, and they are put back only if it is.
   - *When the last call fails,* the chart stops calling. The alert line reads "Not saved: " and the message, with two buttons: "Try again" and "Discard N changes", N being the number of unsaved commits, kept current. It stays until the person chooses, and editing still goes on; rule 3.12 says what changes after 24 hours. "Try again" calls once more and, if that fails, runs the waits again. "Discard" is the put-back of rule 3.5 for the failed commit and all behind it, with `reason: "failed"`.
   - *An expired login.* `Owner decided:` a 401 (not signed in) is not a refusal, and a repeat cannot fix it until the person signs in again. A 403 (signed in, not allowed) is a refusal, put back at once with the server's message (rule 3.5). On a 401, or `classify` returning `"stopped"` (rule 3.5), the chart makes no automatic call and goes straight to the stop above: the mark, "Not saved: " and "Sign in again, then press Try again.", and both buttons. "Try again" calls once and, if that meets a 401 again, stops again at once; it works once the person has signed in. Under `retry: false`, and for a `save` function's change set that rule 3.12 puts back on any failure, it is put back (rule 3.12).
   - *For the host.* No `error` fires while the chart is repeating or stopped: `error` with `source: "save"` always means edits were put back (until this stop and its buttons exist, a `save` that throws other than a refusal stops the queue and fires one `error` with `reason: "stopped"` and `undone: 0`). `chart.state().save` is `"idle"`, `"saving"`, `"retrying"` (a call failed and another will follow) or `"stopped"` (the chart is asking the person); a host with its own controls calls `chart.commands.retrySave()` and `chart.commands.discardUnsaved()`, which do what the buttons do and do nothing in any other state.
   - *No timeout.* The chart never gives up on a call that is running; a promise that never settles holds the queue, and `unsavedCommits` shows it. A `save` that should give up aborts its own request and throws, as the first section's function form does; a handler passes its `signal` to its request (rule 3.1).
7. **After a put-back, the host reads its data again.** The chart has put the edits back; the host's data then says what the server kept. This covers a discard after a lost response (the server did save: the edit comes back with the fresh data) and a per-record loop refused part of the way (the server holds half: the fresh data shows it). The first section's handlers and function forms do this in `onError`; with `src` the chart does it (rule 10.3).
8. **A result the chart cannot use is put back, not repeated.** A missing, extra or duplicate entry in `keys`, a create handler's answer with no key in it under `keys: "server"` (rule 3.1), a key another row holds, `keys` under `keys: "chart"`, or a record in `changed` that the validator refuses: the chart puts back as rule 3.5 does, with `reason: "result"` and a message that names the mistake. Calling again would get the same answer. The server may hold the rows; the host's fresh data shows them.
9. **Unsaved work at teardown.** `chart.state().unsavedCommits` is the number of unsaved commits, read from the queue; `unsaved` is the file mark. `chart.saved()` resolves when every commit that was unsaved at the call has been saved, at once if there was none, and rejects with the `SaveError` if one of them is put back. `destroy()`, and so a React unmount, does not wait: a call that is running finishes and its result is ignored, no further call is made, and every other unsaved commit is dropped. A host that must not lose them warns on leave while `unsavedCommits` is above 0, or awaits `saved()`.
10. **A settle never waits for the person and never changes a value under their hands.** If a put-back comes while a cell is being edited or a bar is held, the chart cancels that gesture first. A failed call that will be repeated cancels nothing. If a save succeeds during a gesture, keys are assigned at once and the next commit is called; `changed`, held values and waiting rows (section 4) are applied when the gesture ends. If more than one save settled during the gesture, their deferred steps are applied in settle order, and each is judged as it would have been at its own settle: a field that was unsaved when commit 1 settled, because commit 2 had changed it, is still held against commit 1's `changed` (rule 4.6) even though commit 2 has since been saved, and commit 2's own step then decides it. A saved edit is never replaced on screen by an older answer.
11. **When a put-back is due and the chart cannot do it** (the history no longer holds the commit after 200 steps, or the chart restarted on WebGL2 since the commit was made), `SaveError.reload` is true, the queue and the history are emptied, and the next data update replaces everything the chart shows: every member applies and a row it lacks is removed (rule 4.5). The host does what it does after any put-back, as in the first section; it needs no other call. `Decided here:` the method `Gantt.save()`, which downloads the document, is renamed `download()` in the release that adds the `save` option, so that "save" has one meaning.
12. **A repeat must leave the server as one successful call would, and the chart repeats only what can.**
    - *The idempotency key.* Each change's `once`, `<id>:<n>` (rule 2), is its request's key: a handler gets it in its second argument, and per-record routes send it, as the first section does. A change-set endpoint uses the change set's `id`. A server that has applied a key answers a repeat as it did the first time, with the same `keys` and `changed`, before any other check; the `was` comparison of rule 4.10 would otherwise refuse the server's own earlier write. It keeps answers for at least 24 hours, because a person may press "Try again" hours later, and a repeat that arrives while the first is still being applied waits for it. `Decided here:` the chart holds up its end: it never calls a change set more than 24 hours after that set's first call. The 24 hours are wall-clock time (`Date.now()`), read at each call and at each press of "Try again", never a timer: a timer stops while the machine sleeps, and the server's stored answers do not. Past that, "Try again" and `retrySave()` are gone, the alert line says the changes are too old to send again, and "Discard" is the one way on. `seq` is not a key: it starts again at 1 in every session.
    - *How a repeat runs.* In the handlers form, and so in the URL form, a repeat resumes at the change that failed (rule 3.1), so only that request is in doubt. A `save` function gets the same change set again and goes through every request; the server answers each one it applied as before.
    - *Repeat safety is per request.* `Owner decided:` in place of "no idempotency key means `retry: false` for everything". An update and a delete are repeated as rule 3.6 says. A 404 or a 410 counts as saved only for a delete whose request was sent on an earlier call of the change set; to a delete's first request it is a refusal, so a wrong route shows. A create is repeated under `keys: "chart"`, where the repeat carries the same key, and under `keys: "server"` only when the host mounts with `idempotent`, which says its server honors the key; a `save` string always has it, because the convention requires it. Otherwise a create that fails with no answer (a network error or the 10-second limit), with a 5xx, or with `classify` returning `"failed"` is put back at once with `reason: "failed"`; one that fails with a 401, a 408 or a 429 is handled as rule 3.6 says, because those statuses come before a server applies anything.
    - *In each form.* The handlers form and the URL form judge the change that failed, as above. A `save` function is judged per change set: a set that holds a create under `keys: "server"`, mounted without `idempotent`, is put back at once on any failure, a 401 included, and every other set is repeated. The chart cannot tell which of a function's requests answered, so the 404 rule does not apply there; a function whose server does not honor the key treats a 404 on a repeated delete as done itself.
    - *`retry: false`* is for an API whose updates are not safe to repeat. The chart calls `save` once per change set and never again: any failure is put back at once, as rule 3.5 does, with `reason: "failed"`, and there are no buttons.
    - *What an unsafe repeat costs.* Under `keys: "chart"` a repeated create to a server that does not honor the key is refused by the unique key, and the read after that put-back shows one row. Under `keys: "server"`, `idempotent` over a server that does not honor the key stores the row twice (scenario 17).

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
4. **`keys: "server"`.** The create carries `key: null` and a `ref`. `save` returns one `{ ref, key }` for it. The key is assigned when the save settles, as the first thing in the synchronous step of rule 4.4: before `changed`, before any held value and before any waiting row. Until then an unknown key in host data or in `update` waits (rule 4.7). So when the server's copy of the row reaches the chart, early or late, the chart's row already holds that key and the two are one row. Edits the person made to the new row meanwhile are queued, and their change sets are written after the key exists (rule 3.3). A create is repeated only as rule 3.12 allows, and a repeated one must be answered with the same key, or the server holds the row twice.
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
3. **`save` is where the server's rules land:** validation, conflicts, locks, permissions only the server knows. A refusal there is a status-carrying throw or a `SaveRefused` with `message`, `key` and `field` (rule 3.5 says how it is recognized).
4. **A rule that never varies belongs in the capability profile,** which is checked before `beforeEdit` and also removes the affordance. A missing handler is checked with it (rule 3.1). The order for one action: read-only, capabilities and handlers, `beforeEdit`, the edit shows, `save`.
5. **`beforeEdit` is not asked again at a drag's release.** Snapping (rule 8.2) makes the released value one the host can store; the server refuses the rest in `save`.

### 8. What the host says at start

1. **These settings describe the host; none is stored in the document.** Each is an option of `mount` and a prop of the components.

   | Setting | Values | Says |
   |---|---|---|
   | `src` | a URL | where the chart reads host data itself; a new value is a new document (rule 10.3) |
   | `headers` | an object, or a function that returns one | headers added to the chart's own requests, read before each request (rule 10.6) |
   | `fields` | a map for tasks and one for links | the host's name for each chart field (rule 8.4) |
   | `durationUnit` | `"days"`, `"hours"`, `"minutes"` | the unit of every duration and lag, in and out (decision 0017, rule 5) |
   | `dateForm` | `"minute"`, `"day"` | the form of every date out (decision 0017, rule 7) |
   | `keys` | `"chart"` (default), `"server"` | who makes the key of a new row (rule 5) |
   | `retry` | waits in milliseconds (default `[1000, 3000, 9000]`), `false` | when a failed call is repeated (rules 3.6 and 3.12) |
   | `idempotent` | `false` (default), `true` | the server answers a repeated idempotency key as it did the first time, so a create under `keys: "server"` may be repeated (rule 3.12); a `save` string ignores it and is always `true` |
   | `history` | `"chart"` (default), `"host"` | who owns undo (rule 6) |
   | `capabilities` | the profile below | what the host's database can take (decision 0018, rules 9 to 12) |

2. **The profile gains five value members.** `kinds` lists the date-limit kinds a person may set (`[]` for none; rule 9) and `perTask` is the most of them one task holds. `durationStep` and `lagStep` are `"minutes"`, `"hours"` or `"days"`: a typed value that is not a whole step is refused, and a drag snaps to the step. `dates` is `"minute"` or `"day"`; `"day"` refuses a time of day, except a calendar's opening for a start and its closing for a finish. `editable.structure` splits into `rows` (insert and delete), `outline` (indent, outdent and move) and `paste`. The profile is a prop of the React, Vue and Angular components. Each member is enforced at the gate and changes no computed number.
3. **Custom fields pass through.** A member of a task or link in host data that is not a chart field, once the map of rule 8.4 is applied, is a *custom field*. `Owner decided:` the chart keeps it with the record, never reads, shows or edits it, and gives it back unchanged under the same name, in a create's `set` and a delete's `was`. So the create that undoes a delete gives the host back its own columns, as they were when the row was deleted (rule 6.2). A data update or patch sets a custom field as it sets any member, compared by value; no edit writes one, so it is never unsaved and never held. A row the person makes in the chart has none. This amends decision 0017, rule 9, which left an unknown member of a task or link out and counted it in `leftOut`. The cost: a misspelled member (`durration`) is kept and no longer counted. `chart.state().customFields` lists the names seen for tasks and for links, so a developer finds it there. The chart does not show custom fields (columns are rule 14), and the document does not store them yet.
4. **The field map.** `Owner decided:` the map is in this record, so a host writes no rename function (Syncfusion's `taskFields` is the same idea [S4]).

   ```ts
   interface Fields {
     tasks?: { [chartField: string]: string | { type: string; date: string } }; // the object form is for `constraint` only
     links?: { [chartField: string]: string };
   }
   ```

   - *What it says.* Each entry gives one chart field (decision 0017's names, and `key`) the host's name for it: `{ name: "title", start: "plannedStart", duration: "days" }`. A chart field with no entry keeps its own name. The map is read once, at start, so an object written inline in a render is safe and a later change to it does nothing. The map renames members and converts no value: units are `durationUnit`, dates are `dateForm`, and a reference holds a key (rule 2.5).
   - *Where it applies.* Wherever a task or link crosses between host and chart: host data at `mount`, in props and in `setData`; `update` and `changed`; `set` and `was` in every change (`save`, handlers, the `edit` event, the before-edit payload); and `field` in `SaveRefused` and `SaveError`. It does not apply to a change's own members (`record`, `op`, `key`, `ref`, `cascade`), to a `schedule` change, or to the document and `snapshot()`, which keep the chart's names.
   - *The key.* `key: "id"` says the host's `id` member is the row's key. A create under `keys: "chart"` then carries the new key as `set.id`, and `parent`, `from` and `to` hold such ids under whatever names the map gives those fields.
   - *The typed constraint* (section 9) maps as one member that holds `{ type, date }` or `null` (`constraint: "limit"`), or as two flat members (`constraint: { type: "constraintType", date: "constraintDate" }`). In the flat form both members are `null` for no constraint, a change that writes the constraint writes both in `set` and both in `was`, and a record with only one of the two set is refused. The `type` values are the six of section 9. An entry for `constraint` under `constraintForm: "separate"` is refused at `mount`.
   - *One to one, checked at `mount` before any data is read.* Three mistakes make `mount` reject, with a message that names the fields: an entry for a chart field that does not exist; one host name given to two chart fields; and a host name that is the own name of another chart field that has no entry (`{ name: "start" }` alone, where the host's `start` would feed two fields). One exception: `key: "id"` needs no entry for the chart's own `id`, which a record with a key leaves out (rule 5.2).
   - *Two host members never feed one chart field.* Once a chart field has an entry, a host member that carries the chart's own name for that field is not that field: with `name: "title"`, a row's own `name` member is a custom field and passes through (rule 8.3).
   - *A host name no row carries.* When the first host data that holds rows has none with a mapped name, the chart logs one console warning that names it; the misspelling would otherwise show only as an empty column.
   - *Out of scope.* Nested paths (`"dates.start"`), computed fields (a finish made from two columns), value conversion (a status word to a percent, another spelling of a link type or constraint type) and ids that are numbers (a key is text: decision 0016, rule 3). A host with any of those maps them in its own code, before `tasks` and inside its handlers or `save`, and rule 12 counts those lines.

### 9. The typed constraint at the boundary

```ts
type ConstraintType = "snet" | "snlt" | "fnet" | "fnlt" | "mso" | "mfo" | "so" | "fo";
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
   | `mso` | `limit.pin_start`: the bar goes to the date |
   | `mfo` | `limit.pin_finish`: the bar goes to the date |
   | `so` | `snet` and `limit.snlt` on the same instant: the bar is not moved, and float goes negative when logic pushes past the date |
   | `fo` | `limit.fnet` and `limit.fnlt` on the same instant, the same way |

4. **"Mandatory" and "on" are two kinds, not a setting.** `Owner decided:` Primavera P6 has both a pin ("Mandatory Start", "Mandatory Finish": the bar goes to the day) and a floor plus a ceiling on one date ("Start On", "Finish On"). `mso` and `mfo` stay the pins and no option changes what they mean; `so` and `fo` are the second pair, each read as the two limits it stands for and written back as one typed constraint. The names are `so` and `fo` because the record's vocabulary has none for them. A host whose own type names are the P6 pair maps each to the kind it means, once, in its own module. This replaces an earlier draft of this rule, a `mandatoryDates` schedule option, which was never built.
5. **Under `"typed"` a task holds one constraint.** Setting one replaces every date bound the task had, as one change with `set: { constraint }` and `was: { constraint }`. `start` in host data is still a wanted start kept as `snet`, unless the record has a `constraint` member (`null` included), which wins. A task that arrives from a file with more than one bound keeps them all in the engine; its `constraint` reads the first that is set in the order `mso`, `mfo`, `so`, `fo`, `snlt`, `fnlt`, `snet`, `fnet` (`so` when `snet` and `snlt` hold one instant, `fo` when `fnet` and `fnlt` do), and `leftOut` counts the rest. The engine's profile compares instants. The chart's typed read-back compares the dates as written after `dateForm` is applied, so under `dateForm: "day"` an `snet` at 08:00 and an `snlt` at 15:00 of one day write the same text and read as `so`, while the profile counts two limits. Both read `so` and `fo` only when the host's `kinds` is left out or lists them: a typed host whose `kinds` omits one gets the single constraint it got before the pair had a name (`snlt` or `fnlt` by the reading order). A date alone is read on the task's calendar (decision 0017, rule 6).

### 10. The URL default

1. **A `save` given as a string is the handlers form over `fetch`, run by the chart.** `Owner decided:` the default that needs almost no host code is a base URL, and there is no separately imported helper. `save: "/api"` is these handlers, with `base` the string less any trailing slash (`"/api/"` is `"/api"`) and `headers()` the `headers` option, read or called and awaited before each request (rule 10.6):

   ```ts
   const send = async (method: string, path: string, body: unknown, c: SaveContext) =>
     fetch(base + path, { method, body: JSON.stringify(body), signal: c.signal,
       headers: { ...(await headers()), ...(body === undefined ? {} : { "content-type": "application/json" }), "idempotency-key": c.once } });
   const at = (key: Key) => encodeURIComponent(key);
   const save: Handlers = {
     createTask: (row, c) => send("POST", "/tasks", row, c),
     updateTask: (key, set, c) => send("PATCH", `/tasks/${at(key)}`, set, c),
     deleteTask: (key, c) => send("DELETE", `/tasks/${at(key)}`, undefined, c),
     createLink: (row, c) => send("POST", "/links", row, c),
     updateLink: (key, set, c) => send("PATCH", `/links/${at(key)}`, set, c),
     deleteLink: (key, c) => send("DELETE", `/links/${at(key)}`, undefined, c),
     updateSchedule: (set, c) => send("PATCH", "/schedule", set, c),
   };
   ```

   Each handler resolves with the `Response`, which the chart reads by rule 3.5 and, for a create under `keys: "server"`, by rule 3.1 (rule 10.5). That is the definition, so every rule of section 3 applies to it unchanged (one call per commit, order, the status table, failure, repeats resumed at the failed change with `idempotent` always true, the 24-hour limit, teardown), `retry` and `keys` mean what they mean there, and there is no second queue. A change to the convention is a change to that code, and the other way round.
2. **The convention is Tachlon's.** `Owner decided:` the table and the list in the first section are the one specification of the routes, the field names, the refusal and the idempotency key. A host's coding assistant builds or adapts the backend to it; the fake server (rule 11.1) is a backend that speaks it. An API that differs only in its field names uses `fields` (rule 10.4); one that differs in anything else uses handlers or the `save` function.
3. **`src`: the chart reads the schedule itself.** `Owner decided:` the chart sends `src` a `GET`, with `headers` and the 10-second limit of rule 10.1, at mount and after every `error` whose `source` is `"save"` (every put-back fires one, rule 3.5, and so does rule 3.11's): that is rule 3.7's read, done by the chart. Nothing else starts a read: no timer, no focus, no reconnect. A read started while another runs, its waits between repeats included, drops the older read: its answer, its repeats and its failure. The answer at mount is read as `mount` reads plain data. A re-read is `setData` (rule 4.1), so it is host data, and rules 4.2 to 4.8 judge it against unsaved edits; after a put-back, rule 4.5 makes it the truth for the records the put-back touched. A value from a read counts as having arrived when its `GET` was sent, so rule 4.4's tests drop it against a commit called later, and a field or row that a commit first called after that time wrote, saved or not, keeps the chart's value in place of rule 4.5's truth: the read could not know it.
   - *A failed read.* A network error, a 408, a 429 or a 5xx is read again at `retry`'s waits (rule 3.6; none under `retry: false`); another status, a body that is not JSON, or data the validator refuses is not. When the last read at mount fails, `mount` rejects with a message that names the URL and the status or the error, and a component fires `error` with `source: "src"` and `fatal: true` and shows no chart. When the last re-read fails, the chart keeps what it shows, fires `error` once with `source: "src"` and `fatal: false`, and the alert line reads "The schedule could not be read again. Reload the page to see what was saved."; editing and saving go on, and the next put-back reads again.
   - *`src` beside `tasks` or `links`.* `Owner decided:` refused at mount: `mount` rejects, and a component fires `error` with `source: "src"` and `fatal: true`, with a message that names both options and says to pick one. A host that passes `tasks` and `links` itself makes rule 3.7's read itself, as the first section's handlers form does; a `save` string works with either.
   - *A new `src`.* A new value is a new document. A component starts the chart again, as at mount, and the unsaved commits are dropped as at `destroy()` (rule 3.9); with `mount`, the host calls `destroy()` and mounts again. A read still running for the old value is dropped. A host that must keep the unsaved commits awaits `saved()` first.
4. **`fields` with the URL form.** Allowed, and applied as rule 8.4 says: to what `src` answers as to host data, to request bodies as to `set`, and to a refusal's `field`. A `:key` in a path is the key's value; the map renames members only.
5. **Keys.** Under `keys: "chart"`, the default (rule 5.1), a `POST` carries the key in its body (rule 5.3) and the body of its answer is not read. Under `keys: "server"` the `POST` must answer a JSON object that holds the new row's key under the host's name for `key` (the stored row does). The chart takes that as the create's `{ ref, key }` (rule 5.4) and puts the key in place of every later `{ ref }` to the row in the same change set (rule 3.1). An answer without that member is a result the chart cannot use (rule 3.8). A repeated `POST`, answered as the first one was, gives the same key (rule 3.12).
6. **Credentials and headers.** `Owner decided:` requests use `fetch`'s default credentials, so a same-origin API with a session cookie needs nothing. An API that needs a token sets `headers`: an object, or a function that returns one (`Decided here:` or a promise of one, so a token from an asynchronous source works). The chart reads it, or calls it, before each request for `src` and `save`, so a renewed token, or a new `headers` value passed as a prop, is used from the next request on, as a new `save` function is (rule 3.1). A `headers` function that throws or rejects, and a request that reaches the 10-second limit, fail as a network error does: a save repeats it (rule 3.6) and a read is read again (rule 10.3). There is no `fetch` option: a cross-origin cookie, another timeout or any other request shape uses handlers or the `save` function, and passes its data itself. A stated limit: a schedule whose `GET` takes more than 10 seconds cannot mount from `src`; its host passes `tasks` and `links` itself.
7. **Server changes are not part of the URL form.** A stated limit: the URL form reads no `changed` from an answer and reads `src` only when rule 10.3 says. A server that changes rows it was not sent, or pushes another person's changes, reaches the chart through `chart.update` (rules 4.6 and 4.9), which the host calls, as the first section's plain `mount` does with `serverEvents`, or through a `save` function that returns `changed` (rule 3.4).
8. **Errors.** In the URL form, `error` with `source: "save"` still fires once per put-back (rule 3.5), and the chart's own read follows it; a host needs no handler. `error` with `source: "src"` fires only as rule 10.3 says. Rule 3.6 holds: neither fires while a call or a read is being repeated, or while the chart is asking the person.

### 11. The fake server and its scenarios

1. **The example app is the first section's code against a fake server that misbehaves on purpose.** The server lives under `examples/`, holds its rows in memory, is deterministic from a seed, and takes a script: latency per request, which request to refuse and with what, when to answer a 401 or a 500, when to change a row from outside, when to push a late result, when to drop a request or a response, and whether to honor a repeated idempotency key. It speaks the first section's convention (routes, answers, the refusal, the idempotency key), so it is also a reference backend a host's assistant can read. Its rows use the chart's names or, as the script says, its own, as the first section's backend does. It serves both route styles (per record, and one change-set endpoint) and both key modes. The suite drives the chart in a browser through the three forms of the first section: the handlers form, whose `./api` is a client of the fake server's routes written as a host writes one (it sends `c.once` and `c.signal`, and throws an error carrying the status on a non-2xx); the URL form, given the handlers form's `fields` and `dateForm` when the rows use the server's own names; and the function form. The handlers form runs every scenario but 6 and 18 to 20. The URL form runs every scenario but 6; 7, 8 and 9, the second case of 3 and the `keys: "server"` case of 17, in which host data arrives from outside (rule 10.7); and 25 and 26, whose settings a `save` string does not have. Scenarios 18 to 20 need `src` and run through the URL form only. The function form runs every scenario the handlers form runs but 24 to 26, which test what handlers add, and also runs 6, whose change-set endpoint needs a function. Under `keys: "chart"` it runs the first section's code as written. Under `keys: "server"` it runs a copy that differs in that setting, in `idempotent` (the fake server honors the key unless a scenario says otherwise) and, in the function form, in a `save` that replaces each `{ ref }` and returns the keys (rule 3.2), and in nothing else. A scenario that names a setting (`retry: false` in scenario 15, `keys: "server"` without `idempotent` in scenario 25) or the change-set endpoint (scenario 6) changes that and nothing else; scenario 25 runs in that key mode only.
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
   11. *A failure part of the way.* On per-record routes the second of three requests of one commit is refused; the chart puts the commit back, the host reads again (in the URL form, the chart), and the chart shows what the server kept. When that request is dropped instead, the commit is saved whole and the server applies the first request once: the handlers form and the URL form resume at the second request, and the function form sends the first again, which the server answers as before.
   12. *Undo across the server.* A task with two links and a column the chart does not know is deleted and the delete undone: the server gets the task create and then the link creates, the column's value is intact, and when the server refuses that undo the task is gone again and the step is back on the undo list.
   13. *The server is away.* It drops every request while the person makes three edits. All three stay on screen, marked not saved; the first is called four times at the default waits and the others not at all; the chart then stops and asks. The server comes back, the person presses "Try again", and the three reach the server once each, in commit order.
   14. *Discard.* As scenario 13, but the person presses "Discard 3 changes": the three are put back, `error` fires once with `reason: "failed"`, `calls: 4` and `undone: 3`, and the host reads again (in the URL form, the chart).
   15. *Repeats off.* With `retry: false`, a dropped response puts the commit back at once, `save` has run once, and the read after it (the host's, or in the URL form the chart's) shows the row the server kept, once.
   16. *The host's own names.* Every `set` and `was` the server receives uses its own field names, a create carries the new key as `id`, and a row that has both `title` and a `name` member of its own keeps both.
   17. *A server that does not honor the key.* Told to ignore repeated idempotency keys, the fake server refuses the repeated create of scenario 10 under `keys: "chart"`, and the read after the put-back shows one row; under `keys: "server"`, with `idempotent` as rule 11.1's copy has it, it stores two rows and the chart shows two. The suite expects exactly that, as the record of what rule 3.12 warns of.
   18. *Mount from `src`.* The chart mounts with `src` and `save` and no data of its own, shows the server's rows, and the host's code makes no request.
   19. *`src` fails at mount.* The server answers the first read with a 503 and the second with its rows: the chart mounts after one more read. Told to answer 503 to every read, the mount fails after four reads at the default waits: plain `mount` rejects, and the React component fires `error` once with `source: "src"` and `fatal: true`. Told to answer 404, it fails the same way after one read.
   20. *A refused save, read again by the chart.* On per-record routes the second of three requests of one commit is refused: the chart puts the commit back, `error` fires once, the chart reads `src` once and shows the first request's write, which the server kept, and no host code runs.
   21. *A 500 that applied nothing.* The server answers a `PATCH` with a 500, applies nothing and stores no answer; the repeat a second later, with the same `Idempotency-Key`, applies it. The commit is saved, nothing is put back, no `error` fires and `src` is not read.
   22. *An expired login.* The server answers a save with a 401: no call follows, the alert line asks at once with both buttons, and no `error` fires. The script then accepts the session again, the person presses "Try again", and the commit is saved once. Answered with a 403 and a message instead, the commit is put back at once and the message shows.
   23. *A refusal that carries a status.* The server refuses an update with a 422 and a body that is not JSON. In the handlers form the client throws an error carrying `status: 422`: the commit is put back at once with no repeat, the error's message shows, and the refused change's row is selected though the error names no key. In the URL form and the function form the chart reads the `Response` and the message is "The server refused the change" with the status; through the function form no row is selected, because the chart cannot tell which request was refused.
   24. *A 503 part of the way.* The server answers the second of three requests of one commit with a 503: a second later the chart calls again, starting at the second change; the server receives the first request once in all, and the commit is saved.
   25. *A create the server cannot repeat.* Under `keys: "server"` without `idempotent`, with the fake server told not to honor the key: a create whose response is dropped is put back at once with `reason: "failed"` and no call, and the read after it shows the row the server made, once; an update whose response is dropped is repeated and saved; a delete whose response is dropped is repeated, answered 404, and counts as saved.
   26. *A missing handler.* Mounted with no `createLink`, the chart logs one warning that names it, no link can be drawn, and undoing the delete of a task that had a link is refused at the gate: nothing changes and `save` is not called.

### 12. How we will know it worked

| Measure | Target |
|---|---|
| Lines of host code around the chart in the reference host, measured before and after (the headline number) | 100 or fewer, from 659; the study's floor is about 60. No rename function is among them. What rule 8.4 leaves out of scope, the reference host's own product rules (its capabilities, refusal texts and `beforeEdit`) and its own undo are each counted on their own line, apart from the lines the contract should remove |
| The first section's handlers form | 20 lines or fewer in React, imports included, counted as non-blank lines that are not comments |
| State the host holds for the chart: refs, state hooks, queues, diffs | none for saving or syncing |
| Chart members the example touches | 12 props or fewer, and no method on the chart handle in React |
| The example, every file except the fake server | 80 lines or fewer |
| The scenario suite | every scenario of rule 11.2 in both key modes (scenario 25 in its own), through the forms rule 11.1 names |
| The host that follows the convention: the URL form of the first section | 5 lines or fewer in React and in plain `mount`, imports included; each scenario that rule 11.1 runs through both the handlers form and the URL form passes through both, and the server receives the same requests from each |
| A cold run: a fresh coding assistant given only the README and the fake server writes the integration | it passes the suite in each of three runs; lines, errors and turns are recorded for each |

A line count alone invites a toy, so no single row of this table passes the work; all eight do.

### 13. Build order, and what each step breaks for a 0.3.1 host

The order is the outside review's, and this record agrees with it: `save` takes changes, so their shape comes first; `save` without in-place data cannot be used, so in-place data comes before it; undo is built last, and its contract is decided here, before `save`, because an undo reaches `save`. Two additions: the fake server and the suite land first, failing, and steps 1 to 3 ship in one release, 0.4.0, because any one of them alone leaves the host writing today's glue.

| Step | What ships | Covered by | What breaks for a 0.3.1 host |
|---|---|---|---|
| 0 | The fake server, the scenario suite and the example, failing | none | nothing |
| 1 | Record-shaped changes (section 2), one unit, `dateForm: "day"`, the field map and custom fields (rules 8.3 and 8.4) | no row found; `tl2090dd` covers the displayed form only; the field map: not searched | `edit` commits and the before-edit payload change from `field`, `old`, `new` to `set`, `was`; a host mounted with hours or minutes gets durations in its unit, not days; an unknown member of a task or link is kept as a custom field, not left out and counted |
| 2 | Data in place by key (section 4): props, `setData`, `update` with every input field, links and deletes; `capabilities`, `pending` and `selectedKey` as props | `tl2087cp`, `tl2087uc`; links, creates and deletes by key: no row found | a changed `tasks` or `links` prop no longer starts the chart again; a host that relied on that sets a React `key` or calls `load` |
| 3 | `save` as handlers and as a function, the status table and `classify`, the queue, repeated calls, `retry` and `idempotent`, put-back, `keys`, `changed`, `SaveError`, `state().unsavedCommits` and `state().save` (sections 3 and 5); the URL default (section 10): `src`, a `save` string as handlers over `fetch`, and `headers`, with the convention in the README | `tl2091sq`; its engine half is merged; the URL default: not searched | nothing without `save` or `src`; `Gantt.save()` is renamed `download()` |
| 4 | The typed constraint (section 9), then the profile values and snapping (rule 8.2) | no row found for the constraint; `tl2090cq`, `tl2090sn`; the `structure` split: no row found | `editable.structure` becomes `rows`, `outline` and `paste`; at 0.4.0 changes write `constraint` |
| 5 | Undo (section 6): `history`, `canUndo`, `canRedo`, stale steps, host-only entries | issue tl#2092; no row found | nothing under the default |

`Owner decided:` the URL default is built in step 3 with `save`, as handlers over `fetch` on the same queue (rule 10.1), not after it; the handlers form and the function form are built in the same step. The convention ships where the first section does: the package README's "Connect to your backend" section is the first section, unchanged (rule 1.1), and the README is the package's own documentation (decision 0016, rule 6), so a host's coding assistant finds it in the installed package.

The study's R8 (asking `beforeEdit` again at a drag's release) is dropped (rule 7.5).

### 14. Not in this record

- **Resources and assignments** in host data. A task keeps its display-only `resources` list (decision 0017).
- **Column configuration:** which columns, their order and width, cell renderers, and columns for custom fields.
- **Nested, computed and converted fields in the field map,** and ids that are numbers (rule 8.4).
- **Any transport but the convention.** The chart calls `fetch` only for `src` and a `save` string (section 10). Other routes, a change-set endpoint, GraphQL or a socket are the host's handlers or `save` function.
- **An event per saved commit, and `inverse` and `replay` exports, for a host that owns its undo.** `Owner decided:` not now: a host that owns its undo uses the `save` function today, and an event per saved commit is a candidate for a later record.
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

## What this choice costs

The adversarial read that proposed handlers first also gave the case against them. All three stand.

1. **The classification is implicit.** The status policy is in the chart, so a reader of the host's code sees no retry logic, and a library that puts `status` on an error for another reason is misread (rule 3.5). It also catches only a refusal read as a failure, which costs repeats and no data. A client that reports an error by resolving is read as saved, and a console warning is the only guard (rule 3.1).
2. **Three ways in, with two repeat models.** The handlers form resumes at the failed change and the function form repeats the whole change set (rule 3.12), so both are documented and tested (rule 11.1). The handler names grow with each record kind the chart learns (resources and assignments, section 14), and a forgotten handler turns edits off, with a warning at mount.
3. **The reference host gains least.** It owns its undo with a stack that runs each command itself, which needs the commit boundary that handlers do not give, so it stays on the function form (section 14). On the adversarial read's paper count, the cleaned function form takes its integration module from 155 lines to 148, and handlers would reach 132 only with the event and exports left out of this record. The first form is chosen for hosts not yet met, by judgment, over the one host measured.

## What could not be verified

- Bryntum's own documentation pages did not open (the guide and API pages returned a 404 or a site overview). That its sync is one batched request with per-store `added`, `updated` and `removed`, and that a created record is answered with `$PhantomId` and `id`, is verified from a tutorial on the vendor's blog [S2] and the vendor's skill file [S3]. Whether a sync response may carry other rows the server changed, and what the component does after a failed sync: not verified. So rule 3.6's repeated calls and their waits are this record's own choice, checked against no other component; whether DHTMLX or Syncfusion repeat a failed request was not read either.
- Whether a backlog row already covers the field map or the repeated calls was not searched.
- Whether deleting a summary task deletes the tasks under it today was not checked. Rule 2.7 says how those deletes are listed when one delete takes other tasks with it.
- What P6 and MS Project do with a mandatory date when logic pushes past it (rule 9.4) is from memory: not verified.
- Which client libraries put a numeric `status` on their errors (rule 3.5: axios, ky and ofetch carry one, many plain `fetch` wrappers do not, and tRPC, GraphQL clients and Firebase need `classify`) is from the adversarial read's memory of their documentation: not verified. Its line counts for the reference host were made on paper.
- Nothing in this record was run. Every statement about today's behavior comes from reading `origin/main` at `45d86f2`.

## Rows this governs

`tl2091sq` (section 3), `tl2087cp` and `tl2087uc` (section 4), `tl2090cq` and `tl2090sn` (rule 8.2), issue tl#2092 (section 6), and the rows still to be written for steps 0, 1 (the field map and custom fields included), 2 (links, creates and deletes by key), 3 (handlers, the status table, repeated calls and `idempotent`, and the URL default) and 4 (the typed constraint, the `structure` split).

## Sources

- [S1] DHTMLX Gantt, server-side integration: https://docs.dhtmlx.com/gantt/guides/server-side/
- [S2] Bryntum blog, connecting Bryntum Gantt to a planning backend (2025-02-21): https://bryntum.com/blog/how-to-connect-and-sync-bryntum-gantt-to-a-servicenow-project-portfolio-management-planning-chart/
- [S3] Bryntum skills repository, `bryntum-crud`: https://raw.githubusercontent.com/bryntum/skills/refs/heads/main/bryntum-crud/SKILL.md
- [S4] Syncfusion Gantt, data binding: https://help.syncfusion.com/gantt-sdk/typescript/gantt-chart/data-binding
