// The feature steps: one per user-visible feature, each a short clip for the book (gs#152). Never published as its own
// video; the README story is steps.mjs. A PR that adds a user-visible feature adds its step here in the same diff (docs/CONTRIBUTING.md, "The screencast").
// The list has the shape of steps.mjs (a `setup` and `steps`) and starts at the opening view; scroll-to-critical comes
// first because the zoom, drag-bar and select steps use the critical task it locates.

import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { acceptOpenAsk, criticalPoint, openFilters, revealCritical, shared, zoomTo } from "./steps/helpers.mjs";
import { editSteps } from "./steps/edit.mjs";
import { cellCursor } from "./steps-cursor.mjs";
import { indentOutdent } from "./steps-indent.mjs";
import { insertDelete } from "./steps-rows.mjs";
import { links } from "./steps-links.mjs";
import { codeFilter, editLink, openP6, showMoved } from "./steps/p6.mjs";
import { openingView } from "./steps/opening.mjs";

/**
 * The book's "Open a schedule" clip (gs#454): open a file from the page, scroll to the first critical task, close the
 * panel of what was not read. clip.mjs puts its `--file` on `ctx.file`; without one (the journey spec, gs#396) it opens
 * the test fixture packages/gantt/tests/fixtures/ledger-timeline.xer. On the MacBook only demos/screencast/ is copied, so run.sh
 * also copies the fixture to ./fixtures/ beside this module, and that copy wins when it exists. It replaces the
 * 50,000-task schedule, so it stays the last scene. The name is not "open", which is the README story's own first
 * step (test-story.mjs); its bars draw in on the open (gs#309). The screenshots go through `ctx.shot` when the recorder provides one (clip.mjs does;
 * record.mjs does not).
 */
const COPIED = fileURLToPath(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const FIXTURE = existsSync(COPIED)
  ? COPIED
  : fileURLToPath(new URL("../../packages/gantt/tests/fixtures/ledger-timeline.xer", import.meta.url));
export const openFile = {
  name: "open-file",
  journey: "J1",
  caption: "Open a P6 or MS Project file",
  run: async (ctx) => {
    const file = ctx.file ?? FIXTURE;
    await ctx.shot?.("open-a-schedule-1", [{ selector: "#open-button", n: 1 }]);
    await ctx.ring("#open-button");
    const previous = (await ctx.page.evaluate(() => window.gantt.opened?.name ?? null));
    acceptOpenAsk(ctx.page);
    await ctx.page.locator("#open-input").setInputFiles(file);
    await ctx.until((was) => {
      const opened = window.gantt.opened;
      return opened !== undefined && opened.name !== was && window.gantt.status().n === opened.tasks;
    }, previous);
    // The open draws the bars of the first screen in (gs#309): a reopened session starts with no copy time, so one is
    // there only because the open started that slide. The hold lets the 360 ms draw-in play on the recording.
    if ((await ctx.page.evaluate(() => window.gantt.status().slide?.copyMs ?? null)) === null) {
      throw new Error("the open started no draw-in");
    }
    await ctx.hold(1.5);
    const located = await ctx.page.evaluate(() => {
      const app = window.gantt.app;
      const row = app.test_first_critical_row() ?? null;
      if (row === null) return null;
      const task = JSON.parse(app.table_rows(row, row + 1))[0]?.[0] ?? null;
      return task === null ? null : { row, task };
    });
    if (located === null) throw new Error("the opened schedule has no critical task");
    shared.criticalRow = located.row;
    shared.task = located.task;
    await revealCritical(ctx);
    await ctx.until(() => window.gantt.status().criticalBars > 0);
    await ctx.hold(1);
    const point = await criticalPoint(ctx, 12);
    const panelShown = await ctx.page.evaluate(() => document.getElementById("unsupported")?.hidden === false);
    // Each callout sits in empty space beside its target and covers no text: 1 just under the toolbar's file name (between
    // the task table's WBS and Task headers), not at the name's right edge, where "Import notes" is; 2 left of the critical bar.
    const info = await ctx.page.evaluate(() => {
      const box = document.querySelector("#open-info").getBoundingClientRect();
      return { x: box.left + 4, y: box.bottom + 24 };
    });
    await ctx.shot?.("open-a-schedule-2", [
      // The toolbar's one line: its file name (gs#646 removed the second line).
      { point: info, n: 1 },
      { point: { x: point.x - 28, y: point.y }, n: 2 },
      ...(panelShown ? [{ selector: "#unsupported-head", n: 3, side: "left" }] : []),
    ]);
    await ctx.hold(1);
    if (panelShown) {
      await ctx.ring("#unsupported-close", { ms: 1000, pulses: 1 });
      await ctx.page.locator("#unsupported-close").click();
      await ctx.until(() => document.getElementById("unsupported")?.hidden === true);
    }
    await ctx.hold(1.5);
  },
};
// The pre-roll (record/browser.mjs, preRoll): runs once after the frame-rate check, before the cut.
export const setup = openingView;

export const steps = [
  {
    name: "scroll-to-critical",
    journey: "J2",
    caption: "Scroll to the critical path (red)",
    run: async (ctx) => {
      const located = await ctx.page.evaluate(() => {
        const app = window.gantt.app;
        const row = app.test_first_critical_row() ?? null;
        if (row === null) return null;
        const task = JSON.parse(app.table_rows(row, row + 1))[0]?.[0] ?? null;
        const start = app.row_start(row);
        return task === null || start === undefined ? null : { row, task, start };
      });
      if (located === null) throw new Error("the schedule has no critical task");
      shared.criticalRow = located.row;
      shared.task = located.task;

      await revealCritical(ctx);
      await ctx.until(() => window.gantt.status().criticalBars > 0);
      await ctx.hold(1);
    },
  },
  {
    name: "zoom",
    journey: "J2",
    caption: "Zoom from years to days: weekends shade in, anchored at the cursor",
    run: async (ctx) => {
      const point = await criticalPoint(ctx);
      await zoomTo(ctx, point, "week");
      await ctx.hold(1);
      await zoomTo(ctx, point, "day");
      await ctx.hold(1);
    },
  },
  {
    name: "bar-look",
    journey: "J2",
    caption: "Rounded bars with air: a pastel body, a saturated fill for percent complete",
    run: async (ctx) => {
      await ctx.until(() => window.gantt.status().criticalBars > 0);
      await ctx.hold(2);
    },
  },
  {
    name: "bar-names",
    journey: "J2",
    pre: 'window.gantt.status().level === "day"', // the zoom scene before it leaves the day zoom
    caption: "Task names beside their bars from the weeks zoom inward",
    run: async (ctx) => {
      // The zoom step left the view at the day level, inward of weeks: every visible bar is named.
      await ctx.until(() => window.gantt.app.test_bar_labels().length > 0);
      await ctx.hold(2);
    },
  },
  {
    name: "driving-path",
    journey: "J2",
    caption: "Select a task: its driving path lights up from the task back, nearest first",
    run: async (ctx) => {
      await ctx.page.evaluate((task) => {
        window.gantt.app.clear_selection();
        window.gantt.app.select_task(task);
      }, shared.task);
      await ctx.until(() => window.gantt.status().selected !== null);
      await ctx.hold(2); // the reveal takes 300 ms; the rest is the whole path at rest
      await ctx.page.evaluate(() => window.gantt.app.clear_selection());
    },
  },
  {
    name: "header",
    journey: "J2",
    pre: 'window.gantt.status().level === "day"', // the zoom scene before it leaves the day zoom
    caption: "A two-tier header: week names over day letters, with a faint line at every day",
    run: async (ctx) => {
      // The zoom step left the view at the day level: a day is 14px or wider, so the lower tier holds day letters.
      await ctx.until(() => window.gantt.status().pxPerMinute * 1440 >= 14);
      await ctx.hold(2);
    },
  },
  {
    name: "pan",
    journey: "J2",
    caption: "Drag to pan the timeline",
    run: async (ctx) => {
      const box = await ctx.canvasBox();
      const x = box.x + box.width - 100;
      const y = box.y + ctx.AXIS / 2;
      const before = await ctx.page.evaluate(() => window.gantt.app.time_at(0));
      await ctx.page.mouse.move(x, y);
      await ctx.page.mouse.down();
      try {
        for (let k = 1; k <= 30; k++) {
          await ctx.page.mouse.move(x - 10 * k, y + 5 * k);
          await ctx.frames(1);
        }
      } finally {
        await ctx.page.mouse.up();
      }
      await ctx.until((start) => window.gantt.app.time_at(0) !== start, before);
      await ctx.hold(1);
    },
  },
  ...editSteps,
  {
    name: "critical-only",
    journey: "J2",
    caption: "Critical only: hide every task that has float",
    run: async (ctx) => {
      await ctx.page.keyboard.press("Shift+C");
      await ctx.until(() => {
        const status = window.gantt.status();
        return status.criticalOnly === true && status.rowCount < status.n;
      });
      // The view still shows the Level 1 dates; c jumps to the next critical task, so the filtered rows have bars.
      const before = await ctx.page.evaluate(() => ({ t: window.gantt.app.time_at(0), y: window.gantt.status().scrollY }));
      await ctx.page.keyboard.press("c");
      await ctx.until(({ t, y }) => window.gantt.app.time_at(0) !== t || window.gantt.status().scrollY !== y, before);
      await ctx.hold(1.5);
      await ctx.page.keyboard.press("Shift+C");
      await ctx.until(() => {
        const status = window.gantt.status();
        return status.criticalOnly === false && status.rowCount === status.n;
      });
    },
  },
  {
    name: "lookahead",
    journey: "J2",
    caption: "Lookahead: only the activities active in a 3-week window, with their summaries",
    run: async (ctx) => {
      await ctx.page.evaluate(() => {
        document.getElementById("table-body").scrollTop = 0;
      });
      await ctx.until(() => window.gantt.status().scrollY === 0);
      const total = (await ctx.status()).n;
      // A window in the middle of the schedule: the start date of the leaf halfway through the sorted leaf starts.
      const day = await ctx.page.evaluate(() => {
        const app = window.gantt.app;
        const rows = JSON.parse(app.table_rows(0, window.gantt.status().rowCount));
        const starts = rows.filter((row) => row[5] === 0).map((row) => row[1]).sort();
        return starts[Math.floor(starts.length / 2)].slice(0, 10);
      });
      // The button turns the window on with the default (the data date, or the first Monday, for 3 weeks); the date box
      // appears with it, and a new date moves the window.
      await openFilters(ctx.page);
      await ctx.page.locator("#lookahead-toggle").click();
      await ctx.until(() => window.gantt.status().lookahead.on === true);
      await ctx.page.locator("#lookahead-start").fill(day);
      await ctx.page.locator("#lookahead-start").blur();
      await ctx.until(({ day, n }) => {
        const status = window.gantt.status();
        return status.lookahead.start === day && status.rowCount > 0 && status.rowCount < n;
      }, { day, n: total });
      // Bring the window's first activity into view: pan until the first leaf row starts a fifth in from the left.
      const box = await ctx.canvasBox();
      await ctx.page.mouse.move(box.x + box.width / 2, box.y + ctx.AXIS + 10 * ctx.ROW);
      let before = Infinity;
      for (let i = 0; i < 1000; i++) {
        const dx = await ctx.page.evaluate((width) => {
          const app = window.gantt.app;
          const rows = JSON.parse(app.table_rows(0, window.gantt.status().rowCount));
          const row = rows.findIndex((entry) => entry[5] === 0);
          return app.x_of(app.row_start(row)) - width / 5;
        }, box.width);
        if (Math.abs(dx) < 1 || Math.abs(dx) >= before) break;
        before = Math.abs(dx);
        await ctx.page.mouse.wheel(Math.sign(dx) * Math.min(Math.abs(dx), 3000), 0);
        await ctx.frames(1);
      }
      await ctx.hold(2);
      await openFilters(ctx.page);
      await ctx.page.locator("#lookahead-toggle").click();
      await ctx.until((n) => {
        const status = window.gantt.status();
        return status.lookahead.on === false && status.rowCount === n;
      }, total);
    },
  },
  {
    name: "toolbar",
    journey: "J2",
    caption: "Toolbar: collapse to level 1, then expand all",
    run: async (ctx) => {
      const n = (await ctx.status()).n;
      await ctx.page.locator("#level-1").click();
      await ctx.until((n) => window.gantt.status().rowCount < n, n);
      await ctx.hold(1.5);
      await ctx.page.locator("#level-all").click();
      await ctx.until((n) => window.gantt.status().rowCount === n, n);
      await ctx.hold(0.5);
    },
  },
  insertDelete,
  indentOutdent,
  cellCursor,
  links,
  {
    name: "fit-flies",
    journey: "J2",
    caption: "Fit eases back to the whole project; a wheel or a key stops it where it is",
    run: async (ctx) => {
      // Zoomed in first, so Fit has somewhere to fly from.
      const point = await criticalPoint(ctx);
      await zoomTo(ctx, point, "day");
      await ctx.hold(1);
      await ctx.ring("#fit-button");
      await ctx.page.locator("#fit-button").click();
      // The flight takes 300 ms; the hold shows it fly and then the whole project.
      await ctx.hold(2.5);
    },
  },
  // The four P6 steps left the README story (gs#384): they open fit-out.xer, which replaces the schedule, so they come
  // after every step that works on this one, in the order the story had them, and before open-file, which stays last.
  openP6,
  codeFilter,
  editLink,
  showMoved,
  openFile,
];
