# Save, reopen and export a schedule
<!-- journey: J9 -->

**Goal:** keep your work in a file you can open again, and write a P6 or MS Project file for
someone who needs one. A saved Tachlon file keeps the schedule, its baseline, the actuals and the
data date, but not an MS Project file's deadlines or resource names; an export is for sending, and
it leaves some things out.

<figure class="shot">
<img src="media/j9-save.webp" alt="The Open… menu open over the office fit-out sample: the samples, then Save with the note Downloads a .gantt file, Export the list as CSV, Save image (PNG) and Print...">
<figcaption>Save is in the Open… menu; the download button beside it exports a P6 or MS Project file.</figcaption>
</figure>

## By hand

1. **Save.** Press `Ctrl+S` with the chart focused, or click **Open…** in the toolbar and choose
   **Save**. The browser downloads `<name>.gantt`, named after the file you opened with its
   extension changed: `office-fitout.xml` saves as `office-fitout.gantt`. A note at the bottom right
   says `Saved office-fitout.gantt` until your next edit or the next file you open. Save is a
   download: it never writes over the file you opened.
2. **The unsaved dot.** After any edit a dot shows beside the file name in the toolbar, and the
   browser tab's title starts with `•`. A save clears it, and so does opening another file. Close
   or reload the page with the dot showing and the browser asks first.
3. **Finish an edit before you save.** While a cell is open for editing, or a bar is being dragged,
   nothing is saved. Press `Enter` or `Escape` first, then `Ctrl+S`.
4. **Reopen.** Press `Ctrl+O`, or click **Open…** and choose **Open a file…**, or drop the file on
   the chart. The chooser takes `.xer`, `.xml` and `.gantt` files, and the page reads a file by its
   content, never its name. A reopened `.gantt` file shows the schedule as you saved it: the edits,
   the baseline and its variance cells, the data date and the progress cells. The tab's title is the
   file's name, `office-fitout.gantt - Tachlon`, and the dot is off.
5. **Opening over unsaved edits asks first.** If the dot is showing, the browser asks `Open
   office-fitout.xer? The changes to the current schedule are not saved and will be lost.` **Cancel**
   keeps the schedule and its edits; **OK** replaces it. Save first if you want both.
6. **Export.** Click the download button beside **Open…** (its label is **Export…**). A window opens
   with two buttons, **MS Project XML** and **P6 XER**. Each one downloads `<name>.xml` or
   `<name>.xer` and names it in the window: `Downloaded office-fitout.xer as P6 19.12.` The P6 file is
   always version 19.12; there is no choice of version. **Close** shuts the window.
7. **What an export leaves out.** Under the file's name the window counts the items left out and
   lists them, one line each, up to 200 lines and then `and N more`. `Nothing was left out.` says
   the file holds the whole schedule. On the office fit-out sample the P6 window lists `the baseline
   on 50 tasks was not written`, and the MS Project window lists the reader's own note, `not in the
   project, left out by the reader: 47 tasks have resource assignments; only the resource names were
   read, and no dates are affected.` Both files keep each started task's actual start and
   remaining work and each finished task's actual finish, so on the sample, which
   has ten tasks under way, neither lists any of it (whether MS Project or P6 itself accepts the
   file is not checked: each needs a copy of its program). Opening the exported `.xml` shows it:
   **Detailed design and specification**, which read `2027-01-22` as its start, comes back started,
   at `2027-01-22`. Opening the
   exported `.xer` shows 60 rows against the schedule's 59: the extra one is a summary row at the
   top, named `Project 1`, that holds all the phases.
8. **Which file to keep.** Keep the `.gantt` file as the working copy: it holds the baseline, which a P6
   export leaves out; both exports keep the actuals and the data date. Export when a P6 or MS Project user needs the
   schedule, and read the window's list before you send it.
9. **What is not built.** **Save to cloud** and **Share a link** are in the **Open…** menu, grayed out
   as `Coming soon`. They are on [What's coming](coming.md).

## Where it is checked

`packages/gantt/tests/save-open.spec.ts` asserts `Ctrl+S` downloads the document and **Open** reads
it back, `save-name.spec.ts` the file's name and that a save during a cell edit saves nothing,
`unsaved-mark.spec.ts` the dot, `save-note.spec.ts` the note, `save-failure.spec.ts` the message
for a failed save, `open-unsaved.spec.ts` the question before an open over edits, and
`open-document.spec.ts` that a file opens by its content.
`packages/gantt/tests/export.spec.ts` edits a row and exports it to both formats.
`apps/hosted/tests/save-menu.spec.ts` asserts the **Save** item, and `menu-coming-soon.spec.ts` the
two grayed-out items.
`crates/app/tests/document_round_trip.rs` checks save, open and save again, and `export_mspdi.rs`
and `export_xer.rs` the two exports; `crates/import/tests/mspdi_export.rs` and `xer_export*.rs`
read each written file back and compare it with the schedule.
