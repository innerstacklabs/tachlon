<!-- ANCHOR: book-head -->
# Architecture

A map of the crates, the direction their dependencies run, the layers a frame passes through,
and the invariants each part keeps. It does not list methods: the code and its doc comments say
what a function does. Where the code is not enough there is a decision record or a page:

| Read | For |
|---|---|
| `docs/WASM_API.md` | The wasm exports (public and test-only), the theme tokens, the JSON shapes. |
| `docs/decisions/` | 0001 task identity and rows; 0002 outline and summary rows; 0004 format crates licensed separately; 0005 users first, component later; 0007 the hosted app is a separate project; 0011 the cloud API lives in `apps/cloud`; 0012 embeddable component. Number 0006 is taken (the name), 0003 is not published here, and 0008 to 0010 have their own records; each is accepted. |
| `crates/app/tests/architecture/` | The checks that keep this page, the wasm surface and the file sizes true (see "Checks"). |

## Crate dependency direction

| Crate | Workspace dependencies | Role | Licence |
|---|---|---|---|
| `engine` | none | Schedule model, calendars, critical-path solve, fixture generator | `MIT` |
| `timeline` | `engine` | Viewport, scene (a CPU draw list), wgpu renderer, theme | `MIT` |
| `xer` | none | Primavera P6 XER reader (table layer, typed layer) | `MIT` |
| `import` | `engine`, `xer` | Maps MS Project XML and XER into an `engine::Schedule` | `MIT` |
| `app` | `engine`, `import`, `timeline` | `Session` (the native-testable app), and `crates/app/src/web/`, the wasm entry | `MIT` |

The workspace licence is `MIT` (`Cargo.toml`, `[workspace.package]`), and so is every crate. `xer` has no dependencies,
`forbid(unsafe_code)`, and builds for `wasm32-unknown-unknown`, so it uses no file, thread or clock API. `import` and `app` enable its
`synth` feature in `[dev-dependencies]` only, for test input.

The direction is `engine <- timeline <- app`, with `app` also calling `import`, which calls `xer`.
`engine` depends on neither `timeline` nor `app`. Cargo refuses a cycle in `[dependencies]` but
allows one through `[dev-dependencies]`, so `crates/engine/tests/dependency_direction.rs` reads
`crates/engine/Cargo.toml` and fails if any table names `timeline` or `app` (by key, by a
`package` rename or by path). `crates/xer/tests/licence_boundary.rs` fails if `crates/xer` gains
a dependency or a licence other than `MIT`. Both read text, not `cargo metadata`,
so neither sees a rename inherited from the workspace table.

## Layers

<!-- ANCHOR_END: book-head -->
```
apps/hosted (host page, samples)   loads only gantt-core = the built packages/gantt (decision 0007)
   |                                and @gantt/chrome = apps/chrome (the toolbar, the file controls and the Filters menu and the find box, decision 0012)
   |        \
   |         store.ts only: the cloud API /api/docs = apps/cloud (server code, a project of its
   |                        own; decision 0011; the chart below never names it)
packages/gantt/src (TypeScript: DOM, events, requestAnimationFrame)
   |  calls only the exports listed in docs/WASM_API.md
crates/app/src/web/     (wasm32 only: App, LoadedSchedule; owns canvas and Renderer)
   |
crates/app/src          Session: Schedule + Solution + Timeline + outline, filters, history
   |            \
crates/timeline          crates/import -> crates/xer
   |            /
crates/engine           Schedule, Calendar, compute / recompute_cone_moved
```

<!-- ANCHOR: book-mid -->
One frame (`App::frame`): the browser's `requestAnimationFrame` calls `tick` in
`packages/gantt/src/loop.ts`; `App::frame` applies a pending drag, asks `Session::frame_scene` for the
`Scene` (the scene, the dependency arrows, the move outlines, the link overlay, then the bar
labels), and passes it to `Renderer::render`. The loop asks for a frame only while something changes:
it draws until the status has not changed for half a second (`QUIET_MS`) and then sleeps, with no
frame requested. Any call into the app from outside a tick wakes it, and the loop draws every frame
while a drag, a slide, the moved note or a measured run is going. `Status.frames` does not advance
while it sleeps.
<!-- ANCHOR_END: book-mid -->
<!-- ANCHOR: book-tail -->
The task table reads the same `Timeline::order` and `Solution::dates` through
`App::table_rows`, which delegates to `Session::table_rows_json`. Nothing in `packages/gantt/src` keeps a row list or a copy of the dates.
If `App::frame` throws, the frame guard (`packages/gantt/src/frame-guard.ts`) shows the error banner,
keeps the last good frame and requests the next one; it stops after three failures in a row, or
at once after a panic, and offers a reload only when the page that holds the chart gave one (the
developer page does). On an automatic WebGPU run a failed frame is not retried: the chart switches
to WebGL2 in its root (see "Embedding"), or, after a panic, the developer page reloads on WebGL2
and a host page sees the banner and a `fatal` `error` event. A panic stops every chart on the
page, since they share the module. Nothing is saved to browser storage (J9). The test-only
`App.test_fail_next_frame` forces a failure in one chart.

## What each part keeps

**`engine`.** Pure Rust, no wasm. Every date is an `Instant`: whole minutes from a project origin
that is a Monday 00:00. A duration or a lag is working minutes of a calendar and exists only
inside calendar math. `Schedule::compute` is the full solve. `Schedule::recompute_cone_moved`
re-solves the forward cone of one changed task and reports the tasks whose early dates moved and
where they were (`Moved`: the task with its old early start and finish);
`recompute_cone` is the same call without the report. A brute-force minute-by-minute oracle
checks `compute` (`crates/engine/tests/brute_force.rs`), and `crates/engine/tests/cone.rs`
checks that the cone path equals the full solve, and `crates/engine/tests/tombstones.rs` that a schedule with
deleted tasks solves like the same schedule built without them. `LagCalendar`
picks, per schedule, whose working time every lag is counted in. `Task::limit` (`Limit`: start-no-later-than, finish-no-earlier-than, finish-no-later-than, and a `Pin` to start or finish on a date) bounds the passes: a late limit no date meets is a negative float (critical), not an error. A pin binds no dependency: the tasks before it get the negative float. `Limit::resume` floors the early start of a started task's remaining work at a date. `Limit::alap` (as late as possible) is a post-pass after the forward pass: last task first, each unstarted marked task takes the latest start that moves no successor, so a schedule with one takes a full `compute()` from every cone. A deleted task (`Task::deleted`, `Schedule::delete_task`) keeps its id and slot:
`topo::build` leaves it and every dependency touching it out of the graph, so the passes
never see it, its `Solution` slot reads zero, and any operation on it is
`ScheduleError::UnknownTask` (decision 0001, amended). A person edits a link through
`add_link` (refuses a self link, a repeated pair and a loop, naming the loop's path),
`remove_dependency` and `restore_dependency`; a removed link keeps its `DepId` and is out of the graph
like a dead one (`crates/engine/src/links.rs`, `crates/engine/tests/links.rs`); importers keep the
unchecked `add_dependency`. The engine knows no outline, no
summaries, no rows and no calendar dates.

**`timeline`.** Everything except `render` is plain Rust with no GPU, so it is tested natively.
`Timeline` owns the viewport, the pointer state and the row order (`Timeline::order`, row to task
index; `row_of`, task index to row, `HIDDEN_ROW` for a task with no row). `scene::build` turns
the viewport, the order and the task dates into an owned `Scene`; `Renderer` draws a borrowed
`Scene`. Every colour comes from the `Theme` handed to the builders (decision 0005: its values
are the `--gantt-*` CSS tokens listed in `docs/WASM_API.md`). `Summaries` holds the span of each summary row; `Timeline`
owns it and `Session` is its only writer. A `Rect` is 40 bytes: `stroke` is the ring width in
physical pixels, 0 for a filled quad. Above 0 the one rect pipeline draws a dashed ring just inside
the quad's edge (a fifth vertex attribute and a branch in the fragment shader: no second pass). The
move outline (`scene::add_move_outlines`) is one such rect per moved visible bar at its old extent,
at most 400 per frame (the ones nearest the canvas centre), spliced in under the bars and under the
arrows. A proposed edit's ghost (`scene::add_ghost_outlines`) is the same ring appended after the
bars, 2 px wide at 70% alpha, so a ghost inside its own bar shows and reads apart from an outline. A task of
zero length gets a diamond ring, like its bar: the shader dashes a diamond ring along its edges,
with a dash on each point, and draws it solid when an edge is shorter than two dash periods (the
normal row size), so it is always a whole diamond. The ring and the bar's own diamond keep their whole box at the canvas edge.

**`xer`.** `parse` is the table layer (encoding detection, `%T`/`%F`/`%R` rows, warnings and
errors); `read` is the typed layer (owned rows, P6 field names, `Other(String)` for codes a newer
P6 adds). A bad value in a key field is an `Error`; anywhere else it is `None` plus a `Warning`.
`xer::synth` (feature `synth`) writes test input and is not an exporter.

**`import`.** `parse_mspdi` and `parse_xer` return an `Imported`: an `engine::Schedule`, one
`ImportedTask` per engine task in the same order, and `unsupported` lines. What the importer
cannot apply is counted and reported in `unsupported`, not dropped silently. It never computes the
schedule. The origin is the Monday on or before the earliest plan start. Each XER project becomes
a root summary, its WBS nodes become summaries, and its activities become leaves. The lag calendar
comes from `SCHEDOPTIONS`; a project that does not state one gets `Predecessor` and a note.
An XER calendar exception with working periods (a shortened day, a working Saturday) replaces
that date's weekly periods; a second exception on one date is dropped and counted.
An MSPDI `DayWorking=1` exception's `WorkingTimes` replace that date's weekly periods; a repeat date is
dropped and counted.

**`app`.** `Session` owns the `Schedule`, the `Solution`, the `Timeline`, the task display data
(`TaskMeta`, which `Session::new` and `Session::reopen` take), the last search results (`find`
and `find_go`), the collapse flags, the selection, the row filter, the lookahead window and the
edit history. The JSON the page reads (the status line, the table rows and the hover card) is
written by `Session` in `json.rs`; `App` fills a `RenderFacts` (backend, surface, scene counts,
`epoch`, `frames`) for the status. `Session::frame_scene(measure)` is the one place a frame's
draw list is composed; `measure` is the label-width closure the renderer supplies, so a native
test passes a fixed width. `Session::display_dates` is the one place positions are read for
drawing: the bars, the arrows, the bar labels and the link overlay of `frame_scene` all take that
slice, so a moved task slides as one picture, and the solution under it is never blended. The `web` module and its wasm-bindgen exports exist only under
`cfg(target_arch = "wasm32")`; `Session` and everything it uses compile and test natively. `App`
adds the canvas and the renderer, and holds no schedule data of its own. The invariants:

- *`Session`'s fields are private.* Code outside it reads the timeline, `revision`, `stamp`, the
  last drag result and the chain-build count through `Session::timeline`, `revision`, `stamp`,
  `drag_info` and `chain_builds`, and moves the view through `Session::resize`, `wheel`,
  `scroll_by`, `set_hover_row`, `pointer_leave`, `fit` and `scroll_to_row` (all in `view.rs`).

- *One edit path.* `Edit` (`Move`, `SetDuration`, `Rename`, `Insert`, `Delete`, `Restore`, `Reparent`, `AddLink`, `RemoveLink`, `RestoreLink`) is the one kind of change, and it
  names a task by `TaskId`, never by row (decision 0001); `Insert` carries a `Place` and a name.
  `Session::run` applies an edit and returns its
  inverse; `Session::edit` records that inverse for undo (`HISTORY_CAP`, 200) and clears redo. A
  drag is an `Edit::Move`: `Session::constrain` sets the start-no-earlier-than and calls
  `recompute_cone_moved`. `SetDuration` calls `recompute_duration_cone_moved`, whose backward pass covers the
  task's own late dates and its predecessors' float, and rolls up the moved summaries (the roll-up
  rebuilds every summary when more than half the tasks moved). The duration cone falls back to a
  full `compute` when it is wide, when the project finish moves, or when an early start lands past
  its late start; the call returns a `DurationCone` (`Cone`, `Fallback(FallbackReason)` or
  `Replaced`) that says which. The call is exact because the solution equals `compute()` before the edit: moves
  and duration edits preserve that equality, insert, delete, restore and link edits full-compute,
  and a rename leaves the schedule unchanged.
  `Rename` changes the session's `TaskMeta::name` only and recomputes nothing. A person's rename
  goes through `Session::edit`, which refuses an empty name, a control character and `" / "`;
  `apply` and undo store a name as given, so an imported name that holds `" / "` comes back byte
  for byte. `Insert`, `Delete`, `Restore` and `Reparent` also do a full `compute`, then rebuild the outline, the summary
  spans and the rows (see "Where rows come from today"). `Reparent { task, place, siblings }`
  (`crates/app/src/reparent.rs`) moves a task with its subtree to a `Place` and changes no task's
  dates. The subtree's levels shift by one delta; `Siblings::Adopt(n)` first makes the next `n`
  siblings the task's last children, and `Release(n)` hands its last `n` children back as its next
  siblings, so outdent (`Session::outdent_selected`, which adopts the siblings below, as MS
  Project does) and its undo are one edit each. Its inverse is a `Reparent` to the old place with
  the opposite `Siblings`, and history's private `Meta` carries the old level, so undo restores a
  level the parent does not imply (an import can hold a gap) and the same preorder position.
  `Session::indent_selected` puts the row last under its previous sibling. Both refuse, changing
  nothing, with no previous sibling, at the top level, under a row filter or lookahead window,
  during a bar press, and when a leaf with a link would become a summary. The link edits (`crates/app/src/edit/links.rs`)
  refuse a busy session and a summary end (`LinkToSummary`), let `Schedule::add_link` and
  `restore_dependency` judge a repeat (`AlreadyLinked`) and a loop (`LinkCycle`, whose message names
  the loop and elides its middle past five tasks), then run a full `compute`, the summary roll-up
  and `rebuild_links` (live links only), and roll the schedule back if the solve fails. `AddLink`
  returns `RemoveLink`, `RemoveLink` returns `RestoreLink` and back, so undo and redo keep the
  link's `DepId`. Readers of the dependency list (outline, insert, highlight, details) use the
  live links only.
- *A name is the task's own.* `TaskMeta::name` is the name the file gave (`Pour slab`), kept byte
  for byte except that a control character becomes a space and a blank name becomes `Task <uid>`;
  an imported `" / "` stays. The path the table, the hover card and find use is derived:
  `Session::outline_path` joins the names of the ancestors and the task with `" / "`, so renaming
  a parent changes its children's paths and none of their names. The bar labels and the name
  field of the hover card use the task's own name. A path with a `" / "` in a name does not split
  back into names; nothing needs it to.
- *Rows are derived in one place.* `Session::visible_rows` walks the outline preorder once and
  applies collapse, the row filter and the lookahead window together. The collapse flags live in
  `Session`, not in `Outline`. The rows reach the `Timeline` only through `Timeline::set_order`,
  called from `with_outline`, `toggle` and `apply_rows`. With a row filter or a window on,
  `follow_filter` refreshes the rows after an edit, an undo or a drag; while a bar press is active
  it only marks them stale, so the dragged bar cannot leave the rows under the pointer.
- *Summaries follow the solution.* Every path that changes the solution goes through
  `Session::run`, which rolls the summary spans up again (`rollup_moved` after a `Move` or a
  `SetDuration`, `rollup_full` after an insert, delete, restore or link edit), so undo and redo reach them the same way the edit did.
- *The move report.* After an edit, undo or redo that moves other tasks, `Session::move_report`
  (`crates/app/src/moved.rs`) holds `{ seq, text, others, finish_days, slide, was }`: the tasks
  other than the edited one whose early start or finish changed, with the early dates they had
  before (the engine's `Moved` list includes the edited task, so the report leaves it out), and the
  days the project finish moved (the difference of the calendar-day numbers of the old and new
  `project_finish`). A drag runs one `Move` per frame, so a journal keeps the first old dates of
  each task across the frames (a visited vector sized at the press, nothing allocated per frame);
  the release turns it into one report with the press-time dates and `slide: false`, dropping
  tasks that ended where they began, and a cancel or a drag that returns to its start makes none.
  Any other change, a rename included, clears the report; `reopen` starts with none and carries
  the counter on. `status_json` writes it as `moved`. `Session::add_move_outlines` draws the report's
  old places as dashed outlines while `MoveReport::alpha` is above 0. It only appends to the `Scene`:
  it never writes the `Solution`, and it reads the report, never the schedule. The cap on outlines
  does not change `others` or `text`, which count every moved task.
- *The slide.* `solution` is never blended: it is the full solve after every edit, during a slide and
  after it. A slide draws from one scratch copy of `solution.dates` (`crates/app/src/moved/slide.rs`,
  state in `Reports`) in which `timeline::slide::apply` writes the moved tasks that have a row.
  `Session::display_dates` is that copy while a slide runs and `solution().dates` itself, the same
  slice, otherwise. `Session::advance_motion` is the one step, called once per frame by `App::frame`, after `apply_drag`: it snaps a slide whose `stamp` is not
  the session's, which covers every
  path that replaces the solution, the edits that make no report included, and a change of rows; it starts a slide on the
  first frame that sees a report with a new `seq`, `slide` true, reduced motion off and a moved task
  with a row (the clock of that frame, not `MoveReport::at`); and it drops the copy at the end. A
  report that starts no slide still counts as seen, so one made with reduced motion on never plays
  after it is switched off. `Session::set_reduced_motion` is the page's setting: `reopen` carries
  it on and drops the slide. `Session::slide_copy_ms` is how long the last copy took.
  An open starts a slide of its own (gs#309): `App::open` calls `Session::mark_open`, which enters the tasks of the
  rows in the window, no others, with a was-extent of no width (`was_ef == was_es`), through
  `Session::start_slide_from`; the bars grow from start to finish over the usual 360 ms, and the copy already holds the
  first frame. Reduced motion starts none. A wheel, a scroll that moves, a row jump or Fit drops it, as an edit does
  through the stamp. The cost is one copy of the dates per open and writes bounded by the rows in the window.
- *Dates.* The engine has instants only. `dates::Origin` is the one place an instant becomes a
  calendar date; the file supplies the origin, the demo uses `Origin::DEMO`.
- *Opening a file cannot lose the schedule on the page.* `load_bytes` keeps no state, and
  `Session::reopen` builds everything that can fail before anything is swapped, so a failed open
  leaves the previous schedule as it was. The engine runs as WebAssembly in the reader's browser
  and reads the file there.

**`packages/gantt/src`.** TypeScript owns the DOM, the events and the animation-frame loop. It calls only the
public exports of `docs/WASM_API.md`; a `test_*` export is used only in the page's "Test-side
files", and the test hook (`window.gantt`) is named only in `packages/gantt/src/test-hook.ts`. A host reads the view state a toolbar shows through `Gantt.state()` and hears it change through
`on("change")`; both come from the `Status` the loop reads once a frame (`packages/gantt/src/view-state.ts`), which
compares the few fields it carries and builds a new state only when one moved, so the event fires at
most once a frame and an idle frame allocates nothing for it. The chrome's find box follows the
filters through that same event. A host drives find, the outline levels, the rows, the data date and
the baseline through `Gantt.commands`; each edit command answers `{ ok }` or a named refusal
(`packages/gantt/src/edit-result.ts` maps the app's `EditError::kind`), never a throw. The chart's find and
status keys fire `on("find")` and `on("toggle-status")`, its open and keys keys `on("choose-file")` and `on("toggle-keys")`. The Filters menu and the find box are the
app's chrome in `apps/chrome/` (`@gantt/chrome`, private), with the baseline, Insert and Delete, level, zoom, data date and row count controls, and the file controls: Open (the button, Ctrl+O, a drop and the project list), Export, Checks, the keys list, the import notes, the unsaved mark, and the tab's title and unload question, which read `state().fileName`, `unsaved` and `importNotes`. A page calls `installChrome(gantt, root)`
after `mount`, and the chrome puts its elements into the empty `filters-menu`, `find`, `file-tools`, `file-notice`, `keys-tools`, `file-dialogs`, `baseline-tools`, `row-tools`, `zoom-tools` and `readouts` parts of
the chart's toolbar and root, drives the chart through `commands`, `open()`, `state()`, `filterInputs()`, `exportMspdi()`, `checks()` and `busy()`,
and hears `change`, `edit`, `move`, `open`, `error`, `find`, `choose-file` and `toggle-keys` like any host. The chart reads, parses and swaps in a file (`packages/gantt/src/open-schedule.ts`) and keeps the question before an open over unsaved changes; it writes no tab title, adds no `beforeunload` listener and shows no file control (`packages/gantt/tests/bare-chart.spec.ts`). A switch to WebGL2 gives the
chart a new toolbar, so on `error` with the source `backend` the chrome puts the same elements into
the new parts. The developer page and the hosted app both install it. The status line reads run
facts the API does not give, so it is the developer page's (`packages/gantt/src/status-line.ts`, handed to
the chart as a page knob, shown and hidden on `toggle-status`); a host page has none of it. The hosted app has a smaller speed line of its own (`apps/hosted/src/speed-line.ts`): it hears `toggle-status` and reads only `Gantt.speed()`, which calls no `App` method, so it polls four times a second while shown and a sleeping chart stays asleep. The loop
syncs the chart's own scroll to a filter's rows (`packages/gantt/src/filter-scroll.ts`, which reads drawn bars the API does not give). A chart mounted without `installChrome` has none of these controls (`packages/gantt/tests/bare-chart.spec.ts`). The ID column is the table's
(`packages/gantt/src/id-column.ts`), not the toolbar's.

## Theming

The chart's colours are its theming API (decision 0005): one `Theme` struct in
`crates/timeline/src/theme.rs`, and the same values as `--gantt-*` CSS custom properties on the
chart root. The token list, and how `set_theme` works, are in the Theming section of
`docs/WASM_API.md`.

With no attribute the chart follows the system's light or dark setting. `data-theme="dark"` or
`"light"` on the root forces that set whatever the system says (`packages/gantt/src/styles/tokens.css`). The
system's set is `:where(.gantt):where(:not([data-theme]))` inside the media query, and the two forced sets
come after it; all three have zero specificity, so a host's overrides win over each. `watchTheme`
observes `data-theme`, so a change recolours the canvas once, as a style edit does. The hosted page
reads `?theme=dark|light`, sets the attribute on `#app` and its own `color-scheme` on `<html>`, and
ignores any other value.

## Embedding

The chart is a component that a host page places in a box of its own (decision 0005). Three rules
hold:

1. *The chart is as big as its container.* Nothing in `packages/gantt/src` or the page's CSS reads the
   window's size, except `measure.ts`, which only records it in its report. The panels inside
   the root are positioned against it.
2. *Keys act only while focus is inside the root.* One `keydown` listener on the root; nothing
   listens for a key on `window`, `document`, `document.body` or `document.documentElement`.
3. *Every style rule is under the root class.* Each selector in the `<style>` blocks of
   `packages/gantt/index.html` and `packages/gantt/src/chart.html` and in `packages/gantt/src/**/*.css` starts with `.gantt` or
   `:where(.gantt)`.

The architecture checks keep rules 1 to 3 (see "Checks"), and `packages/gantt/tests/embedding.spec.ts` keeps
them in a real browser. The checks read the chart's markup, `packages/gantt/src/chart.html`, as well as
`packages/gantt/index.html`.

The host gives only the root: a `div` with the class `gantt` and a box with a height. A page may
hold several roots, one chart each (gs#1083). The markup names each part with `data-part`, and the
lookups (`packages/gantt/src/dom.ts`) and the style rules find a part by that name inside the root. Ids are
given at mount, per root: the first chart on a page writes a part's name as its id, each further
chart puts `chart-<n>-` before it (a destroyed chart, or a mount that rejects, gives its number back), and the id references in the markup (`aria-labelledby` and the
like) are written the same way, so no id is on the page twice. Each chart has its own frame guard,
and a failed frame counts in its own chart; the charts share one WASM module, so a panic stops
every chart, and each reports it; `packages/gantt/tests/two-charts.spec.ts` keeps this. The chart's markup (`packages/gantt/src/chart.html`)
and its stylesheets (`packages/gantt/src/styles/`) belong to the entry: `packages/gantt/src/index.ts` imports the
stylesheets, and `mount` inserts the markup in the root before it looks anything up.

The chart never reloads the host page. When WebGPU fails, at the start or in a later frame, the
chart takes down its canvas and renderer and starts again in the same root, forced to WebGL2, on
the schedule it held (`packages/gantt/src/webgl-switch.ts`): the schedule `mount` was given (a file, the demo or the stress fixture) if nothing was
opened, edited or left unsaved, else the document `toDocument()` returns (a baseline set or
cleared moves no revision, but it is unsaved, so it is kept), with the unsaved mark still on when
it was on. A chart given `tasks` and `links` always starts again on that document: `mount` keeps neither list once the first start
has finished (a large list is the biggest thing the page's JS heap holds), and `fromTasks` read them into the document that start loaded. When `toDocument()` throws on a chart mounted with `tasks` and no `schedule`, no list is left to start from, so the WebGL2 switch fails through the `MountError` path (stage `"schedule"`): the chart shows the failure and reports a `fatal` error. A measured run (the developer page's `?measure`) is the exception: it starts again on the page's own schedule and
runs its script again, as the reload it replaces did, because the stress fixture's head task that the drag phase needs is not in a
document. A document of a session with no outline (the stress fixture) carries no parents, so it reads. A start that throws part-way runs its disposers and frees its app. The new start replaces the root's children. The `Gantt` the host holds drives the new
start, its `on` handlers carry over, and one `error` event with the source `backend` gives the
reason. The view, the selection and the undo history start fresh. A panic leaves the WASM module
unusable on the page, so it cannot switch: the chart reports a `fatal` error and shows its
message, and the host decides whether to reload. The chart's own listeners on the root go
through `listening` (`packages/gantt/src/shell.ts`), so `destroy` takes them off and leaves the host's own.

## Task-indexed and row-indexed structures

`TaskId.0` indexes `Schedule::tasks` and `Solution::dates`; `DepId.0` indexes the dependencies. A
row is a display position and passes through `Timeline::order` to become a task index. Both
domains hold one value per task today, so an identity order could hide a mix-up in a small fixture.
The generated schedules therefore permute task ids, and `order_by_start` sorts by early start.
Table JSON carries the task id; the page derives the row from the slice it asked for.

## Where rows come from today

- *Demo and stress fixtures.* `demo` and `LoadedSchedule::stress` call `order_by_start` once after
  the first solve.
- *A file, or the demo page.* `Session::with_outline` installs the `Outline` (a forest over
  `TaskId` built from `TaskMeta::parent`; it refuses a cycle, a length mismatch and a dependency
  that touches a summary) and replaces the order with the outline preorder. `App::open` builds a
  new session with `Session::reopen`, which starts with no selection, filter, window or collapsed
  summary, and takes the new file's `TaskMeta`.
- *After that, `apply_rows`.* A disclosure toggle (`toggle`), the level commands
  (`collapse_to_level`, `expand_all`, `collapse_selected`, `expand_selected`, `expand_selected_branch`), the row filter
  (`RowFilter` has seven values, one at a time: `Off`, `Critical`, `NearCritical`, `LongestPath`,
  `Slipped`, `Moved` and `ActivityCode`) and the lookahead window (independent of
  the filter) all change what `visible_rows` returns. The window is the half-open span
  `[start, start + weeks * WEEK)`; a task intersects it when `es < end` and `ef > start`, using
  early dates, because those are the dates the table and the bars show. A summary stays as context
  when a kept leaf is in its subtree. Find and `go_to` skip hidden tasks, and `go_to` expands the
  collapsed ancestors first.
- *A reparent* (indent, outdent, undo of either) rebuilds the outline and the rows with the same
  `rebuild_structure` as insert and delete, then `go_to` selects the task, which opens a
  collapsed new parent; the selection follows the task by id.
- *A rename* changes names, not rows. Find matches the derived outline path, so a parent's name still finds its children.
- *The selection* is keyed by task id: its `row` is `HIDDEN_ROW` while a collapsed summary hides
  it. `LinkIndex::new` leaves out a dependency that is not live (removed, or an end deleted) or whose end has no row.
  A link is selected the same way, by its dependency id (`Session::selected_link`), and the two
  selections are independent: a click on an arrow selects the link and leaves the task, a click on
  a bar selects the task and clears the link, a click on nothing and `clear_selection` clear both.
  `AddLink` and `RestoreLink` select their link and `RemoveLink` clears the selection, so undo and
  redo carry it like a task's. `rebuild_links` drops a selected link that is no longer in force or
  whose ends have no row.
- *The pointer.* A press is one of three things: a bar press (a drag, or a click that selects), a
  pan, or a link press (`press/link.rs`). A link press starts on the link terminal (the dot just
  outside the finish end of the hovered or selected bar, drawn by `add_link_overlay` and
  hit-tested with the same geometry by `terminal_contains`) or, with Alt, anywhere on a leaf bar.
  Past the drag threshold it is a link drag: the bar under the pointer is the target, and its
  verdict (a summary, a repeat or a loop, in the words `Session::edit` refuses with) is worked out
  once per target change and cached, so the loop search runs once per target, not per pointer move.
  The release adds the finish-to-start link through `Session::edit` (one history step), or leaves
  the verdict in `link_refusal`; under the threshold it only selects the task; a cancel adds
  nothing. A link drag is *busy* (`Session::busy`) everywhere a bar press is: undo, redo, edits to
  rows and links, `next_critical` and find's go-to wait for it.

- *Delete and restore.* `Edit::Delete` tombstones a leaf and `Edit::Restore` brings it back at a
  `Place` (a parent and the sibling it follows); each is the other's inverse, so undo and redo
  reuse the ids. Both need an outline (`EditError::NoOutline`) and no bar press
  (`EditError::Busy`), and a summary is refused (`EditError::HasChildren`); a parent that loses
  its last child reads as a leaf. They end in `rebuild_structure`, which builds the outline over
  the live tasks (`Outline::new` takes live ids; its arrays stay sized by slot count) and runs a
  full `compute` before changing anything. A Delete of the selected task selects the next visible
  row's task, else the previous visible one, else none, chosen after the recompute from the rows
  the active filter still shows; a Delete of any other task keeps the selection. Restore selects
  the task and expands a collapsed parent, but only when a row filter leaves the task visible: a
  hidden restored task leaves the selection alone and the filter on. `Edit::Restore` has no level
  field: a person's restore takes the level from the parent it names. History's private inverse
  records keep the parent and level the task held before each step, so every undo and redo puts
  back exactly the meta before that step. A deleted task reads
  as absent through every `Session` reader (`task_label`, `outline_path`, `task_details`, ...), and
  `Session::edit` answers `UnknownTask` for it before it checks a typed name.
  `Session::task_count` is the live count, not the slot count.
- *Insert.* `Session::insert_below_selection` adds a leaf named `New task` as a sibling after the
  selected task's whole subtree, at its level (`Session::place_below`); with nothing selected it
  is the last top-level row. It takes the next free id, 1 working day of the calendar of the
  task it follows, no constraint and no activity ID, and it ends in the same `rebuild_structure`
  (full `compute`). The new task is selected, expanding a collapsed parent. Insert is refused
  under a row filter or lookahead window (`EditError::FilterActive`: the row would not pass and
  would vanish), during a press, and without an outline. Its inverse is `Delete`, whose inverse
  is `Restore`, so undo and redo keep the id and a later insert takes a new one; undoing an
  insert follows the Delete rule for the selection.

## Checks

`crates/app/tests/architecture/` is a module of the `it` test binary; `exports`, `web_sources`, `entry`, `theming`,
`embedding`, `style_scope`, `size`, `hosting`, `chrome` and `lints` each hold a check and its self-tests. The
directory also holds the `name` check (`name.rs`, `the_old_name_stays_out_of_the_repository`),
`entry_types.rs` and `support.rs`.

- *The wasm surface* (`exports`, `web_sources`): every export under `crates/app/src/web/` is a
  row in `docs/WASM_API.md` with a doc comment; `wasm_bindgen` appears
  in no other `crates/*/src` file; `packages/gantt/src` names a `test_*` export or the hook only where the
  page allows.
- *The page entry* (`entry`): every export of `packages/gantt/src/index.ts` is a row under "Page entry" in
  `docs/WASM_API.md`, and every row has an export; no exported declaration of the entry names a type
  from `measure.ts` or `page-knobs.ts` (the developer page's knobs), so a host page sees no measure type.
- *Theme and embedding* (`theming`, `embedding`, `style_scope`): every `--gantt-` token is on the
  page and used; no stylesheet but `tokens.css` holds a literal colour, except the files in `LITERAL_COLOURS` (`theming.rs`), whose counts only shrink; nothing reads the window's size or listens for a key on the window or document;
  outside `main.ts`, nothing looks parts up on the document (`getElementById`, `querySelector`,
  `querySelectorAll`, `getElementsBy*`) or listens on it; every selector starts with `.gantt`.
- *Size* (`size`): `apps/chrome/src` is source too; flat limits of 400 lines for source, 300 for an in-file test module and 600 for
  a test file. The files over a limit when the budget began are listed at their size, and the list
  only shrinks. The budget covers `apps/hosted/` too: its `src`, `samples`, `scripts`, page and
  `*.config.ts` are source, its `tests` are test files, and `apps/hosted/dist` and
  `apps/hosted/public/samples` are skipped as generated.
- *The hosting boundary* (`hosting`, decision 0007): the component (`crates/*/src`,
  `crates/*/Cargo.toml`, `scripts/build-web.sh` and every file under `packages/gantt/` at any extension,
  but `node_modules/` and the paths `packages/gantt/.gitignore` names: `packages/gantt/src/wasm/`, `packages/gantt/dist/`,
  `packages/gantt/test-results/`, `packages/gantt/playwright-report/`) names nothing about hosting: not `apps/hosted`,
  `pages.dev`, `gantt-app`, `sample=` or `samples/`, and no import specifier holds
  `apps/`. Comments count. `apps/hosted/src` and `apps/hosted/*.config.ts` import only
  `gantt-core` (only its `gantt.css` under it), `node:` modules, relative paths inside `apps/hosted/` and the
  packages `apps/hosted/package.json` declares (it declares `gantt-core`), and so do the `<script src>` and `<link href>` of
  its page, its `import.meta.glob` patterns (also when the call has a type argument, as in
  `import.meta.glob<string>("./x/*.ts")`) and its CSS `@import`s. `vite.config.ts` and `tsconfig.json` name neither `@gantt/chart` nor `gantt-core`: no alias sends the app to the source.
  `apps/chrome/src` follows the same rule against `apps/chrome/package.json`. The `web_sources` rule holds in `apps/hosted/src`; no selector there starts
  with `.gantt`. An empty parse fails. The threat model is the one in `main.rs`: a specifier built
  at run time is not caught.
- *The cloud API has one door* (`hosting`, decision 0011): no file of the component holds
  `/api/docs`, and in the scripts under `apps/hosted/src` only `store.ts` holds it or imports a
  path inside `apps/cloud`; neither `package.json` declares a dependency that resolves into
  `apps/cloud`. Inline page scripts, non-script files and `public/` are not read; review holds
  them to the rule. Same threat model as above: a route built at run time is not caught.
- *The chart without its chrome* (`chrome`, decision 0012): no file of `packages/gantt/src` (but the generated
  `packages/gantt/src/wasm/` and the developer page `main.ts`) imports, re-exports or dynamically imports a
  relative path whose file is `toolbar`, `filters-menu`, `find`, `status-line`, `open`, `export`, `keys-help`, `unsupported`, `tab` or `save-mark`, from any
  directory. Those are the app's chrome, which lives in `apps/chrome/` and drives the chart only
  through its public data API, commands and events. The imports `main` had when the check began
  were its `EXEMPT` list, one `path | import` line each, sorted bytewise; the list is empty now (gs#1087 moved the toolbar, gs#1088 the file controls), and an
  import of a chrome module fails, a listed import that is gone fails until its line is deleted, and an
  unsorted, repeated or malformed line fails. The API stories gs#1194 and gs#1195 deleted the lines
  they ended, the first part of gs#1087 the find box's, the Filters menu's and the status line's, and
  its second part the toolbar's: the list is empty now. An empty parse fails.
- *The retired names* (`name`, decision 0013): no tracked file or path names the working code name
  or the product name that followed it, in any case. The decision records (`docs/decisions/`) are
  history and are not searched. Everything else that still does is an entry in
  `crates/app/tests/architecture/name/allowlist.txt`, sorted: the list's own path, or one exact text
  of one file with its reason (`path | text # why`: the infra names another change renames, and a
  script that reads a decision record). The list only shrinks: an entry that no
  longer matches fails, and so does a listed path that is not tracked. A line of a `docs/**/*.md`
  file that ends with `<!-- verbatim -->` is a quote and is not searched. An empty file list and an
  unreadable, unsorted or malformed allowlist fail. The product's name is written only at the edges
  (`README.md`, `book/`, `docs/PRODUCT.md`, `docs/decisions/`, `packages/gantt/src/product.ts`,
  `packages/gantt/index.html`); every other file reads `PRODUCT_NAME` from `packages/gantt/src/product.ts`, and a tracked
  file or path elsewhere that names it fails (gs#1120).
- *The strict lints* (`lints`, gantt#408): each `Cargo.toml` named in the root `members` holds
  `workspace = true` under `[lints]`, so the rules in `[workspace.lints.clippy]` reach every crate. A
  member without it fails by name, and a root that names no member fails. It runs inside `cargo test`.

Also: `crates/engine/tests/dependency_direction.rs` and `crates/xer/tests/licence_boundary.rs`
(above), and the Playwright specs in `packages/gantt/tests` (`embedding.spec.ts` keeps the three embedding
rules in a real browser).
<!-- ANCHOR_END: book-tail -->
