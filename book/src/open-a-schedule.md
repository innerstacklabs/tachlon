# Open a schedule

<!-- journey: J1 -->

{{#include ../../packages/gantt/guide/open.md:goal}}

<video class="clip" controls muted playsinline preload="metadata" poster="media/open-a-schedule.webp">
  <source src="media/open-a-schedule.mp4" type="video/mp4">
</video>

## Get a file out of P6

{{#include ../../packages/gantt/guide/open.md:p6}}

## Get a file out of MS Project

{{#include ../../packages/gantt/guide/open.md:msp}}

## Steps

{{#include ../../packages/gantt/guide/open.md:open}}

   On this page, you can also click the **Open…** button in the toolbar: it opens a menu with **New schedule** and **Open a file…** first; choose **Open a file…**.

{{#include ../../packages/gantt/guide/open.md:read}}

<figure class="shot">
<img src="media/open-a-schedule-1.webp" alt="The toolbar with the Open button marked 1">
<figcaption>1: the Open… button. The chart behind it is the <a href="/?sample=office-fitout">office fit-out sample</a>, the schedule the page opens with until you open a file or pick another sample.</figcaption>
</figure>

<figure class="shot">
<img src="media/open-a-schedule-2.webp" alt="An opened XER file: the one line about the file in the toolbar marked 1, a red critical activity marked 2">
<figcaption>1: the one line the toolbar shows about your file: its name. Point at it to read its format and its task count too. A long line is cut with …; the tooltip has the whole of it. 2: a critical activity, red on the chart and in the task list. The panel of what was not read is shut: step 5 says how to open it.</figcaption>
</figure>

<figure class="shot">
<img src="media/open-a-schedule-tour.webp" alt="The office fit-out sample open: the task table marked 1, the timeline marked 2, the toolbar marked 3, a critical bar marked 4">
<figcaption>The office fit-out sample, open.</figcaption>
</figure>

1. **The task table.** One row per activity, indented under its summary. A red diamond marks a
   critical activity. Click a summary's chevron to fold it.
2. **The timeline.** Each activity is a bar on the time axis, with its links drawn between bars.
   Gray bands are non-working days. The dashed line is the data date.
3. **The toolbar.** **Open…**, **Export…** and **Checks**, the file name, **Filters**, **Baseline**, **Progress** and **Columns**, **Insert** and **Delete**, the outline levels, zoom and **Fit**, the data date, the find box and `?`.
4. **The critical path.** Critical bars are red. Press <kbd>Shift</kbd>+<kbd>C</kbd> to show only them.

<div class="book-tip">
<img src="media/icon.svg" alt="">
<p><strong>Tip from Tachlon.</strong> Press <kbd>Shift</kbd>+<kbd>C</kbd> to show only the critical path, and again to go back to the view you had. A plain <kbd>c</kbd> jumps to the next critical activity.</p>
</div>

## What the panel says about an MS Project file

{{#include ../../packages/gantt/guide/open.md:panel}}

## Good to know

- The app is built for schedules of 50,000 activities. The aim is to open one in under two
  seconds. On one machine, a MacBook Pro M3 Pro, a generated 50,000-activity P6 file opened in
  171.5 ms and a generated MS Project file in 171.0 ms at the 95th percentile
  (`docs/MEASURING.md`, "Open time"). No other machine has been measured.
{{#include ../../packages/gantt/guide/open.md:know}}
