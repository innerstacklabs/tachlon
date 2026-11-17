# What Export writes and what Save means

Status: accepted 2026-10-28 by karpathy, items 1 to 5 as recommended (gs#703, story 1 of the save and share plan). Item 6 is superseded by [decision 0010](0010-document-format.md) (Dave, 2026-10-28): Ctrl+S saves a Tachlon document, and writing XER or MSPDI is always Export. Item 3's narrowing of J9 step 3 stands until a second-version XER file is in the repo.

Written 2026-10-28 (gs#703, row gs703p). It settles assumptions A2 and A3 of PRODUCT.md, section 7, as proposals, and answers the question decision [0001](0001-task-identity-and-rows.md) left for J9.

## Context

`main` opens a P6 XER or an MS Project XML file and edits it. It writes neither format. The workflow 4 plan on gs#703 lists twelve stories that build the writers and the Save step; its stories 2 to 8 each assert one of the answers here, so the answers come first.

What is already ruled, and not re-decided here:

- Decision [0004](0004-format-crates-licensed-separately.md): `crates/xer` is its own dependency-free crate, licensed `MIT OR Apache-2.0`; the mapping into the engine model lives outside it; the crate is read-only and best effort "at first"; "Writing XER comes with the scheduler journeys (J9)".
- Decision 0003: XER export comes with the scheduler journeys; `.mpp` is out of scope.
- Decision [0001](0001-task-identity-and-rows.md): a deleted task keeps its `TaskId` as a tombstone, and a dependency with a deleted end is dead. Compaction "runs when the undo history is empty or is being cleared, such as on save", and also when tombstones exceed 25% of tasks. A later amendment in the same record says compaction is "still out" until the J9 decision settles a `Remap` and its save-time policy.
- PRODUCT.md, section 5, rows 3 and 4: XER is written "at a stated P6 version" and owners require "version match or earlier than GOV"; MSPDI is written with J9. PRODUCT.md, J9: the export dialog "names the P6 version and lists any field that version cannot hold".
- PRODUCT.md, A3 (proposed there, not ruled): XER keeps the original tables it does not model and writes them back unchanged; MSPDI writes only what the model holds and reports the rest.

What the repo shows today, which the answers rest on:

- `crates/xer` parses `ERMHDR` and every `%T` table, detects UTF-8 or Windows-1252, and accepts LF, CRLF and CR line ends. Typed reading covers `PROJECT`, `PROJWBS`, `TASK`, `TASKPRED`, `SCHEDOPTIONS` and `CALENDAR`; every other table survives only as a name and a row count (`NotRead`). `Header` keeps the version, export date and currency cells.
- Every XER fixture in the repo (`web/tests/fixtures/*.xer`) and the `synth` writer carry the version `19.12`. The crate README says the reader was written against public samples of 8.2, 8.4 and 20.12, none committed. So the repo holds one version's layout and cannot tell which fields differ between versions.
- `TaskMeta.code` (the activity ID) is `None` for a task the file gave no ID, and `Session` inserts such tasks.
- The `synth` writer is a test helper and says it is not an exporter.

## Decision

Each item is a recommendation. The alternative and the reason follow it.

1. **Where the row writer lives.** Split by what the code knows. The table-layer writer, which turns tables of strings into XER text (the `ERMHDR` line, `%T`, `%F`, `%R`, `%E`, the encoding and the line ending), goes in `crates/xer`. The mapping from the engine schedule and the app metadata to P6 tables and fields (which fields, which version, calendars to `clndr_data`) goes in `crates/import`, under AGPL, beside the MSPDI and XER importers.
Superseded by gs#1044: Tachlon became closed source (2026-10-31); decision 0014 later made it MIT.
   - *Alternative:* all of it in `crates/import`, so decision 0004 and the crate README stay as they are.
   - *Reason for the recommendation:* the encoding and line-ending rules already live in `xer`, next to the parser that defines them; a writer in the same crate is tested against that parser by the crate's own round-trip tests (the crate already round-trips its test writer in `xer_corpus`). The crate then knows nothing of P6 meaning, so the writer adds no AGPL code to it, and other tools that use the crate get both directions. *Cost:* decision 0004 item 3 ("It is read-only") and the crate README and `lib.rs` wording ("not an exporter") need an amendment, and the crate takes on the best-effort label for writing too. A later row makes that edit; this record only proposes it.
   Superseded by gs#1044: Tachlon became closed source (2026-10-31); decision 0014 later made it MIT.

2. **What an opened XER keeps and replays on export.** Keep the whole source in memory when the file opens, and replay what the person did not change:
   - the `ERMHDR` cells as read, except the export date, which becomes the time of export (it names when the file was written, not when the original was);
   - the encoding and the line ending as detected (the parser detects both; it does not yet record the line ending, so story 2 adds that);
   - the rows of every table the importer does not model, unchanged, in file order;
   - for the six modelled tables, the cells the model does not map (the source row, matched by its key and overlaid with the cells the model owns), so an unedited file does not change cells the person never touched.
   A replayed row that names a deleted task is dropped and listed in the export's "could not hold" list. In the `synth` layout `TASKRSRC` and `TASKACTV` rows carry `task_id` and `proj_id` fields; whether real P6 files follow the same names is unverified here, so story 2 reads the key field by name and lists any table it cannot check. The tasks the file gave keep the file's own `task_id`, `proj_id`, `wbs_id` and `clndr_id`; a task the person inserted gets an id above the largest in the file.
   - *Alternative:* extend the model until it holds every field and table (resources, activity codes, UDFs, currency), and write only from the model.
   - *Reason for the recommendation:* it is the A3 proposal, and it is the only one that does not silently drop what an opened P6 file holds (resources, assignments, activity codes: the crate README lists them under "Not read", and the gap table in PRODUCT.md, section 6, says no `ACTVCODE` table is read). A model that holds them is a large, unplanned build; replaying them costs a copy of the source in memory, which story 2 measures against the one hub run of 231 ms for the whole load of a 50,000-activity file (docs/IMPORT.md). *Costs:* memory about the size of the file; stale rows are possible when the person deletes or changes a task that a kept row refers to, which the drop-and-list rule above handles only for rows that name a deleted task.
   - A file opened from MSPDI has no source tables to replay. MSPDI export writes what the model holds and lists the rest (A3, as proposed).

3. **The default P6 version.** The version in the opened file's `ERMHDR`. A schedule not opened from an XER (a sample, an MSPDI file, a new blank schedule) writes `19.12`.
   - *Alternative:* a fixed default (the newest version) for every export, or a version list the person picks from.
   - *Reason for the recommendation:* writing back the version the file came from is the only choice the repo can test: the layouts of other versions are unverified, and the repo holds no file of a second version. `19.12` is the one version every fixture carries. An owner that asks for "version match or earlier" (PRODUCT.md, section 5) is served only when the file came from that version or an earlier one; the record does not claim more.
   - *Consequence for J9:* PRODUCT.md, J9 step 3 says the person chooses a version. Until a file of a second version is in the repo, the dialog shows the version it will write and does not offer others; offering a version the writer has no tested layout for would claim a fidelity nobody has measured. The plan's round-trip test for a second version (R5) cannot be written until that file exists. This narrows J9 and needs karpathy's word.
   - What a version cannot hold is listed in the "could not hold" list. Which fields differ between versions is not known from the repo; the list starts with what the model cannot write at all (for example resources and constraints other than start-no-earlier-than; the importer's `unsupported` list is the source for the first cut).

4. **Activity IDs for rows the person inserted.** Generate at insert, not at export, and store the result in `TaskMeta.code`, so the ID column shows what the file will carry and undo and redo restore the same value.
   - *Rule:* take the activity ID of the nearest preceding live row that has one, split it into a leading prefix and its trailing digits (`A1-2` is `A1-` and `2`; `A1010` is `A` and `1010`), and use the same prefix, the same digit width and the number one above the largest number any live row has with that prefix. If a row with that exact ID exists, add one until none does. If no row has an ID (a blank schedule, an MSPDI file), start at `A1000` and add 10 each time.
   - *Alternative:* generate at export, which leaves the ID column blank until then and can give a different value on each export.
   - *Reason for the recommendation:* the fixtures show that files use several shapes (`A1-1`, and `A1010` in the `meta.rs` examples), so a fixed pattern would fit none of them; following the neighbour fits the file. PRODUCT.md, A5, already says activity IDs are unique per project, and the plan on gs#703 says P6 needs a unique code per activity. The rule is deterministic, so the tests of story 5 can assert the value.
   - MSPDI export does not write the activity ID.

5. **Compaction (the 0001 question).** Export does not compact, does not change the session and does not clear the undo history. It skips tombstoned tasks and dead dependencies and writes the survivors. Save is therefore not a compaction point.
   - *Alternative:* compact on every save, as decision 0001 first said ("such as on save").
   - *Reason for the recommendation:* J9 keeps undo across a save (PRODUCT.md, A2), and compaction needs an empty history and invalidates every `TaskId` that JavaScript holds, so a save would erase the person's undo. A file written without tombstones, when reopened, is compact already, so the cost the 25% threshold guards against does not outlive the session. The `Remap` and the threshold trigger stay unbuilt until a measured need.
   - *Edit for a later row:* decision 0001 item "Compaction is still out" and the phrase "such as on save" should say that the J9 decision settled this: no save-time compaction. This record does not edit 0001.
   - Open risk: the file's ids of the surviving tasks keep gaps where rows were deleted. The fixtures show contiguous ids and the repo cannot tell whether P6 accepts gaps. The by-hand check on a machine with P6 (PRODUCT.md, J9 acceptance) covers it, and this record does not claim P6 accepts it.

6. **What Ctrl+S means.** *Superseded by [decision 0010](0010-document-format.md); kept as written for the record.* Export in the format that was opened, under the file's own name: an opened XER writes an XER at the opened version, an opened MSPDI file writes MSPDI. With no opened file (a sample or a blank schedule), Ctrl+S opens the Export dialog. The Export button always offers the other format and lists what that format cannot hold. The browser saves the file as a download; writing back over the original is a later story.
   - *Alternative:* a document format of its own (PRODUCT.md, A2), which holds the schedule plus the app data the engine never sees.
   - *Reason for the recommendation:* with item 2 the opened file is the document. A new format adds a reader and a writer that nobody else opens, and the owner's contract names P6 or MSPDI, not Tachlon. *Cost:* app data with no home in either format (A2 lists baselines and saved layouts) is not saved by Ctrl+S; it appears in the "could not hold" list, and a sidecar or a document format waits for the first story that needs one, with its own record.
   - J9 acceptance ("save then open gives the same document byte for byte on a second save") becomes a test of this record: write, read, write again gives the same bytes (plan round-trip test R1).
   - The browser-storage copy of J9 step 1 is not decided here (story 10 is held on gs#704). If it is built, the recommendation is that it stores the same bytes an export writes.

## Consequences

- If karpathy rules as recommended: stories 2 to 8 of the plan have their assertions. Story 2 keeps the source (header, encoding, line ending, unmodelled tables, unmapped cells of the modelled tables). The XER writer and its row writer follow items 1 and 3. Story 5 asserts item 4's IDs. Item 5 needs no engine change. Story 9 follows item 6.
- Edits that follow a ruling, each its own row: decision 0004 item 3 and `crates/xer/README.md` (item 1); decision 0001 (item 5); the dialog wording of J9 step 3 in PRODUCT.md (item 3).
- No claim here about how P6 or MS Project treat the output. That P6 accepts the file, and which layouts differ by version, stay unverified until the by-hand check and a second-version file exist (PRODUCT.md, J9 acceptance and assumption 4).
- Nothing here changes code, a test or user-facing text.
