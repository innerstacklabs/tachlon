// Screencast steps that edit and navigate the tree: drag a bar, select, collapse, find.
import { criticalPoint, revealCritical, shared } from "./helpers.mjs";

export const editSteps = [
  {
    name: "drag-bar",
    journey: "J7",
    caption: (_ctx, result) => `Drag a bar: the critical path recomputes in ${result.recomputeMs.toFixed(1)} ms`,
    run: async (ctx) => {
      await revealCritical(ctx);
      const point = await criticalPoint(ctx, 4);
      await ctx.page.mouse.move(point.x, point.y);
      await ctx.until((task) => window.gantt.status().hover?.task === task, shared.task);
      const revision = (await ctx.status()).revision;
      const start = await ctx.page.evaluate((row) => window.gantt.app.row_start(row), shared.criticalRow);
      let recomputeMs = 0;
      let measured = false;
      const note = (drag) => {
        if (typeof drag?.recomputeMs !== "number") return;
        measured = true;
        recomputeMs = Math.max(recomputeMs, drag.recomputeMs);
      };
      let pressed = false;
      await ctx.page.mouse.down();
      pressed = true;
      try {
        await ctx.page.mouse.move(point.x + 4, point.y);
        await ctx.frames(1);
        await ctx.until(() => window.gantt.status().drag?.active === true);
        let status = await ctx.status();
        note(status.drag);
        for (let k = 1; k <= 8; k++) {
          await ctx.page.mouse.move(point.x + 15 * k, point.y);
          await ctx.frames(1);
          status = await ctx.status();
          if (status.drag?.active) note(status.drag);
        }
      } finally {
        if (pressed) await ctx.page.mouse.up();
      }
      await ctx.until((before) => window.gantt.status().revision > before, revision);
      await ctx.shot?.("j7-change", []);
      await ctx.page.keyboard.press("Control+Z");
      await ctx.until(({ row, es }) => window.gantt.app.row_start(row) === es, { row: shared.criticalRow, es: start });
      if (!measured) throw new Error("the drag status never reported recomputeMs");
      return { recomputeMs: Math.round(recomputeMs * 10) / 10 };
    },
  },
  {
    name: "select",
    journey: "J7",
    caption: "Click a bar: its driving path lights up",
    run: async (ctx) => {
      const point = await criticalPoint(ctx, 4);
      await ctx.page.mouse.move(point.x, point.y);
      await ctx.until((task) => window.gantt.status().hover?.task === task, shared.task);
      await ctx.page.mouse.click(point.x, point.y);
      await ctx.until((task) => window.gantt.status().selected?.task === task, shared.task);
      await ctx.hold(1.5);
      await ctx.page.keyboard.press("Escape");
      await ctx.until(() => window.gantt.status().selected === null);
    },
  },
  {
    name: "collapse",
    journey: "J2",
    caption: "Collapse a summary from the table",
    run: async (ctx) => {
      await ctx.page.evaluate(() => {
        document.getElementById("table-body").scrollTop = 0;
      });
      await ctx.until(() => window.gantt.status().scrollY === 0);
      const summary = await ctx.page.evaluate(() => {
        const app = window.gantt.app;
        const status = window.gantt.status();
        const rows = JSON.parse(app.table_rows(0, status.rowCount));
        const row = rows.findIndex((entry) => entry[5] === 1 && app.task_outline_level(entry[0]) === 1);
        if (row < 0) return null;
        const task = rows[row][0];
        const level = app.task_outline_level(task);
        let next = row + 1;
        while (next < rows.length && app.task_outline_level(rows[next][0]) > level) next++;
        return { row, task, hidden: next - row - 1, before: status.rowCount, n: status.n };
      });
      if (summary === null || summary.hidden <= 0) throw new Error("top-level expanded summary with children not found");
      if (summary.before !== summary.n) throw new Error(`expected ${summary.n} expanded rows, found ${summary.before}`);
      const disclosure = ctx.page.locator(`#table-rows .row[data-row="${summary.row}"] .disclosure`);
      await disclosure.click();
      await ctx.until((count) => window.gantt.status().rowCount === count, summary.before - summary.hidden);
      await ctx.hold(1);
      await disclosure.click();
      await ctx.until(({ before, n }) => {
        const status = window.gantt.status();
        return status.rowCount === before && status.rowCount === n;
      }, { before: summary.before, n: summary.n });
      await ctx.hold(0.5);
    },
  },
  {
    name: "find",
    journey: "J2",
    caption: "Ctrl+F: find a task by name",
    run: async (ctx) => {
      const query = await ctx.page.evaluate(() => {
        const app = window.gantt.app;
        const status = window.gantt.status();
        const rows = JSON.parse(app.table_rows(0, status.rowCount));
        const root = rows.findIndex((entry) => entry[5] === 1 && app.task_outline_level(entry[0]) === 1);
        if (root < 0) return null;
        const rootLevel = app.task_outline_level(rows[root][0]);
        let end = root + 1;
        while (end < rows.length && app.task_outline_level(rows[end][0]) > rootLevel) end++;
        for (let row = root + 1; row < end; row++) {
          if (rows[row][5] !== 1) continue;
          const text = app.task_name(rows[row][0]).toUpperCase();
          const needle = text.toLowerCase();
          const matches = rows.filter((entry) => app.task_name(entry[0]).toLowerCase().includes(needle));
          if (matches.length >= 3 && matches[0][0] === rows[row][0]) return text;
        }
        return null;
      });
      if (query === null) throw new Error("summary query with at least three matches not found");
      await ctx.page.keyboard.press("Control+f");
      await ctx.until(() => {
        const panel = document.getElementById("find");
        return panel !== null && !panel.hidden && document.activeElement?.id === "find-input";
      });
      for (const character of query) {
        await ctx.page.keyboard.type(character);
        await ctx.frames(2);
      }
      await ctx.until(() => window.gantt.status().selected !== null);
      const first = (await ctx.status()).selected.task;
      await ctx.hold(1);
      await ctx.page.keyboard.press("Enter");
      await ctx.until((task) => {
        const selected = window.gantt.status().selected;
        return selected !== null && selected.task !== task;
      }, first);
      await ctx.hold(1);
      await ctx.shot?.("j2-navigate", []);
      await ctx.page.keyboard.press("Escape");
      await ctx.until(() => document.getElementById("find-input")?.value === "");
      await ctx.page.evaluate(() => window.gantt.app.clear_selection());
      await ctx.until(() => window.gantt.status().selected === null);
    },
  },
];
