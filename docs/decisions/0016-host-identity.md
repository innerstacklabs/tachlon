# Host identity: slot ids inside, keys at the boundary

Status: accepted. Decided by karpathy, 2026-11-16 (gantt#2110; devteam-1 answer 1499 accepts the architecture review of the host API, gantt#2076 comment 6938711824, section 1). A line marked `Decided here:` is the drafter's choice where the review left one open; it stands until karpathy overrules it. Amends [decision 0001](0001-task-identity-and-rows.md).

## Context

Decision 0001 makes `TaskId` a slot index that never moves in a session: a delete is a tombstone and undo clears it. That index is only stable inside one session. The document writer renumbers tasks to live display order and writes links by the new positions (`write` and `live_order` in `crates/app/src/document/write.rs`), and `fromTasks` reorders to preorder (`packages/gantt/src/adapters/plain.ts`), so a host that keeps an index across a save or a reload has the wrong task. A task has carried the host's key since gantt#2100 (`TaskMeta.key` in `crates/app/src/meta.rs`; read by `read_task` in `document/read/items.rs`, written by `write` in `document/write.rs`), but nothing maps a key to a task, and nothing refuses two tasks with one key: the plain validator checks only that a key is text (the `TASK` table in `packages/gantt/src/adapters/validate.ts`). The public surface still speaks indexes: the `select` and `move` events (`GanttEvents` in `packages/gantt/src/events.ts`), and the commands, proposals, headless reads and about 30 wasm exports the review lists (not re-read). Code is cited at `origin/main` `6b05b07f`, by file and symbol.

## Decision

1. **The slot id is the only identity inside the engine and the app.** `TaskId`, `DepId` and `CalendarId` stay as decision 0001 has them. `Edit` variants, the history, the solver and the scene never hold a key.
2. **One module owns host keys.** `crates/app/src/host/keys.rs` (beside the other host state; row tl2082lc) holds the one map from task key to `TaskId`, the link keys by `DepId` and the calendar keys by `CalendarId`. `TaskMeta.key` stays the stored value for a task; the map is the only index over it, and no other code scans the meta list or builds a second map. An architecture test refuses a key lookup outside the module.
3. **Keys are opaque text, unique per record kind.** The app never reads a key's content. A task, a link and a calendar may share a text. `Decided here:` an empty key is refused, so that "no key" has one spelling, `null` or an absent member.
4. **A duplicate key is refused at the door with a typed error.** The document reader refuses the document (`DuplicateKey`, naming the key and both positions), and the plain validator lists it as an issue beside the duplicate-`id` check it already has (in `inspect`, `validate.ts`). A deleted task keeps its key and its place in the map, because undo brings it back, so a key held by a tombstone is still taken. `Decided here:` the map is built at load, when any record has a key, and not on the first keyed call: the uniqueness check needs every key hashed at load anyway. The review's sections 1 and 6 disagreed on this.
5. **Every host-facing call and event takes and returns the key.** In `packages/gantt`, an argument that names a task, a link or a calendar accepts its key. Every event, `results()` entry, snapshot record and change carries `key` (text or `null`) and the session slot under the name `task` (or `link`). A key that names no live record is a typed refusal (`unknownKey`). `Decided here:` a task argument is `string | number`, a text being a key and a number the session slot, so the chrome keeps its calls and a host needs no second set of methods.
6. **An index given to a host is session-only, and the docs say so.** `docs/WASM_API.md` and the package README state, at each call and event that carries `task` or `link`, that the number is valid until the next load, open or remount and must not be stored. This covers `results()`: its slot is `task`, session-only (decision 0017 renames the row's `id`).
7. **A task created in the chart has no key until the host gives it one.** Its changes carry `key: null` and its slot (decision 0019), never its code. After the host saves the row, it calls `assignKey({ task: slot }, key)`. `assignKey` sets the key and the map, refuses a key already taken, and is not an edit: no undo step, no commit, no gate, no revision. It also works on a slot that an undo has tombstoned meanwhile, so a redo brings back a keyed task. `Decided here:` the same call takes `{ link: slot }` for a link drawn in the chart.
8. **In plain data, `id` is a handle and `key` is the identity.** `id` is required, unique within one payload, and is what `parent`, `from` and `to` name; it means nothing after the payload is read. `key` is the host's durable identity. `Decided here:` three rules follow.
   - A task with no `key` has none. The component does not copy `id` into `key`, because a snapshot read back would then turn a slot number into a key.
   - The Activity ID column is the member `code`. When `code` is absent it is `String(id)`, as today (`fromTasks` in `plain.ts`); `code: null` is no Activity ID. A host whose `id` is a UUID sets `code`.
   - A snapshot writes `id` as the session slot in text, `key` as the key or `null`, and `code` on every task, so reading it back changes no key and no Activity ID.

## Consequences

- Row tl2082lc creates `host/keys.rs` with the map and the uniqueness check as well as the link and calendar keys, and starts after the refactor gantt#2111.
- Row tl2077ev: a change's `key` is the host key or `null`. The row text's "else its code" is removed.
- Row tl2081rs: `results()` names the slot `task` and says it is session-only.
- gantt#2087, gantt#2091 and gantt#2092 use the map and `assignKey`; none builds its own.
- The document's baseline stays keyed by position (`crates/app/src/document/read/baseline.rs`; not re-read): the document is storage. A host-facing baseline (gantt#2086) is keyed.
- Cost: one string hash per keyed record at load. The pull request that adds the map records the open time from plain data with keys at 50,000 tasks (the review's section 6).
- Compaction is still out, as in decision 0001. When it arrives its `Remap` is applied to the map inside `host_keys.rs`.

## Rows this governs

gantt#2082 (tl2082lc), gantt#2077 (tl2077ev, tl2077sn), gantt#2081 (tl2081rs), gantt#2086, gantt#2087, gantt#2091, gantt#2092.
