// The words of the chart's own markup (gantt#1875): `chart.html` names each by a key (`data-t` for an element's text, `data-t-title`,
// `data-t-aria-label` and `data-t-placeholder` for the attribute of that name), and these are the English the chart shows for each key.
// A host's bundle (`MountOptions.texts`, `applyTexts` in `locale.ts`) replaces any of them; a key it leaves out stays English. `MESSAGES_EN` below is the sentences the chart writes (`messages.ts`).
export const TEXTS_EN: Readonly<Record<string, string>> = {
  // The toolbar's groups and buttons.
  "chart-tools": "Chart tools", "file": "File", "baseline": "Baseline", "progress": "Progress", "zoom": "Zoom", "fit": "Fit", "variance": "Variance",
  "columns": "Columns", "variance-title": "Hide or show the Start, Finish and Float variance columns",
  "progress-title": "Show the Actual start, Actual finish, Remaining and % complete columns",
  "columns-title": "Show or hide the Start, Finish and Float columns, and move Dur, Start, Finish and Float",
  "fit-title": "Fit: show the whole project, from its first start to its finish (f)",
  // The columns menu: the show and move controls of the four columns that move.
  "move-dur-up": "Move Dur up", "move-dur-up-title": "Move the Dur column up the list, left in the table", "move-dur-down": "Move Dur down",
  "move-dur-down-title": "Move the Dur column down the list, right in the table", "show-start-title": "Show or hide the Start column",
  "move-start-up": "Move Start up", "move-start-up-title": "Move the Start column up the list, left in the table",
  "move-start-down": "Move Start down", "move-start-down-title": "Move the Start column down the list, right in the table",
  "show-finish-title": "Show or hide the Finish column", "move-finish-up": "Move Finish up",
  "move-finish-up-title": "Move the Finish column up the list, left in the table", "move-finish-down": "Move Finish down",
  "move-finish-down-title": "Move the Finish column down the list, right in the table", "show-float-title": "Show or hide the Float column",
  "move-float-up": "Move Float up", "move-float-up-title": "Move the Float column up the list, left in the table",
  "move-float-down": "Move Float down", "move-float-down-title": "Move the Float column down the list, right in the table",
  // The task list, its splitter and the timeline.
  "task-list": "Task list", "resize": "Resize the task list",
  "resize-title": "Drag to resize the task list; double-click to reset; arrow keys move it 16 px", "timeline": "Timeline", "task": "Task",
  "dur": "Dur", "start": "Start", "finish": "Finish", "float": "Float", "float-title": "Total float, in working days of each task's own calendar",
  "legend": "Legend",
  // The link editor, the task dialog and the notes.
  "link": "Link", "link-input": "Link type and lag, such as FS or SS+2d",
  "link-title": "Type and lag: FS, SS, FF or SF, then an optional lag such as +2d. Enter applies, Escape closes.", "name": "Name",
  "duration": "Duration", "percent": "Percent complete", "actual-start": "Actual start", "actual-finish": "Actual finish", "remaining": "Remaining",
  "snlt": "Start no later than", "fnet": "Finish no earlier than", "fnlt": "Finish no later than",
  "date-format": "YYYY-MM-DD", "save": "Save", "cancel": "Cancel", "dismiss": "Dismiss", "reload": "Reload", "show-moved": "Show moved",
  "measurement-link": "Download the measurement (JSON)", "loading": "loading…",
};
// The names of the columns and fields the script writes while the chart runs (`field-names.ts`): the head cells the table adds, the hover card's rows and
// the cell editor's name for a screen reader. A name the markup also has (`start`, `finish`, `duration`) keeps its key in `TEXTS_EN`.
export const FIELDS_EN: Readonly<Record<string, string>> = {
  "id": "ID", "wbs": "WBS", "percent-head": "% complete", "task-name": "Task name",
  "var-start": "ΔStart", "var-finish": "ΔFinish", "var-float": "ΔFloat", "var-start-title": "Start against the baseline, in working days",
  "var-finish-title": "Finish against the baseline, in working days", "var-float-title": "Float against the baseline, in working days",
  "start-no-earlier": "Start no earlier than", "deadline": "Deadline", "late-start": "Late start", "late-finish": "Late finish", "calendar": "Calendar",
  "resources": "Resources", "critical": "Critical", "driven-by": "Driven by", "status": "Status", "baseline-start": "Baseline start",
  "baseline-finish": "Baseline finish", "predecessors": "Predecessors", "successors": "Successors",
};
// The sentences the chart writes while it runs (`messages.ts`): `{name}` holes, and for a count a form per plural category after a dot. A host's `texts` bundle replaces any of them.
export const MESSAGES_EN: Readonly<Record<string, string>> = {
  // Opening and loading a schedule.
  "not-opened": "{name} was not opened. {why}", "open-confirm": "Open {name}? The changes to the current schedule are not saved and will be lost.",
  "too-big": "{name} is {size}; files over {limit} MiB are not opened.", "bytes.one": "{count} byte", "bytes.other": "{count} bytes", "mib": "{size} MiB",
  "empty": "{name} is empty.", "unreadable": "could not read {name}: {why}", "unopenable": "could not open {name}: {why}",
  "unopenable-format": "could not open {name}: {why}. {product} opens P6 XER, MS Project XML and .gantt files.",
  "projects": "{name} holds {count} projects; open it with the project to read, or \"all\".", "not-loaded": "Not loaded. {why}",
  "open-busy": "Finish the cell edit or drag first.", "edited": "The schedule was edited while the file was read.", "newer-file": "A newer file was opened first.",
  // Saving, the selection, the row position and the baseline.
  "put-back.one": "{why} ({count} edit put back)", "put-back.other": "{why} ({count} edits put back)",
  "saved": "Saved {file}", "rows-selected": "{count} rows selected", "row-position": "Row {row} of {total}",
  "baseline-set.one": "Baseline set: {count} row", "baseline-set.other": "Baseline set: {count} rows",
  "baseline-set-at.one": "Baseline set at data date {day}: {count} row", "baseline-set-at.other": "Baseline set at data date {day}: {count} rows",
  // The sentence a screen reader hears for the selected bar (`bar-speech.ts`).
  "bar-dates": "{name}. Start {start}, finish {finish}", "bar-float": "float {float}", "bar-critical": "critical {value}", "bar-yes": "yes", "bar-no": "no",
  "bar-percent": "{percent} complete", "bar-predecessors.one": "{count} predecessor", "bar-predecessors.other": "{count} predecessors",
  "bar-successors.one": "{count} successor", "bar-successors.other": "{count} successors", "bar-baseline": "baseline start {start}",
  // A frame that failed.
  "render-stopped": "Render error: {why}. The chart stopped; the last good view is still shown.",
  "render-stopped-reload": "Render error: {why}. The chart stopped; the last good view is still shown. Reload to recover.",
  "render-failed": "Render error: {why}. The last good view is still shown (failed {failed} of {max}).",
  "render-recovered": "Render error: {why}. The chart is drawing again.",
};
// The words of the assistant's chat panel (`assistant/panel.ts`); they are not in the chart's markup, and a host's `texts` bundle replaces any of them the same way.
export const ASSISTANT_EN: Readonly<Record<string, string>> = {
  "assistant": "Assistant", "assistant-log": "Conversation", "assistant-message": "Message", "assistant-send": "Send",
  "assistant-accept": "Accept", "assistant-reject": "Reject", "assistant-accepted": "Change applied.", "assistant-rejected": "Change dropped.",
  "assistant-refused": "The change was not applied.", "assistant-failed": "The assistant could not answer.", "assistant-limit": "The assistant asked for too many steps and was stopped.",
};
