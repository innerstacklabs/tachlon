# The host save contract

Status: proposed, 2026-11-18. A draft for the owner to read and correct before anything is built; none of it is built. A line marked `Decided here:` is the drafter's choice where the study and the outside review disagree or leave a point open. Once accepted it amends [decision 0016](0016-host-identity.md) (rules 7 and 8), [decision 0017](0017-host-record-vocabulary.md) (rule 9 and its closing note on mandatory dates) and [decision 0019](0019-commit-pipeline.md) (rules 7, 8, 10 and 12).

A *host* is the product that embeds the chart. This record designs the host's side first: the code a developer, and their coding assistant, writes to connect the chart to a backend. Every rule after the first section exists to make that code true.

## Connect the chart to your backend

The imagined backend is a plain REST API: `GET /api/schedule` answers `{ tasks, links }`, tasks and links have `POST`, `PATCH /:id` and `DELETE /:id` routes, and a refusal is a 422 with `{ message, key?, field? }`. In React:

```tsx
import { Gantt, SaveRefused, type Save } from "tachlon/react";
import "tachlon/gantt.css";

const api = async (method: string, path: string, body?: unknown) => {
  const res = await fetch(`/api${path}`, { method, headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
  if (res.status === 422) throw new SaveRefused(await res.json()); // the server said no: { message, key?, field? }
  if (!res.ok) throw new Error(`Not saved: the server answered ${res.status}.`);
  return res.status === 204 ? undefined : res.json();
};

const save: Save = async ({ changes }) => {
  for (const c of changes) {
    const path = `/${c.record}s`; // "/tasks" or "/links"
    if (c.record === "schedule") await api("PATCH", "/schedule", c.set);
    else if (c.op === "create") await api("POST", path, { key: c.key, ...c.set });
    else if (c.op === "update") await api("PATCH", `${path}/${c.key}`, c.set);
    else if (!c.cascade) await api("DELETE", `${path}/${c.key}`); // the database drops a deleted task's links itself
  }
};

export function Schedule({ tasks, links, reload }: { tasks: Task[]; links: Link[]; reload: () => void }) {
  return (
    <Gantt tasks={tasks} links={links} durationUnit="days" dateForm="day" keys="chart"
      save={save} onError={(e) => e.source === "save" && reload()} style={{ height: 480 }} />
  );
}
```

With no framework, the same `api` and `save`:

```ts
import { mount } from "tachlon";

const chart = await mount(root, { ...(await api("GET", "/schedule")), durationUnit: "days", dateForm: "day", keys: "chart", save });
chart.on("error", async (e) => { if (e.source === "save") chart.setData(await api("GET", "/schedule")); });
serverEvents.onmessage = (m) => chart.update(JSON.parse(m.data)); // rows the server changed later (rule 4.9)
```

That is all of it. The host writes no save queue, no put-back, no diff, no "previous value" and no key bookkeeping. `tasks` and `links` are the host's rows in the chart's field names (rule 8.4 says why renaming is not in this record). A server that makes its own ids mounts with `keys="server"` and returns the ids from `save` (rule 5).

## Context

Embedding the chart in one real product took 659 lines of host code. A study of those lines found that a third (216) exist because the page API has no save function, no put-back and no in-place data update, and another third (224) translate vocabulary: per-field changes folded back into per-record saves, dates cut to a day, six limit fields mapped to one typed constraint. The pieces exist below the page API: the `edit` event carries commits (decision 0019, rule 8), the engine can take commits back (`App.rollback_to` and `App.acknowledge` in `docs/WASM_API.md`), and `update(records)` sets five members by key. The React component starts the chart again when `tasks` or `links` changes (`packages/gantt/src/react.tsx`), so a host cannot pass its own state.

The three components this one is compared with all own the save path. DHTMLX's data processor sends one request per added, updated or deleted record and takes the new id from the response (`tid`) [S1]. Bryntum's project sends one request per sync with every change in it and maps a client-made `$PhantomId` to the server's id [S2, S3]. Syncfusion binds a `DataManager` with an optional batch URL [S4]. None of the pages read asks the host to turn change reports into save calls. An outside review of the plan asked for the contract to be written before any of it is built, with type signatures, and tested against a fake server that misbehaves.

## Decision

Terms used below. *Host data* is the `tasks` and `links` the host gives the chart. A *commit* is one undo step (decision 0019, rule 1). A commit is *unsaved* from the moment it is made until its `save` settles: it is *in flight* while `save` runs for it and *queued* before that. A field is unsaved while an unsaved commit writes it; a row is unsaved while an unsaved commit creates or deletes it. A save *settles* when its promise resolves (saved) or rejects (failed).

### 1. The target

1. **The first section is the contract's test.** A change to this record that makes that code longer, or adds state the host must hold, needs a reason stated beside it. The example app (rule 10) is that code against the fake server, and the README's "Connect to your backend" section is that code, unchanged.

### 2. The shape of a change

```ts
type Key = string;
type Ref = { ref: string }; // names a row created in the same change set that has no key yet (keys: "server" only)

interface Change {
  record: "task" | "link" | "schedule";
  op: "create" | "update" | "delete"; // a "schedule" change is always an update
  key: Key | null;                     // null for the schedule, and for a create under keys: "server"
  ref?: string;                        // on a create under keys: "server": the name SaveResult.keys answers
  set?: Record<string, unknown>;       // create: the whole record. update: the members that changed, new values
  was?: Record<string, unknown>;       // update: the same members, old values. delete: the whole record as it was
  cascade?: true;                      // a delete that follows from another delete in this change set
}

interface ChangeSet { seq: number; source: CommitSource; changes: readonly Change[] }
```

1. **One change per record.** A commit that sets three fields of one task carries one change with three members in `set` and the same three in `was`. A record appears at most once in a change set. This replaces the one-change-per-field shape of decision 0019, rule 7 (`field`, `old`, `new`), because a host saves records, and folding fields back into records was most of the 224 vocabulary lines.
2. **`set` and `was`.** An update carries only the members that changed. A create carries the whole record as `snapshot()` writes it, `custom` included (rule 8.3), and no `was`. A delete carries the whole record in `was` and no `set`, so a host that keeps an audit trail or its own undo has what it removed.
3. **One unit, the one the host mounted with.** Durations and lags in `set` and `was` are in `durationUnit`. This fixes a stated inconsistency: `docs/WASM_API.md` (`App.take_commits`) says a commit's duration is "a number of days of the task's calendar", while `update` reads `durationUnit`. Decision 0017, rule 5 wins: one unit, in and out, on every surface. A host that mounts with hours gets hours.
4. **Dates in the form the host chose.** Under `dateForm: "day"` every date in a change is `YYYY-MM-DD`, converted as decision 0017, rule 7 says (a finish-kind date is the day of its instant less one minute). `was` uses the same form, so a host compares it with what it stored.
5. **References are keys.** `parent`, `after`, `from`, `to` and `calendar` hold the other record's key. The one exception is a row created in the same change set under `keys: "server"`: it is `{ ref }`, an object, so it cannot be read as a key. A change set never holds a session slot. `Decided here:` this replaces `{ task: slot }` at the save boundary, because a slot is valid only inside one session and a host that stored one would be wrong after a reload.
6. **Order inside a change set is the order a server can apply top to bottom.** A create comes before any change that names its row (a parent before its children, both tasks before the link that joins them), and a link delete comes before the delete of a task it touches.
7. **A delete that follows from another delete is marked `cascade: true`.** Deleting a task deletes its links (decision 0017, "A link"); each of those link deletes carries the mark. A host whose database cascades skips marked changes; a host whose database does not applies every change in order. The mark is only ever on a delete: the undo of that delete lists the links as ordinary creates, because the host must make them again.
8. **The `edit` event and the before-edit payload carry the same changes,** with the session slot added (`task` or `link`), because a chart with no `save` may hold rows with no key. A chart with `save` never needs the slot.

### 3. `save`

```ts
type Save = (changeSet: ChangeSet) => Promise<SaveResult | void>;

/** Rows the server changed: whole or partial records by key, and keys it deleted. */
interface ServerChanges { tasks?: readonly TaskRecord[]; links?: readonly LinkRecord[]; deleted?: { tasks?: readonly Key[]; links?: readonly Key[] } }

interface SaveResult {
  keys?: readonly { ref: string; key: Key }[]; // keys: "server" only: exactly one for each create that has a ref
  changed?: ServerChanges;                      // applied by key, with no undo step and no save call
}

/** Throw this from `save` when the server judged the change and said no. */
class SaveRefused extends Error { constructor(refusal: { message: string; key?: Key; field?: string }) }

/** The `error` event of a failed save. `reload`: the chart could not put itself back, so give it fresh data with `load`. */
interface SaveError { source: "save"; message: string; fatal: false; refused: boolean; key?: Key; field?: string; seq: number; undone: number; reload: boolean }
```

1. **`save` is a mount option and a prop.** `CommitSource` is `typed`, `drag`, `dialog`, `paste`, `proposal`, `assistant`, `host`, `undo` or `redo`; a `rollback` commit is never sent. A commit with no changes for the host is not sent. With no `save`, the chart behaves as 0.3.1 does and the host reads the `edit` event.
2. **One call per commit, with the whole change set.** `Decided here:` not one call per record. The commit is the unit the chart can put back, so it must be the unit the host saves: a commit saved in three calls that fails on the second has no clean way back. A host coming from a batched library (Bryntum sends every store's `added`, `updated` and `removed` in one request [S2]) posts `changes` to one endpoint and saves it in one transaction. A host coming from a per-record library (DHTMLX sends one request per record [S1]) loops over `changes` in order, as the first section does. Under `keys: "server"` that loop also keeps a map from each `ref` to the id its `POST` returned and replaces a `{ ref }` it meets later in the same set: about four lines, which a change-set endpoint or chart-made keys never need.
3. **One save in flight, in commit order.** The person keeps editing while a save runs; those commits queue. The chart calls `save` for the next commit only after the one before has settled and its result has been applied. A change set is written when it is sent, not when the commit is made, so it names every key that earlier saves assigned. `seq` rises with every commit and is never reused in a session; the gaps a `rollback` commit leaves are expected. Queued commits are not merged into one call (rule 13).
4. **Resolving means saved.** With no value: the server holds the change as sent. `keys` assigns server-made keys (rule 5). `changed` is what the server changed beyond what it was sent: rows it recalculated, a name it trimmed, a row a rule of its own deleted. The chart applies `changed` by key with no undo step, no `save` call and no before-edit call, by the rules of section 4.
5. **Rejecting means not saved.** The chart puts itself back to the state before that commit, takes back every commit queued behind it as well (they were made on top of it; decision 0019, rules 10 and 11), fires `error` with a `SaveError`, and shows `message` in its alert line with the number of changes undone. For a `SaveRefused` with a `key` the chart selects that row, and with a `field` it shows the message at that cell. Rows whose commit is unsaved are dimmed as `setPending` dims them, by the chart, until the commit settles.
6. **Any throw is handled the same way.** `Decided here:` a network failure is put back exactly as a refusal is; only `refused` differs. The chart cannot know whether the server holds a change whose response was lost, and it never retries by itself, because only the host knows whether its API is safe to call twice. A `save` that can retry safely does so before it throws; `seq` is the value to send as an idempotency key.
7. **After a failed save, the host reads its data again.** The chart has put the edits back; the host's data then says what the server kept. This covers a lost response (the server did save: the edit comes back with the fresh data) and a per-record loop that failed part of the way (the server holds half: the fresh data shows it). The first section does this in `onError`.
8. **A result the chart cannot use is a failed save.** A missing, extra or duplicate entry in `keys`, a key another row holds, `keys` under `keys: "chart"`, or a record in `changed` that the validator refuses: the chart treats the save as failed (rule 3.5) with a message that names the mistake. The server may hold the rows; the host's fresh data shows them.
9. **Unsaved work at teardown.** `chart.state().unsaved` is the number of unsaved commits, and `chart.saved()` resolves when it reaches 0. `destroy()`, and so a React unmount, does not wait: the save in flight finishes and its result is ignored, and queued commits are dropped. A host that must not lose them warns on leave while `unsaved` is above 0, or awaits `saved()`. The chart sets no timeout on `save`; a promise that never settles holds the queue, and `unsaved` shows it.
10. **A settle never waits for the person and never changes a value under their hands.** If a save fails while a cell is being edited or a bar is held, the chart cancels that gesture and puts back. If a save succeeds during a gesture, keys are assigned at once and the next save is sent; `changed`, held values and waiting rows (section 4) are applied when the gesture ends.
11. **When the chart cannot put itself back** (the history no longer holds the commit after 200 steps, or the chart restarted on WebGL2 with commits unsaved), `SaveError.reload` is true, the queue is emptied, and the host gives the chart fresh data with `load`. `Decided here:` the method `Gantt.save()`, which downloads the document, is renamed `download()` in the release that adds the `save` option, so that "save" has one meaning.

### 4. Who wins between host data and unsaved edits

1. **Data props update the chart in place, by key.** A change to `tasks` or `links` (in React, Vue and Angular) or a call to `chart.setData({ tasks, links })` (plain `mount`) never starts the chart again. It is not an edit: no undo step, no `save` call, no before-edit call, and the selection and scroll position stay.
2. **A data update is the difference from the one before it.** The chart remembers the last host data. For each key it compares the new record with the remembered one, member by member and by value: a member that differs is a new value, a key that is new is a new row, a key that is gone is a removed row. A member that did not change does nothing, whatever the chart shows. `Decided here:` this, and not "host data replaces what the chart shows". Reason: a host passes old data all the time without meaning to (a render that builds a new array, a read that began before a save and ends after it), and under the other rule each of those would undo a saved edit or bring back a deleted row. The cost: the host must keep its data current, because a change that never appears in host data is never seen. Passing the same array again is skipped without comparing.
3. **An unsaved field keeps its local value.** A new host value for it is *held* (the latest one wins) and does not show. Other fields of the same record update at once. So an outside change to a row with a change in flight shows right away in every field the person did not touch.
4. **When the commit is saved,** the chart applies, in this order and in one synchronous step: the keys in `keys`; then `changed`; then held values; then waiting rows (rule 4.7). For a held value of a field that commit wrote, the first test that matches decides:
   - if `changed` names the record, `changed` is applied and the record's held values are dropped: the server has answered;
   - if the held value arrived before `save` was called for the commit, it is dropped: the save is newer;
   - if it arrived while `save` was running, it is applied: the chart cannot know which the server took last, and the next data update corrects it if the save won.

   Rule 3.10 is the one exception to "one step": during a gesture, everything after the keys waits for the gesture to end, and counts as having arrived when the save settled.

   A field that a later unsaved commit also writes stays unsaved, and its held value waits for that commit under the same three tests.
5. **When the commit fails,** it and the commits behind it are put back, and every held value for a field that is no longer unsaved is applied. The chart then shows the host's data.
6. **`changed` and `chart.update(changes)` are patches.** Each takes `ServerChanges`: records by key holding only the members to set. A member is applied at once unless its field is unsaved, in which case it is held under rule 4.4 (a `changed` value is always older than any commit still queued, so it is dropped when that commit is saved). A patch does not alter the remembered host data. `update` keeps taking the 0.3.1 array of task records.
7. **Rows.** A key that is new in host data and unknown to the chart is added, with one exception: under `keys: "server"`, while a commit that creates a row is in flight, an unknown key *waits* until that save settles, because it may be the server's copy of the row the chart is still saving (rule 5.4). A key that is new in host data and already in the chart (a row the chart created, coming back) is compared with the chart's values member by member, and the differences apply as in rule 4.3. A key that is gone from host data is removed with its links; if the row has an unsaved change, the removal is held like a value. A row the chart created is never removed because host data lacks it: only a key that host data once held can go missing. A row in `changed` or `update` with an unknown key is added if the record is whole and refused (`unknownKey`) if it is not.
8. **What the person deleted stays deleted** while host data still lists it, because an unchanged record is no difference. If the delete's save fails, the row comes back with the put-back.
9. **A late result is a data update like any other.** A server that recalculates asynchronously and reports minutes later sends its changes through `update` (or the host puts them in its data), and rules 4.2 to 4.7 apply as written: fields with an unsaved change keep the person's value, every other field takes the late one. The chart has no version check, so two duties fall to the server. It sends only the rows and members it changed, never a whole schedule read before the last save settled. And it sends inputs only: a computed date is not an input. `start` in a record is a wanted start and is kept as a start-no-earlier-than limit (decision 0017), so a server sends `start` only where it means that. Comparing a server's computed dates with the chart's own is row `tl2087hd`, not this record. While a late result is expected, the host marks the rows with `pending`, which is a hint and not a lock.
10. **`was` is the server's conflict check.** The chart resolves nothing between two people. A server that must not let a save overwrite an outside change compares `was` with what it holds and refuses.

### 5. Keys for new rows

1. **With `save`, the host says who makes keys: `keys: "chart"` or `keys: "server"`.** `Decided here:` there is no default; `mount` rejects a `save` without `keys`. The outside review asked for chart-made keys by default and the study for server-made. Either default fails quietly for the host that needed the other one, and rule 4.2 makes that failure a lasting duplicate row, so the choice costs one word and is never a guess. Both modes are first-class and pass the same scenarios (rule 10).
2. **Every row has a key.** With `save`, a task or link in host data with no `key` is refused (`noKey`) at `mount` and in `setData`. A record that has a `key` may leave `id` out, and `parent`, `from` and `to` then name keys; a payload names records by `id` throughout or by `key` throughout, never both. `Decided here:` this loosens decision 0016, rule 8, so a host passes its rows in the shape that changes come back in.
3. **`keys: "chart"`.** The chart makes the key when the row is made, before the commit is written, with `crypto.randomUUID()` or the host's `newKey: () => string`. The create carries it in `key`, there is no `ref`, and a reference to the row is its key from the first change on. The server stores that key as the row's id, or beside its own id, and gives it back as `key` in host data. The row is never without a key, so it cannot appear twice.
4. **`keys: "server"`.** The create carries `key: null` and a `ref`. `save` returns one `{ ref, key }` for it. The key is assigned when the save settles, as the first thing in the synchronous step of rule 4.4: before `changed`, before any held value and before any waiting row. Until then an unknown key in host data waits (rule 4.7). So when the server's copy of the row reaches the chart, early or late, the chart's row already holds that key and the two are one row. Edits the person made to the new row meanwhile are queued, and their change sets are written after the key exists (rule 3.3).
5. **A refused create** takes the row away with the put-back, and every queued commit that touched it (rule 3.5).
6. **The undo of a delete** is a create that carries the key the row had. Under `keys: "server"` it also carries a `ref`, and the result may answer with a new key: the chart then gives the row the new key and the old one is free. History entries follow, because the history holds slots and never keys (decision 0016, rule 1).

### 6. Undo

1. **The host says who owns the history: `history: "chart"` (default) or `history: "host"`.** Both are first-class.
2. **`"chart"`: one history, in the chart.** Undo and redo are commits with `source: "undo"` and `"redo"`, and they reach `save` as any commit does: the undo of an edit is an update with `set` and `was` swapped, the undo of an insert is a delete, and the undo of a delete is a create (rule 5.6) followed by creates for its links. `save` may refuse one; the put-back then restores the state before the undo, and the step is on the undo list again. What the host needs: an API that accepts the reverse of everything it accepts, which means it can make a deleted row again from the record in `set`, with `custom`. For its own toolbar it reads `chart.state().canUndo` and `canRedo` and calls `chart.commands.undo()`. A host-only action is an entry on the same history, as decision 0019, rule 12 says.
3. **`"host"`: the host owns history and the chart's own undo is off.** The undo and redo keys fire `undo` and `redo` events, as the find key fires `find`, and change nothing. The host records what it needs from each change set (`set` and `was`), runs its own reverse against its API, and the result arrives as host data (section 4), never as a commit. The chart still puts a failed save back; that is not undo. What the host needs: its own stack and a reverse for each action.
4. **Entries a failed save makes stale.** A put-back removes the failed commit and those behind it from the undo list and leaves no redo step for them (`App.rollback_to` in `docs/WASM_API.md`).
5. **Entries a server change makes stale.** An undo or redo step is stale when a server change (`changed`, a data update or `update`) gives a field the step wrote a value other than the one the step wrote, or removes a record the step touches. `Decided here:` a stale undo step leaves the history together with every step older than it; newer steps stay. If any redo step is stale, the redo list is emptied. Reason: steps are taken back newest first, and an older step was recorded against a state the stale step can no longer restore. The chart says so once in its alert line ("3 undo steps were removed: the server changed those rows") and `canUndo` follows. A server change that sets a field to the value the step wrote makes nothing stale.

### 7. `beforeEdit` beside `save`

1. **Keep both.** `beforeEdit` is synchronous and runs at the gate before anything changes (decision 0019, rules 3 to 6). `save` is asynchronous and runs after the edit shows.
2. **`beforeEdit` is for rules the page can answer from what it already holds:** a locked row, a role, a summary row the host treats as a folder. It makes no network call. A refusal there costs the person nothing: no change, no put-back.
3. **`save` is where the server's rules land:** validation, conflicts, locks, permissions only the server knows. A refusal there is a `SaveRefused` with `message`, `key` and `field`.
4. **A rule that never varies belongs in the capability profile,** which is checked before `beforeEdit` and also removes the affordance. The order for one action: read-only, capabilities, `beforeEdit`, the edit shows, `save`.
5. **`beforeEdit` is not asked again at a drag's release.** Snapping (rule 8.2) makes the released value one the host can store; the server refuses the rest in `save`.

### 8. What the host says once, at start

1. **Five settings describe the host; none is stored in the document.**

   | Setting | Values | Says |
   |---|---|---|
   | `durationUnit` | `"days"`, `"hours"`, `"minutes"` | the unit of every duration and lag, in and out (decision 0017, rule 5) |
   | `dateForm` | `"minute"`, `"day"` | the form of every date out (decision 0017, rule 7) |
   | `keys` | `"chart"`, `"server"` | who makes the key of a new row (rule 5) |
   | `history` | `"chart"`, `"host"` | who owns undo (rule 6) |
   | `capabilities` | the profile below | what the host's database can take (decision 0018, rules 9 to 12) |

2. **The profile gains four members.** `constraints: { types: ConstraintType[] }` lists the constraint types a person may set (`[]` for none; rule 9). `durationStep` and `lagStep` are `"minute"`, `"hour"` or `"day"`: a typed value that is not a whole step is refused, and a drag snaps to the step. `editable.structure` splits into `rows` (insert and delete), `outline` (indent, outdent and move) and `paste`. The profile is a prop of the React, Vue and Angular components. Each member is enforced at the gate and changes no computed number.
3. **Custom fields survive the round trip.** A task or link may carry `custom`, a JSON object the chart never reads. It stays with the record, a data update or patch replaces it whole, a create writes it in `set`, a delete writes it in `was`, and `snapshot()` returns it. It is in this record because the undo of a delete must give the host back its own columns (rule 6.2); without it that create loses data. The chart does not show or edit `custom` (columns are rule 13), and the document does not store it yet. An unknown member outside `custom` is still left out and counted, as decision 0017, rule 9 says, so a misspelled field name is still reported.
4. **A field map is the next record, not this one.** `Decided here:` a map from the host's field names to the chart's (as Syncfusion's `taskFields` does [S4]) would remove the host's rename function, and it is wanted. It is left out here for three reasons. It changes no rule in this record: every surface already uses one vocabulary (decision 0017, rule 1), so a map is one table applied at one boundary. A map that is only partly right is worse than none, and a complete one must settle nested shapes (the constraint, a link's ends, calendars), name clashes, and how a mapped custom field becomes a column, which belongs with column configuration. And rule 11 counts the rename function's lines on their own, so the cost of waiting is measured.

### 9. The typed constraint at the boundary

```ts
type ConstraintType = "snet" | "snlt" | "fnet" | "fnlt" | "mso" | "mfo";
type Constraint = { type: ConstraintType; date: string } | null;
```

1. **A task has one member, `constraint`,** in host data, changes, patches and the before-edit payload. `null` is no constraint. The type names are decision 0017's limit names. Host stores and exchange files hold one typed constraint per activity, and the reference host's 71 lines of limit mapping exist only because the chart speaks six separate fields.
2. **`constraint` and the six date-limit members are two spellings of the same engine bounds.** A task in host data that carries both is refused. Which spelling the chart writes is a sixth setting of the kind in rule 8.1: `constraintForm: "typed" | "separate"`. `Decided here:` the default stays `"separate"` through 0.3.x so no host breaks, and becomes `"typed"` at 0.4.0. `alap` and `resume` are not date limits and stay their own members.
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

### 10. The fake server and its scenarios

1. **The example app is the first section's code against a fake server that misbehaves on purpose.** The server lives under `examples/`, holds its rows in memory, is deterministic from a seed, and takes a script: latency per request, which request to refuse and with what, when to change a row from outside, when to push a late result, when to drop a response. It serves both route styles (per record, and one change-set endpoint) and both key modes. The suite drives the chart in a browser, and the example passes every scenario unchanged in both key modes.
2. **The scenarios.**
   1. *Slow saves.* With 800 ms of latency, three typed edits reach the server as three calls in commit order, never two at once.
   2. *A field refusal.* The server refuses a duration over 100 days with a message, the key and `field: "duration"`; the cell goes back, the row is selected and the message shows at the cell.
   3. *A server-made key.* An inserted row takes its key from the result and appears once, also when the host's fresh data with that row lands before the result does.
   4. *An edit before the key.* A row is inserted and renamed before its save returns; the rename is saved second and names the server's key.
   5. *A refused create with edits behind it.* The server refuses the insert of scenario 4; the row and its rename both go, and one message says two changes were undone.
   6. *A server recalculation.* Saving a duration makes the server change the constraint of two rows the person did not touch and return them in `changed`; they update with no undo step, and undo takes back only the person's edit.
   7. *An outside change during a save.* While a save is in flight another user changes the same row; the field the person did not touch shows at once, and the field they did touch waits and ends on the server's value.
   8. *A late result.* Two minutes after a save the server pushes new values for five rows, one of which has an unsaved edit; four update and the fifth keeps the edit until its save settles.
   9. *An old read.* A full read that began before a save, and before a delete, returns after both; the saved value stays and the deleted row does not come back.
   10. *A lost response.* The server stores a create and the response is dropped; the chart puts the row back, the host reads again, and the row appears once.
   11. *A failure part of the way.* On per-record routes the second of three requests of one commit is refused; the chart puts the commit back, the host reads again, and the chart shows what the server kept.
   12. *Undo across the server.* A task with two links and a `custom` value is deleted and the delete undone: the server gets the task create and then the link creates, `custom` is intact, and when the server refuses that undo the task is gone again and the step is back on the undo list.

### 11. How we will know it worked

| Measure | Target |
|---|---|
| Lines of host code around the chart in the reference host, measured before and after (the headline number) | 100 or fewer, from 659; the study's floor is about 60. The rename function is counted on its own line |
| State the host holds for the chart: refs, state hooks, queues, diffs | none for saving or syncing |
| Chart members the example touches | 12 props or fewer, and no method on the chart handle in React |
| The example, every file except the fake server | 80 lines or fewer |
| The scenario suite | 12 of 12 in both key modes, with the example unchanged |
| A cold run: a fresh coding assistant given only the README and the fake server writes the integration | it passes the suite in each of three runs; lines, errors and turns are recorded for each |

A line count alone invites a toy, so no single row of this table passes the work; all six do.

### 12. Build order, and what each step breaks for a 0.3.1 host

The order is the outside review's, and this record agrees with it: `save` takes changes, so their shape comes first; `save` without in-place data cannot be used, so in-place data comes before it; undo is built last, and its contract is decided here, before `save`, because an undo reaches `save`. Two additions: the fake server and the suite land first, failing, and steps 1 to 3 ship in one release, 0.4.0, because any one of them alone leaves the host writing today's glue.

| Step | What ships | Covered by | What breaks for a 0.3.1 host |
|---|---|---|---|
| 0 | The fake server, the scenario suite and the example, failing | none | nothing |
| 1 | Record-shaped changes (section 2), one unit, `dateForm: "day"` | no row found; `tl2090dd` covers the displayed form only | `edit` commits and the before-edit payload change from `field`, `old`, `new` to `set`, `was`; a host mounted with hours or minutes gets durations in its unit, not days |
| 2 | Data in place by key (section 4): props, `setData`, `update` with every input field, links and deletes; `capabilities`, `pending` and `selectedKey` as props; `custom` | `tl2087cp`, `tl2087uc`; links, creates and deletes by key: no row found | a changed `tasks` or `links` prop no longer starts the chart again; a host that relied on that sets a React `key` or calls `load` |
| 3 | `save`, the queue, put-back, `keys`, `changed`, `SaveError`, `state().unsaved` (sections 3 and 5) | `tl2091sq`; its engine half is merged | nothing without `save`; `Gantt.save()` is renamed `download()` |
| 4 | The typed constraint and `mandatoryDates` (section 9), then the profile values and snapping (rule 8.2) | no row found for the constraint; `tl2090cq`, `tl2090sn`; the `structure` split: no row found | `editable.structure` becomes `rows`, `outline` and `paste`; at 0.4.0 changes write `constraint` |
| 5 | Undo (section 6): `history`, `canUndo`, `canRedo`, stale steps, host-only entries | issue tl#2092; no row found | nothing under the default |

The study's R8 (asking `beforeEdit` again at a drag's release) is dropped (rule 7.5).

### 13. Not in this record

- **Resources and assignments** in host data. A task keeps its display-only `resources` list (decision 0017).
- **Column configuration:** which columns, their order and width, cell renderers, and columns for `custom` members.
- **The field map** (rule 8.4): the next record.
- **A transport.** The chart never calls `fetch` and takes no URL. The host's routes are the host's; a helper that builds a `save` from a base URL can come later without changing this contract.
- **Merging queued commits** into one call, **retries** and an **offline queue.** `save` retries inside itself (rule 3.6).
- **Versions and merging between people.** No revision numbers, no ETags, no operational transforms. The server refuses in `save` (rule 4.10).
- **Calendar edits through `save`.** A person cannot edit a calendar in the chart today.
- **`custom` in the document format.**
- **Comparing a server's computed dates with the chart's** (`tl2087hd`).

## Consequences

- Decision 0019, rule 7 keeps the commit and its `seq` and `source`; the change inside it takes the shape of section 2. Rule 10's queue is built as section 3 says, and `assignKey` becomes something the queue calls, not the host.
- Decision 0016, rule 7 stands for a chart with no `save`. With `save`, a row is keyless only between its create and that create's settle under `keys: "server"`, and never under `keys: "chart"`.
- `docs/WASM_API.md` (`App.take_commits`) and the README's `update` paragraph state one unit after step 1.
- `ErrorSource` gains `"save"`. The README gains "Connect to your backend", which is the first section of this record.
- The React, Vue and Angular components stop listing `tasks` and `links` among the props that start the chart again.

## What could not be verified

- Bryntum's own documentation pages did not open (the guide and API pages returned a 404 or a site overview). That its sync is one batched request with per-store `added`, `updated` and `removed`, and that a created record is answered with `$PhantomId` and `id`, is verified from a tutorial on the vendor's blog [S2] and the vendor's skill file [S3]. Whether a sync response may carry other rows the server changed, and what the component does after a failed sync: not verified.
- What P6 and MS Project do with a mandatory date when logic pushes past it (rule 9.4) is from memory: not verified.
- Nothing in this record was run. Every statement about today's behavior comes from reading `origin/main` at `45d86f2`.

## Rows this governs

`tl2091sq` (section 3), `tl2087cp` and `tl2087uc` (section 4), `tl2090cq` and `tl2090sn` (rule 8.2), issue tl#2092 (section 6), and the rows still to be written for steps 0, 1, 2 (links, creates and deletes by key) and 4 (the typed constraint, the `structure` split).

## Sources

- [S1] DHTMLX Gantt, server-side integration: https://docs.dhtmlx.com/gantt/guides/server-side/
- [S2] Bryntum blog, connecting Bryntum Gantt to a planning backend (2025-02-21): https://bryntum.com/blog/how-to-connect-and-sync-bryntum-gantt-to-a-servicenow-project-portfolio-management-planning-chart/
- [S3] Bryntum skills repository, `bryntum-crud`: https://raw.githubusercontent.com/bryntum/skills/refs/heads/main/bryntum-crud/SKILL.md
- [S4] Syncfusion Gantt, data binding: https://help.syncfusion.com/gantt-sdk/typescript/gantt-chart/data-binding
