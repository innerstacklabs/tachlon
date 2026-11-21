# Save, reopen and export a schedule
<!-- needs: chart -->

<!-- ANCHOR: goal -->
**Goal:** keep your work in a file you can open again, and write a P6 or MS Project file for someone who needs one. A saved `.gantt` file keeps the schedule, its baseline, the actuals and the data date, but not an MS Project file's deadlines or resource names; an export is for sending, and it leaves some things out.
<!-- ANCHOR_END: goal -->
<!-- shot: save-controls | the controls that save and export, and the note that says what was saved -->

<!-- ANCHOR: save -->
1. **Save.** Press `Ctrl+S` with the chart focused. The browser downloads `<name>.gantt`, named after the file you opened with its extension changed: `plan.xml` saves as `plan.gantt`. A note at the bottom right says `Saved plan.gantt` until your next edit or the next file you open. Save is a download: it never writes over the file you opened.
<!-- ANCHOR_END: save -->

<!-- ANCHOR: dot -->
2. **The unsaved dot.** After any edit a dot shows beside the file name in the toolbar, and the browser tab's title starts with `•`. A save clears it, and so does opening another file. Close or reload the page with the dot showing and the browser asks first.
3. **Finish an edit before you save.** While a cell is open for editing, or a bar is being dragged, nothing is saved. Press `Enter` or `Escape` first, then `Ctrl+S`.
<!-- ANCHOR_END: dot -->

<!-- ANCHOR: reopen -->
4. **Reopen.** Press `Ctrl+O`, or drop the file on the chart. The chooser takes `.xer`, `.xml` and `.gantt` files, and the page reads a file by its content, never its name. A reopened `.gantt` file shows the schedule as you saved it: the edits, the baseline and its variance cells, the data date and the progress cells. The tab's title is the file's name, such as `plan.gantt`, with the product's name after it, and the dot is off.
<!-- ANCHOR_END: reopen -->

<!-- ANCHOR: unsaved -->
5. **Opening over unsaved edits asks first.** If the dot is showing, the browser asks `Open plan.xer? The changes to the current schedule are not saved and will be lost.` **Cancel** keeps the schedule and its edits; OK replaces it. Save first if you want both.
<!-- ANCHOR_END: unsaved -->

<!-- ANCHOR: export -->
6. **Export.** Click the export button beside the Open… button (its label is Export…). A window opens with two buttons, MS Project XML and P6 XER. Each one downloads `<name>.xml` or `<name>.xer` and names it in the window: `Downloaded plan.xer as P6 19.12.` The P6 file is always version 19.12; there is no choice of version. Close shuts the window.
7. **What an export leaves out.** Under the file's name the window counts the items left out and lists them, one line each, up to 200 lines and then `and N more`. `Nothing was left out.` says the file holds the whole schedule. Both files keep each started task's actual start and remaining work and each finished task's actual finish, so a schedule with tasks under way lists none of it (whether MS Project or P6 itself accepts the file is not checked: each needs a copy of its program).
<!-- ANCHOR_END: export -->

<!-- ANCHOR: keep -->
8. **Which file to keep.** Keep the `.gantt` file as the working copy: it holds the baseline, which a P6 export leaves out; both exports keep the actuals and the data date. Export when a P6 or MS Project user needs the schedule, and read the window's list before you send it.
<!-- ANCHOR_END: keep -->
