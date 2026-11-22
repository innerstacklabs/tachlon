# Tachlon has its own document; P6 and MS Project are import and export

Status: accepted. Decided by Dave, 2026-10-28 13:46 (terminal, "yes" to karpathy's recommendation). Supersedes item 6 of [decision 0009](0009-export-and-save.md). Items 1 to 5 of 0009 stand.

## Context

Decision 0009 item 6 made Ctrl+S write the format the schedule was opened from, with no file format of Tachlon's own. Two of the week's goals (docs/plan/week-2026-44.md in lifeos, Dave 2026-10-28) need state that neither P6 XER nor MSPDI can carry:

- Workflow (c), change and re-baseline, keeps baselines across sessions. A P6 file holds a baseline only as a separate project, which Tachlon does not write.
- The cloud version (gs#704: save, share, come back) stores a whole working session: baselines, saved layouts, and later comments and sharing.

PRODUCT.md A2 already named this need ("The document holds the engine schedule plus app data the engine never sees"), and milestone M11 already budgets a document crate and round-trip tests (5 points).

## Decision

1. **The Tachlon document is the saved state.** It holds the schedule, the app data the engine never sees (names, activity IDs, codes, outline, baselines, saved layouts, project name), and the original imported file's bytes when there is one.
2. **Ctrl+S always saves the Tachlon document and never asks which format.** The first Save of a schedule asks only where to save, suggesting `<name>.gantt` (the opened file's name with the extension changed, or `untitled.gantt` for a new schedule); later Saves write to the same place without asking. In a browser that cannot write back to a file, each Save is a download of the same name. The opened P6 or MS Project file is never overwritten by Save. In the hosted version, Save becomes autosave of the same document, and its storage is gs#704's to decide.
3. **Writing P6 or MS Project is always an explicit Export.** Export writes XER or MSPDI by 0009 items 1 to 5, lists what the target format cannot hold, and is the step that submits a schedule to an owner.
4. **Export replays from the embedded source.** 0009 item 2 (keep the unmodelled tables and cells, overlay what the model owns, replay the rest on export) reads them from the original bytes stored in the document, so a schedule saved, reopened and exported days later still writes the tables Tachlon does not model. Export to the format the schedule was imported from replays; export to the other format, or from a document with no original bytes, writes only what the model holds and lists the rest (0009 item 2, MSPDI case).
5. **Format: versioned JSON, gzip-compressed.** The decompressed payload is a JSON object with a top-level `"format": "gantt"` and an integer `"version"`, starting at 1. The original imported file, if any, is a base64 string field with its format name beside it. A reader refuses an unknown `format`, or a `version` newer than it knows, with an error that names the value; it reads every older version it has shipped. JSON because it is readable once decompressed, and both the Rust side and the browser read it with no new dependency. The field layout is the first implementing story's plan, reviewed before code.
6. **Round trip is the contract, on the decompressed JSON.** The writer emits keys in a fixed order with no insignificant whitespace, so save, open, save again gives the same decompressed bytes. That is the test (J9's acceptance). The compressed file is not required to be byte-identical across gzip settings.
7. **Size gate.** The first implementing story saves the 50,000-task stress schedule (seed 20261020) imported from an XER, original bytes included, and reports the compressed size. Over 10 MB, it returns to this record before shipping.

## Consequences

- J9 in PRODUCT.md reads "Save, reopen and submit": Save and reopen use the document; submit is Export.
- Story 9 of the workflow 4 plan on gs#703 (Ctrl+S) follows item 2 of this record, not 0009 item 6. Stories 2 to 8 (the XER and MSPDI writers) are unchanged.
- A new story builds the document reader and writer and its round-trip tests, before story 9. Where the code lives (its own crate or a module beside the session) is settled in that story's plan, within decision 0005's public surface and decision 0007's split (the hosted app reads and writes documents only through `gantt-core`).
- The cost: someone can forget to export before sending. The `.gantt` name and the export dialog keep that visible; a "changed since last export" mark is a possible later story, not decided here. Comments and sharing are not in version 1 of the document; they arrive with the hosted version's own record.
- The browser-storage copy (0009's note on J9 step 1) stores the document.

## Amended by decisions 0017 and 0018

### Amendment (gantt#2110, decisions 0017 and 0018, karpathy 2026-11-16)

Items 1 to 7 stand. Two later records say what the document is and is not:

- *The document is the storage form, not the host's record form* ([decision 0017](0017-host-record-vocabulary.md)). It keeps snake_case keys, integer minutes, instants as minutes from `origin`, and tasks and links by position. Plain data in, the snapshot, `results()`, edit changes and `update(records)` use the host record vocabulary; decision 0017 holds the mapping from each vocabulary field to its document key. `durationUnit` and `dateForm` are never stored.
- *The top level gains one optional key, `options`* ([decision 0018](0018-schedule-options-and-host-capabilities.md)): the schedule options, written only when a member differs from its default, so a schedule on defaults keeps its saved bytes (item 6). Its member names are camelCase, the one exception to the document's snake_case, because the same object is plain data's `options`. A reader refuses an unknown member. A build older than this key refuses a document that holds it; `version` stays 1, as it did for the task `key`.
- *The top level gains a second optional key, `must_finish_by`* (gantt#2074; named by [decision 0017](0017-host-record-vocabulary.md)): the date the project must finish by, in minutes from `origin`, written only when a date is set. A reader refuses a value outside `MIN_DATE..=MAX_DATE` (decision 0020) and a repeat of the key; a build older than this key refuses a document that holds it; `version` stays 1. The buffer it leaves is solved, never stored.
- *A task gains an optional key, `physical_percent`* (tl#2080; [decision 0008](0008-progress-model.md) item 1a): the task's physical percent complete in hundredths of a percent, written after `progress` and only when the task has one, so a document with none keeps its text (item 6). A reader refuses a value over 10000; a build older than this key refuses a document that holds it; `version` stays 1.
- *Host capabilities are never in the document* (decision 0018).
