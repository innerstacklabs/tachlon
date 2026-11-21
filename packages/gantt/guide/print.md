# Print a schedule
<!-- needs: chart -->

<!-- ANCHOR: goal -->
**Goal:** put the schedule on paper, or in a PDF, as pages with a header and a legend, and save a picture of the first page. The pages hold the rows you are looking at, in the order you see them.
<!-- ANCHOR_END: goal -->
<!-- shot: print-page-1 | page 1 of a printed schedule: a header with the file's name, the data date and the page number, the task list with its columns, and the chart beside it -->

<!-- ANCHOR: print -->
1. **Print.** Press `Ctrl+P`. The browser's print dialog opens. Pick a printer there, or the browser's "Save as PDF" destination, to get a file. The browser does the saving; the chart has no PDF button.
<!-- ANCHOR_END: print -->

<!-- ANCHOR: pages -->
2. **What a page holds.** Every page is US Letter, landscape, whatever the size of your window. A header runs across the top: the file's name, `Data date: YYYY-MM-DD HH:MM`, `Page 1 of 3` and `Printed` with today's date. Under it are the task list and the timeline, with the axis, the bars, the links and the data date line as they are on screen. A legend at the foot names the colors: Task, Critical, Deadline, Start constraint, Finish constraint, Summary, Dependency, Data date, Non-working day, Proposal ghost and Just-moved outline. A page takes 25 rows, so a schedule of 59 rows prints on 3 pages.
3. **Which columns.** The columns of the task list as it stands: Task, Dur, Start, Finish and Float, and the variance cells once a baseline is set. A column you hid in the **Columns** menu is left off the page, and Free float, which the menu starts hidden, prints once you show it, and the columns print in the order you gave them there. The four cells of the **Progress** button print while they are on and are left off the page while they are off, and a file with an ID or WBS column prints it. A date prints as the day (`2026-10-05`) without its time. The variance columns are headed `Start var.`, `Finish var.` and `Float var.` on the page, and a WBS code too long for its cell keeps its end behind an ellipsis (`…PACKAGE.12`). The list text is 11 px; when the columns would not all fit whole it shrinks, to 9 px at the least, and then the chart gives up width (down to 48 px) before a column is cut. Hiding a column changes the printout only: Copy and CSV still hold every column.
4. **Which rows.** Every row the list shows, in order: a collapsed branch prints as its summary row alone, and a filter prints only the tasks it keeps. A filter shows in the header, as `Filter: critical` after Critical only, or `Filter: code Trade = Framing` and `Filter: status in-progress` with the value that is on. A Lookahead window shows there too, as `Lookahead: 2027-02-05, 3 weeks`; with no window the header has no such text. To print part of a schedule, select a range of rows first (click a row, then `Shift`+click another); only those rows print, and the page count follows. `Escape` ends the range.
5. **A long schedule.** At most 100 pages print. A schedule of 50,000 tasks fills all 100, and the last page says `47500 rows not printed. Select a range to print them.` Select a range, or filter, to print the rows that did not fit.
<!-- ANCHOR_END: pages -->

## Letter, or A4
<!-- needs: call:printOn -->

<!-- ANCHOR: paper -->
6. **Letter, or A4.** `Ctrl+P` prints on US Letter. A product can print on A4 instead: the pages are A4 landscape (the same 25 rows to a page, on A4's own size), and the browser's print dialog opens on A4. It is for that one print: the next `Ctrl+P` is Letter again. There is no other paper size.
   <!-- control: printOn | opens the print dialog on Letter or A4 paper, for this one print -->
<!-- ANCHOR_END: paper -->

## Save a picture of the first page
<!-- needs: call:exportImage -->

<!-- ANCHOR: image -->
7. **Save a picture of the first page.** The picture downloads as `<file name>.png`, the first printed page 2,112 by 1,632 pixels on a white background. It has the columns the screen shows, in the screen's order, as the printed pages do. Only page 1 is in it; the picture does not open the print dialog.
   <!-- control: exportImage | downloads a picture of a printed page as a PNG file -->
   <!-- shot: print-menu | the menu that holds the print and picture items -->
<!-- ANCHOR_END: image -->

## Keep a print setup
<!-- needs: call:layoutJson, call:applyLayout -->

<!-- ANCHOR: layout -->
8. **Keep a print setup.** A layout keeps the row filter, the Lookahead window and the list columns as the screen shows them (the **Columns** menu's hides and order, the **Progress** and **Variance** buttons), under a name. Another schedule, or the same one next month, opens with its own columns once you apply a layout by name. A line under the toolbar says what went in, and names any part the schedule could not take (a filter that needs a baseline it has not, or a column it has none of); a part not applied changes nothing. A layout does not hold the paper or a date range.
   <!-- control: layoutJson | returns the print setup on screen as text, under a name -->
   <!-- control: applyLayout | puts a kept print setup back on the schedule and lists the parts it could not take -->
<!-- ANCHOR_END: layout -->
