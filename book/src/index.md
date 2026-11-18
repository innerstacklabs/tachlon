# Tachlon

*A guide for developers who put a Gantt chart in their own product, and for the people who use that chart.*

Tachlon is a Gantt and scheduling component that you put in your own product, in the same category as Bryntum Gantt, DHTMLX Gantt and Syncfusion Gantt. It is fully featured, blazingly fast, and open source with no strings. It is built for developers who work with an AI assistant: the package carries an `AGENTS.md` written for the assistant, and one prompt installs the chart.

This book has two parts, for two readers.

- **You are a developer and you want the chart in your product.** Start at [Install it in your app](install.md), the first chapter of the part for developers.
- **You use a chart built with Tachlon.** Read the guide chapters. Each chapter is one task: what you can do, a short clip of it, the steps, and a tip. They assume nothing about how the chart is built.

To try the chart without a file of your own, [open the office fit-out sample](/?sample=office-fitout), the small schedule the hosted app starts with; **Open…** loads a P6 or MS Project file of your own.

## Contents

**For developers**

- [Install it in your app](install.md): install the package, let `npx tachlon init` write the chart component, and open a schedule file.

**Guide**

1. [Open a schedule](open-a-schedule.md): drop a P6 .xer or MS Project .xml file on the chart and read it.
2. [Find your way around a big schedule](j2-navigate.md): zoom, collapse, filter and find.
3. [Build a schedule from the keyboard](j6-build.md): insert rows, type names and durations, indent and outdent.
4. [Change dates and see what moved](j7-change.md): drag a bar or edit a date, then see which tasks moved.
5. [Edit a task in its dialog](j10-task-dialog.md): change several of a task's fields in one place, and undo the whole change in one step.
6. [Update progress against a data date](j8-progress.md): record what happened on site and move the data date forward.
7. [Set a baseline, read what slipped](baseline.md): keep the agreed plan and read what has slipped against it.
8. [Check a schedule](j4-checks.md): count the tasks tied to nothing, odd links, and tasks that are very long or very loose.
9. [Print a schedule](j5-print.md): put the schedule on paper or in a PDF, with a header and a legend.
10. [Save, reopen and export a schedule](j9-save.md): keep your work in a file, and write a P6 or MS Project file for someone else.
11. [Keys](keys.md): every key and what it does.
12. [What's coming](coming.md): the chapters whose features are not built yet.

## See it move

<video class="clip" controls muted playsinline preload="metadata" poster="media/screencast.webp">
  <source src="media/screencast.mp4" type="video/mp4">
</video>
