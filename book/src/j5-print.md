# Print a schedule
<!-- journey: J5 -->

**Goal:** put the schedule on paper, or in a PDF, as pages with a header and a legend, and save a
picture of the first page. The pages hold the rows you are looking at, in the order you see them.

<figure class="shot">
<img src="media/j5-print.webp" alt="Page 1 of 3 of the printed office fit-out sample: a header with the file's name, the data date and the page number, the task list with Task, Dur, Start, Finish and Float columns, and the chart beside it">
<figcaption>Page 1 of the printed office fit-out, as Save image (PNG) saves it: a long name stops inside its column. This still was taken when dates printed with their time; they now print as the day alone.</figcaption>
</figure>

## By hand

1. **Print.** Press `Ctrl+P`, or click **Open…** in the toolbar and choose **Print...**. The menu
   closes and the browser's print dialog opens. Pick a printer there, or the browser's "Save as
   PDF" destination, to get a file. The browser does the saving; the page has no PDF button.
2. **What a page holds.** Every page is US Letter, landscape, whatever the size of your window.
   A header runs across the top: the file's name, `Data date: YYYY-MM-DD HH:MM`, `Page 1 of 3` and
   `Printed` with today's date. Under it are the task list and the timeline, with the axis, the bars,
   the links and the data date line as they are on screen. A legend at the foot names the colours:
   Task, Critical, Deadline, Start constraint, Finish constraint, Summary, Dependency, Data date, Non-working day, Proposal ghost and Just-moved outline. A page takes 25 rows, so the
   59-row office fit-out sample prints on 3 pages.
3. **Which columns.** The columns of the task list as it stands: Task, Dur, Start, Finish and Float,
   and the variance cells once a baseline is set. A column you hid in the **Columns** menu is left off
   the page, and the columns print in the order you gave them there. The four cells of the **Progress**
   button print while they are on and are left off the page while they are off, and a file with an ID or
   WBS column prints it. A date prints as the day (`2026-10-05`) without its time. The variance
   columns are headed `Start var.`, `Finish var.` and `Float var.` on the page, and a WBS code too long for its cell keeps its end
   behind an ellipsis (`…PACKAGE.12`). The list text is 11 px; when the columns
   would not all fit whole it shrinks, to 9 px at the least, and then the chart gives up width (down to 48 px) before a column is
   cut. Hiding a column changes the printout only: **Copy** and CSV still hold every column.
4. **Which rows.** Every row the list shows, in order: a collapsed branch prints as its summary row
   alone, and a filter prints only the tasks it keeps. A filter shows in the header, as
   `Filter: critical` after **Critical only**, or `Filter: code Trade = Framing` and `Filter:
   status in-progress` with the value that is on. A Lookahead window shows there too, as
   `Lookahead: 2027-02-05, 3 weeks`; with no window the header has no such text. To print part of a schedule, select a range of rows
   first (click a row, then `Shift`+click another, as in [Build a schedule from the
   keyboard](j6-build.md)); only those rows print, and the page count follows. `Escape` ends the
   range.
5. **A long schedule.** At most 100 pages print. The generated 50,000-task demo fills all 100, and
   the last page says `47500 rows not printed. Select a range to print them.` Select a range, or
   filter, to print the rows that did not fit.
6. **Letter, or A4.** `Ctrl+P` and **Print...** print on US Letter. For A4, click **Open…**, then
   **Print on A4...**: the pages are A4 landscape (the same 25 rows to a page, on A4's own size),
   and the browser's print dialog opens on A4. It is for that one print: the next `Ctrl+P` is Letter
   again. There is no other paper size.
7. **Save a picture of the first page.** Click **Open…**, then **Save image (PNG)**. The page
   downloads `<file name>.png`, the first printed page as a picture 2,112 by 1,632 pixels on a white
   background (`office-fitout.png` for the sample). It has the columns the screen shows, in the
   screen's order, as the printed pages do. Only page 1 is in it; the picture does not open the print
   dialog.
8. **Keep a print setup.** Click **Open…**, then **Save layout...**, and type a name. This browser
   keeps the row filter, the Lookahead window and the list columns as the screen shows them (the
   Columns menu's hides and order, the Progress and Variance buttons), and the name is the layout's.
   Another schedule, or the same one next month, opens with its own columns: click **Open…**, then
   **Apply layout...**, and type one of the names it lists. A line under the toolbar says what went
   in, and names any part the schedule could not take (a filter that needs a baseline it has not, or a
   column it has none of); a part not applied changes nothing. The layouts stay in this browser only:
   another browser or another person's computer has none. A layout does not hold the paper or a date range.
9. **What is not built.** Choosing a date range, and picking printed columns apart from the
   screen's, are on [What's coming](coming.md).

## Where it is checked

`packages/gantt/tests/print-pages.spec.ts` asserts that `beforeprint` puts one SVG per page on the
sheet and `afterprint` removes them, that printing leaves the scroll and zoom as they were, and that a column hidden in the Columns menu is not on the page.
`packages/gantt/tests/print-sheet.spec.ts` asserts the plain text sheet that prints when no page
was filled, and `journey-j5.spec.ts` prints a filtered schedule and checks that every page has a
header and a legend and that the pages hold every shown row in order, then that `printOn("a4")`
prints A4 pages and the next print is Letter.
`apps/hosted/tests/print.spec.ts` asserts that print media shows only the pages and that the saved
PDF is Letter landscape (792 by 612 points), and that **Print on A4...** closes the menu and prints A4
landscape pages (842 by 595 points) once, with Letter again after it; `print-menu.spec.ts` that
**Print...** closes the menu and calls `window.print` once, and `export-image.spec.ts` the picture's file name, PNG signature and
size.
`apps/hosted/tests/layouts.spec.ts` saves a layout from the File menu, reloads on another sample, applies it and checks the same columns in the same order.
`crates/app/tests/print_columns.rs` checks that the columns a page prints are the ones it is told, in that order, that a hidden Float leaves the name column its width, that a date prints as a day, that the list text stays at 9 px or more and that the timeline names its months.
`crates/app/tests/print_pages.rs`, `print_lookahead.rs` and `print_legend.rs` check the names on the pages against the
visible rows, the page count, the header's data date, filter, Lookahead window and print date, the left-out line, a
range and the legend; `print_pages.rs` also checks the A4 layout.
